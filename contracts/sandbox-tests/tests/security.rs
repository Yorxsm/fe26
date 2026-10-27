//! End-to-end sandbox checks. Build the release wasm first:
//! `cargo build --release --target wasm32-unknown-unknown -p arena-factory -p arena-registry -p heist-game`
//! Then `cargo test -p fe2vi-sandbox --features sandbox`.

use anyhow::Context;
use near_workspaces::types::{KeyType, NearToken, SecretKey};
use near_workspaces::{Account, Contract, Worker};
use serde_json::json;

fn wasm(name: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!(
        "{}/../../target/wasm32-unknown-unknown/release/{name}.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).with_context(|| format!("missing {path}; build the release wasm first"))
}

fn rules(template: &str) -> serde_json::Value {
    json!({
        "template_id": template,
        "max_players": 6,
        "duration_seconds": 3600,
        "round_count": 4,
        "pot_yocto": "10000000000000000000000000",
        "per_agent_cap_yocto": "2000000000000000000000000",
        "per_action_cap_yocto": "500000000000000000000000",
        "entry_yocto": "0"
    })
}

const POT: u128 = 10_000_000_000_000_000_000_000_000;
const SCOUT: u128 = 125_000_000_000_000_000_000_000;
const HEIST_HASH: &str = "320421571d7cf9e178b0b5c0aa1dce8d3ea3f2dbf0bdc507849aaf70a121651d";

struct Arena {
    worker: Worker<near_workspaces::network::Sandbox>,
    registry: Contract,
    factory: Contract,
    heist: Contract,
    moderator: Account,
    creator: Account,
    owner: Account,
}

async fn deploy(real_registry: bool) -> anyhow::Result<Arena> {
    let worker = near_workspaces::sandbox().await?;
    let registry_wasm = wasm("arena_registry")?;
    let factory_wasm = wasm("arena_factory")?;
    let heist_wasm = wasm("heist_game")?;
    let registry = worker.dev_deploy(&registry_wasm).await?;
    let factory = worker.dev_deploy(&factory_wasm).await?;
    let heist = worker.dev_deploy(&heist_wasm).await?;
    let owner = worker.dev_create_account().await?;
    let moderator = worker.dev_create_account().await?;
    let creator = worker.dev_create_account().await?;
    let registry_id = if real_registry {
        registry.id().clone()
    } else {
        owner.id().clone()
    };
    if real_registry {
        registry
            .call("new")
            .args_json(json!({
                "owner": owner.id(),
                "heist": heist.id(),
                "factory": factory.id(),
            }))
            .transact()
            .await?
            .into_result()?;
    }
    factory
        .call("new")
        .args_json(json!({
            "owner": owner.id(),
            "moderator": moderator.id(),
            "registry": registry_id,
            "heist": heist.id(),
        }))
        .transact()
        .await?
        .into_result()?;
    heist
        .call("new")
        .args_json(json!({
            "registry": registry_id,
            "factory": factory.id(),
        }))
        .transact()
        .await?
        .into_result()?;
    Ok(Arena {
        worker,
        registry,
        factory,
        heist,
        moderator,
        creator,
        owner,
    })
}

async fn open_game(arena: &Arena, creator: &Account) -> anyhow::Result<String> {
    let outcome = creator
        .call(arena.factory.id(), "create_game")
        .args_json(json!({
            "rules": rules("heist-night"),
            "game_md_hash": HEIST_HASH,
        }))
        .deposit(NearToken::from_yoctonear(POT))
        .gas(near_workspaces::types::Gas::from_tgas(300))
        .transact()
        .await?
        .into_result()?;
    Ok(outcome.json()?)
}

fn joined_logs(lines: Vec<&str>) -> String {
    lines.join("\n")
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn spend_over_cap_rejected() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let id = open_game(&arena, &arena.creator).await?;
    let alice = arena.worker.dev_create_account().await?;
    arena
        .registry
        .as_account()
        .call(arena.heist.id(), "import_agent")
        .args_json(json!({"agent": alice.id(), "revoked": false}))
        .transact()
        .await?
        .into_result()?;
    alice
        .call(arena.heist.id(), "join")
        .args_json(json!({"game_id": id}))
        .transact()
        .await?
        .into_result()?;
    let over = alice
        .call(arena.heist.id(), "act")
        .args_json(json!({
            "game_id": id,
            "action": "scout",
            "target": null,
            "stake_yocto": (SCOUT * 100).to_string(),
        }))
        .gas(near_workspaces::types::Gas::from_tgas(200))
        .transact()
        .await?;
    let text = format!("{over:?}");
    assert!(
        text.contains("over cap") || over.is_failure(),
        "over-cap act must fail: {text}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn revoked_agent_cannot_act() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let id = open_game(&arena, &arena.creator).await?;
    let alice = arena.worker.dev_create_account().await?;
    arena
        .registry
        .as_account()
        .call(arena.heist.id(), "import_agent")
        .args_json(json!({"agent": alice.id(), "revoked": true}))
        .transact()
        .await?
        .into_result()?;
    let joined = alice
        .call(arena.heist.id(), "join")
        .args_json(json!({"game_id": id}))
        .transact()
        .await?;
    assert!(
        joined.is_failure(),
        "revoked agent must not join: {joined:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn function_call_key_cannot_call_methods_outside_its_scope() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let secret = SecretKey::from_random(KeyType::ED25519);
    let registered = arena
        .creator
        .call(arena.registry.id(), "register_agent")
        .args_json(json!({
            "name": "agent",
            "public_key": secret.public_key().to_string(),
        }))
        .deposit(NearToken::from_millinear(100))
        .gas(near_workspaces::types::Gas::from_tgas(200))
        .transact()
        .await?
        .into_result()?;
    let sub: near_workspaces::AccountId = registered.json()?;
    let agent = Account::from_secret_key(sub, secret, &arena.worker);
    let keys = agent.view_access_keys().await?;
    assert_eq!(keys.len(), 1, "the subaccount has exactly one key");
    let permission = format!("{:?}", keys[0].access_key.permission);
    assert!(
        permission.contains("FunctionCall") || permission.contains("function_call"),
        "expected a function-call key, got {permission}"
    );
    assert!(
        !permission.to_ascii_lowercase().contains("fullaccess")
            && !permission.contains("FullAccess"),
        "full-access key is forbidden: {permission}"
    );
    assert!(permission.contains("act"), "{permission}");
    assert!(permission.contains("bid"), "{permission}");
    assert!(permission.contains("transfer_item"), "{permission}");
    for forbidden in [
        "resolve",
        "delist_game",
        "set_platform_caps",
        "join",
        "revoke",
    ] {
        assert!(
            !permission.contains(forbidden),
            "{forbidden} must not be on the key: {permission}"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn game_cannot_raise_platform_caps() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let raised = arena
        .heist
        .as_account()
        .call(arena.registry.id(), "set_platform_caps")
        .args_json(json!({
            "caps": {
                "max_players": 9,
                "max_duration_seconds": 86400,
                "max_round_count": 8,
                "max_pot_yocto": "50000000000000000000000000",
                "max_per_agent_yocto": "5000000000000000000000000",
                "max_per_action_yocto": "1000000000000000000000000"
            }
        }))
        .transact()
        .await?;
    assert!(
        raised.is_failure(),
        "a game contract cannot raise caps: {raised:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn only_the_moderator_can_delist() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let id = open_game(&arena, &arena.creator).await?;
    let denied = arena
        .creator
        .call(arena.factory.id(), "delist_game")
        .args_json(json!({"game_id": id}))
        .gas(near_workspaces::types::Gas::from_tgas(150))
        .transact()
        .await?;
    assert!(
        format!("{denied:?}").contains("only the moderator can delist") || denied.is_failure(),
        "{denied:?}"
    );
    let allowed = arena
        .moderator
        .call(arena.factory.id(), "delist_game")
        .args_json(json!({"game_id": id}))
        .gas(near_workspaces::types::Gas::from_tgas(150))
        .transact()
        .await?
        .into_result()?;
    let text = joined_logs(allowed.logs());
    assert!(text.contains("game_delisted"), "{text}");
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn cross_contract_spend_reverts_when_registry_check_fails() -> anyhow::Result<()> {
    let arena = deploy(false).await?;
    let id = open_game(&arena, &arena.creator).await?;
    let alice = arena.worker.dev_create_account().await?;
    arena
        .owner
        .call(arena.heist.id(), "import_agent")
        .args_json(json!({"agent": alice.id(), "revoked": false}))
        .transact()
        .await?
        .into_result()?;
    alice
        .call(arena.heist.id(), "join")
        .args_json(json!({"game_id": id}))
        .transact()
        .await?
        .into_result()?;
    let acted = alice
        .call(arena.heist.id(), "act")
        .args_json(json!({
            "game_id": id,
            "action": "scout",
            "target": null,
            "stake_yocto": SCOUT.to_string(),
        }))
        .gas(near_workspaces::types::Gas::from_tgas(250))
        .transact()
        .await?
        .into_result()?;
    let text = joined_logs(acted.logs());
    assert!(
        text.contains("spend_reverted"),
        "callback must revert: {text}"
    );
    let count: u32 = arena
        .heist
        .view("action_count")
        .args_json(json!({"game_id": id}))
        .await?
        .json()?;
    assert_eq!(count, 0, "reverted action must not remain");
    Ok(())
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn cross_contract_spend_sticks_when_registry_accepts() -> anyhow::Result<()> {
    let arena = deploy(true).await?;
    let id = open_game(&arena, &arena.creator).await?;
    let secret = SecretKey::from_random(KeyType::ED25519);
    let registered = arena
        .creator
        .call(arena.registry.id(), "register_agent")
        .args_json(json!({
            "name": "scout",
            "public_key": secret.public_key().to_string(),
        }))
        .deposit(NearToken::from_near(2))
        .gas(near_workspaces::types::Gas::from_tgas(200))
        .transact()
        .await?
        .into_result()?;
    let sub: near_workspaces::AccountId = registered.json()?;
    let alice = Account::from_secret_key(sub, secret, &arena.worker);
    let acted = alice
        .call(arena.heist.id(), "act")
        .args_json(json!({
            "game_id": id,
            "action": "scout",
            "target": null,
            "stake_yocto": SCOUT.to_string(),
        }))
        .gas(near_workspaces::types::Gas::from_tgas(250))
        .transact()
        .await?
        .into_result()?;
    let text = joined_logs(acted.logs());
    assert!(!text.contains("spend_reverted"), "{text}");
    assert!(text.contains("action_taken"), "{text}");
    let count: u32 = arena
        .heist
        .view("action_count")
        .args_json(json!({"game_id": id}))
        .await?
        .json()?;
    assert_eq!(count, 1);
    Ok(())
}

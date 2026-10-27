//! Skin forge on a sandbox node: mint, transfer, approve, payout, bind.
//! Build the wasm first:
//! `cargo build --release --target wasm32-unknown-unknown -p skin-forge`
//! Then `cargo test -p fe2vi-sandbox --features sandbox --test skins -- --ignored`.

use anyhow::Context;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, Contract, Worker};
use serde_json::json;

fn wasm(name: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!(
        "{}/../../target/wasm32-unknown-unknown/release/{name}.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).with_context(|| format!("missing {path}; build the release wasm first"))
}

struct Forge {
    worker: Worker<near_workspaces::network::Sandbox>,
    forge: Contract,
    registry: Account,
    owner: Account,
    treasury: Account,
}

async fn boot() -> anyhow::Result<Forge> {
    let worker = near_workspaces::sandbox().await?;
    let forge = worker.dev_deploy(&wasm("skin_forge")?).await?;
    let owner = worker.dev_create_account().await?;
    let registry = worker.dev_create_account().await?;
    let treasury = worker.dev_create_account().await?;
    forge
        .call("new")
        .args_json(json!({
            "owner": owner.id(),
            "treasury": treasury.id(),
            "registry": registry.id(),
        }))
        .transact()
        .await?
        .into_result()?;
    Ok(Forge {
        worker,
        forge,
        registry,
        owner,
        treasury,
    })
}

fn traits() -> serde_json::Value {
    json!({
        "paint": 1,
        "screen": 0,
        "eyes": 2,
        "headwear": 0,
        "face_item": 0,
        "back_item": 1,
        "emblem": 2,
        "trail": 0,
        "role": 0
    })
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn mint_transfer_approve_payout_and_bind() -> anyhow::Result<()> {
    let forge = boot().await?;
    let agent = forge.worker.dev_create_account().await?;
    let minted = forge
        .registry
        .call(forge.forge.id(), "mint_skin")
        .args_json(json!({ "agent": agent.id(), "traits": traits() }))
        .deposit(NearToken::from_near(1))
        .gas(near_workspaces::types::Gas::from_tgas(80))
        .transact()
        .await?
        .into_result()?;
    let token_id: String = minted.json()?;
    assert_eq!(token_id, "1");

    let worn: Option<String> = forge
        .forge
        .view("equipped_skin")
        .args_json(json!({ "agent": agent.id() }))
        .await?
        .json()?;
    assert_eq!(worn.as_deref(), Some("1"));

    let bob = forge.worker.dev_create_account().await?;
    let moved = agent
        .call(forge.forge.id(), "nft_transfer")
        .args_json(json!({
            "receiver_id": bob.id(),
            "token_id": "1",
            "approval_id": null,
            "memo": "desk"
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;
    let logs = format!("{:?}", moved.logs());
    assert!(logs.contains("nft_transfer"), "{logs}");
    assert!(logs.contains("skin_unbound"), "{logs}");

    let cleared: Option<String> = forge
        .forge
        .view("equipped_skin")
        .args_json(json!({ "agent": agent.id() }))
        .await?
        .json()?;
    assert!(cleared.is_none());

    let market = forge.worker.dev_create_account().await?;
    bob.call(forge.forge.id(), "nft_approve")
        .args_json(json!({ "token_id": "1", "account_id": market.id(), "msg": null }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;

    let buyer = forge.worker.dev_create_account().await?;
    let payout: serde_json::Value = market
        .call(forge.forge.id(), "nft_transfer_payout")
        .args_json(json!({
            "receiver_id": buyer.id(),
            "token_id": "1",
            "approval_id": 1,
            "memo": null,
            "balance": "100",
            "max_len_payout": 3
        }))
        .deposit(NearToken::from_yoctonear(1))
        .gas(near_workspaces::types::Gas::from_tgas(80))
        .transact()
        .await?
        .into_result()?
        .json()?;
    assert_eq!(payout["payout"][agent.id().as_str()], "2");
    assert_eq!(payout["payout"][forge.treasury.id().as_str()], "3");
    assert_eq!(payout["payout"][bob.id().as_str()], "95");

    let wearer = forge.worker.dev_create_account().await?;
    buyer
        .call(forge.forge.id(), "bind_skin")
        .args_json(json!({ "token_id": "1", "agent_id": wearer.id() }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;
    let bound: Option<String> = forge
        .forge
        .view("token_agent")
        .args_json(json!({ "token_id": "1" }))
        .await?
        .json()?;
    assert_eq!(bound.as_deref(), Some(wearer.id().as_str()));

    let arena = forge.worker.dev_create_account().await?;
    forge
        .owner
        .call(forge.forge.id(), "set_recorder")
        .args_json(json!({ "account_id": arena.id(), "allowed": true }))
        .transact()
        .await?
        .into_result()?;
    arena
        .call(forge.forge.id(), "record_result")
        .args_json(json!({
            "token_id": "1",
            "game": "heist-night",
            "outcome": "win",
            "season": 1
        }))
        .transact()
        .await?
        .into_result()?;
    let stats: serde_json::Value = forge
        .forge
        .view("skin_stats")
        .args_json(json!({ "token_id": "1" }))
        .await?
        .json()?;
    assert_eq!(stats["wins"], 1);
    let traits: serde_json::Value = forge
        .forge
        .view("unlocked_traits")
        .args_json(json!({ "token_id": "1" }))
        .await?
        .json()?;
    let gold = traits
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"] == "gold-leaf"))
        .context("gold-leaf row")?;
    assert_eq!(gold["unlocked"], false);
    assert_eq!(gold["progress"], 1);
    assert_eq!(gold["goal"], 10);
    Ok(())
}

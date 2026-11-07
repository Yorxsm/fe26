//! Mainnet CLI. Signing keys are read from the environment, never from the
//! repository. With no key, every command prints the transaction it would
//! submit and does not broadcast.

mod tx;

use std::thread;
use std::time::Duration;

use base64::Engine;
use clap::{Parser, Subcommand};
use fe2vi_core::{default_platform_caps, MIN_AGENT_DEPOSIT_YOCTO, ONE_NEAR};
use fe2vi_intents::{issue_session, live_gate, policy_from_caps, MOCK_USD_PER_NEAR_CENTS};

use crate::tx::SignedCall;

const DEFAULT_RPC: &str = "https://rpc.testnet.fastnear.com";
const DEFAULT_PASSPORT: &str = "passport.fe2vidisp7a3c.testnet";
const DEFAULT_PASSPORT_MARKET: &str = "market.fe2vidisp7a3c.testnet";
const PASSPORT_MINT_DEPOSIT_YOCTO: u128 = ONE_NEAR / 50;

#[derive(Parser, Debug)]
#[command(
    name = "fe2vi",
    about = "fe2VI arena tools for the NEAR testnet passport economy"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum IntentsCommand {
    /// Print the platform-cap policy. Broadcast stays off.
    Policy,
    /// Issue a mock session grant. The token is discarded.
    Grant {
        #[arg(long)]
        game: String,
        #[arg(long)]
        agent: String,
    },
}

#[derive(Subcommand, Debug)]
enum PassportCommand {
    /// Registry mints a passport and binds it to an agent id.
    Mint {
        #[arg(long)]
        token_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Registry binds an unbound passport to an agent id.
    Bind {
        #[arg(long)]
        token_id: String,
        #[arg(long)]
        agent_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Owner unbinds the agent from their passport.
    Unbind {
        #[arg(long)]
        token_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Claim mint/bind jobs from the API and broadcast them as the registry.
    Relay {
        #[arg(long)]
        once: bool,
    },
    /// Mint launch stock to one owner and list each token on the market.
    Seed {
        #[arg(long, default_value_t = 100)]
        count: u32,
        /// Price in NEAR. Defaults to 1 NEAR.
        #[arg(long, default_value = "1")]
        price: String,
        /// Owner and creator of every launch passport. Defaults to FE2VI_ACCOUNT_ID.
        #[arg(long)]
        owner: Option<String>,
        #[arg(long, default_value = "fdr")]
        prefix: String,
        #[arg(long, default_value_t = 1)]
        start: u32,
        #[arg(long)]
        mint_only: bool,
        #[arg(long)]
        list_only: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Approve the market to list one passport at a fixed NEAR price.
    List {
        #[arg(long)]
        token_id: String,
        /// Price in NEAR. Defaults to 1 NEAR.
        #[arg(long, default_value = "1")]
        price: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Cancel a market listing. The signer must be the seller.
    Cancel {
        #[arg(long)]
        token_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Buy a listed passport from the market. The signer is the buyer.
    Buy {
        #[arg(long)]
        token_id: String,
        /// Price in NEAR. Defaults to 1 NEAR.
        #[arg(long, default_value = "1")]
        price: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create an agent subaccount whose only key can call act, bid, and transfer_item.
    RegisterAgent {
        #[arg(long)]
        name: String,
        /// Agent public key, `ed25519:...`. This is not the signer secret.
        #[arg(long)]
        public_key: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Revoke an agent. The owner or the original registrant must sign.
    Revoke {
        #[arg(long)]
        agent: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Mint, bind, or unbind a passport. The signer must be the registry or owner.
    Passport {
        #[command(subcommand)]
        action: PassportCommand,
    },
    /// Show the Intents policy sketch. Does not call the network or print a secret.
    Intents {
        #[command(subcommand)]
        action: IntentsCommand,
    },
    /// Submit factory settle_vault for one finished Vault match, then ask the API to read the receipt.
    /// The signer is FE2VI_SETTLER_SECRET_KEY, not the passport bind key. Without it, this prints the call and does not broadcast.
    SettleVault {
        #[arg(long)]
        match_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Play several Vault matches back to back, then settle each with the restricted settler.
    /// Agent tokens and the settler secret stay in the environment and are not printed.
    VaultLoop {
        #[arg(long, default_value_t = 2)]
        matches: u32,
        #[arg(long)]
        dry_run: bool,
    },
    /// Poll the spectator API, or a heist view if no API is configured.
    Watch {
        #[arg(long)]
        game_id: String,
        /// Print one snapshot and exit.
        #[arg(long)]
        once: bool,
    },
}

#[derive(Clone)]
struct Config {
    rpc_url: String,
    account_id: Option<String>,
    signer_secret: Option<String>,
    registry_id: String,
    heist_id: String,
    passport_id: String,
    passport_market_id: String,
    api_url: Option<String>,
    relay_key: Option<String>,
}

fn redact_line(text: &str) -> String {
    let mut redacted = text.to_string();
    for name in [
        "FE2VI_SIGNER_SECRET_KEY",
        "FE2VI_PASSPORT_RELAY_KEY",
        "FE2VI_SETTLER_SECRET_KEY",
        "FE2VI_SETTLER_KEY",
    ] {
        if let Ok(value) = std::env::var(name) {
            if value.len() >= 8 {
                redacted = redacted.replace(&value, "[redacted]");
            }
        }
    }
    let mut out = String::new();
    let mut rest = redacted.as_str();
    while let Some(index) = rest.find("ed25519:") {
        out.push_str(&rest[..index]);
        out.push_str("ed25519:[redacted]");
        let after = &rest[index + "ed25519:".len()..];
        let end = after
            .char_indices()
            .find(|(_, ch)| !ch.is_ascii_alphanumeric())
            .map(|(offset, _)| offset)
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn main() {
    let cli = Cli::parse();
    match run(&config_from_env(), cli.command) {
        Ok(output) => println!("{}", redact_line(&output)),
        Err(err) => {
            eprintln!("error: {}", redact_line(&err));
            std::process::exit(1);
        }
    }
}

fn config_from_env() -> Config {
    Config {
        rpc_url: std::env::var("FE2VI_RPC_URL").unwrap_or_else(|_| DEFAULT_RPC.to_string()),
        account_id: std::env::var("FE2VI_ACCOUNT_ID")
            .ok()
            .filter(|value| !value.is_empty()),
        signer_secret: std::env::var("FE2VI_SIGNER_SECRET_KEY")
            .ok()
            .filter(|value| !value.is_empty()),
        registry_id: std::env::var("FE2VI_REGISTRY_ID")
            .unwrap_or_else(|_| "registry.fe2vidisp7a3c.testnet".into()),
        heist_id: std::env::var("FE2VI_HEIST_ID")
            .unwrap_or_else(|_| "heist.fe2vidisp7a3c.testnet".into()),
        passport_id: std::env::var("FE2VI_PASSPORT_ID")
            .unwrap_or_else(|_| DEFAULT_PASSPORT.to_string()),
        passport_market_id: std::env::var("FE2VI_PASSPORT_MARKET_ID")
            .unwrap_or_else(|_| DEFAULT_PASSPORT_MARKET.to_string()),
        api_url: std::env::var("FE2VI_API_URL")
            .ok()
            .filter(|value| !value.is_empty()),
        relay_key: std::env::var("FE2VI_PASSPORT_RELAY_KEY")
            .ok()
            .filter(|value| !value.is_empty()),
    }
}

fn intents(action: IntentsCommand) -> Result<String, String> {
    let live = live_gate(
        &std::env::var("FE2VI_INTENTS_LIVE").unwrap_or_default(),
        &std::env::var("NEAR_INTENTS_AGENT_API_KEY").unwrap_or_default(),
    );
    match action {
        IntentsCommand::Policy => {
            let policy = policy_from_caps(&default_platform_caps(), MOCK_USD_PER_NEAR_CENTS);
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "mode": if live { "live-dry" } else { "mock" },
                "broadcast": false,
                "note": "fe2VI does not broadcast swaps, transfers, or withdrawals",
                "policy": policy,
            }))
            .unwrap_or_else(|_| "{}".into()))
        }
        IntentsCommand::Grant { game, agent } => {
            let mut grants = Vec::new();
            let issued = issue_session(&mut grants, &game, &agent, 0, 3600);
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "mode": issued.mode,
                "grant_id": issued.grant_id,
                "label": issued.label,
                "commitment": issued.commitment,
                "token": "withheld",
                "broadcast": false,
            }))
            .unwrap_or_else(|_| "{}".into()))
        }
    }
}

fn assert_chain(rpc_url: &str) -> Result<(), String> {
    let url = rpc_url.to_ascii_lowercase();
    if url.contains("mainnet") {
        return Err("refusing mainnet RPC; passport, market, and escrow are on testnet".into());
    }
    if !url.contains("testnet") {
        return Err("chain RPC must be a testnet endpoint".into());
    }
    Ok(())
}

fn require_account<'a>(id: &'a str, name: &str) -> Result<&'a str, String> {
    if id.trim().is_empty() {
        return Err(format!(
            "{name} is not configured. factory, registry, and heist are not deployed, so this command stays closed"
        ));
    }
    Ok(id)
}

fn near_to_yocto(value: &str) -> Result<u128, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') {
        return Err("price must be a positive NEAR amount".into());
    }
    let (whole, frac) = match value.split_once('.') {
        Some((whole, frac)) => (whole, frac),
        None => (value, ""),
    };
    if frac.len() > 24
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
    {
        return Err("price must be a decimal NEAR amount".into());
    }
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| "price overflow")?
    };
    let mut padded = frac.to_string();
    while padded.len() < 24 {
        padded.push('0');
    }
    let frac: u128 = if padded.is_empty() {
        0
    } else {
        padded.parse().map_err(|_| "price overflow")?
    };
    whole
        .checked_mul(ONE_NEAR)
        .and_then(|v| v.checked_add(frac))
        .filter(|v| *v > 0)
        .ok_or_else(|| "price overflow".to_string())
}

fn run(config: &Config, command: Command) -> Result<String, String> {
    assert_chain(&config.rpc_url)?;
    match command {
        Command::RegisterAgent {
            name,
            public_key,
            dry_run,
        } => {
            let args = serde_json::json!({ "name": name, "public_key": public_key });
            submit(
                config,
                dry_run,
                require_account(&config.registry_id, "FE2VI_REGISTRY_ID")?,
                "register_agent",
                &args,
                MIN_AGENT_DEPOSIT_YOCTO,
                80_000_000_000_000,
            )
        }
        Command::Revoke { agent, dry_run } => {
            let args = serde_json::json!({ "agent": agent });
            submit(
                config,
                dry_run,
                require_account(&config.registry_id, "FE2VI_REGISTRY_ID")?,
                "revoke",
                &args,
                0,
                50_000_000_000_000,
            )
        }
        Command::Passport { action } => match action {
            PassportCommand::Mint {
                token_id,
                owner,
                agent_id,
                dry_run,
            } => {
                let args = serde_json::json!({
                    "token_id": token_id,
                    "owner": owner,
                    "agent_id": agent_id,
                });
                submit(
                    config,
                    dry_run,
                    &config.passport_id,
                    "mint",
                    &args,
                    PASSPORT_MINT_DEPOSIT_YOCTO,
                    80_000_000_000_000,
                )
            }
            PassportCommand::Bind {
                token_id,
                agent_id,
                dry_run,
            } => {
                let args = serde_json::json!({ "token_id": token_id, "agent_id": agent_id });
                submit(
                    config,
                    dry_run,
                    &config.passport_id,
                    "bind",
                    &args,
                    0,
                    50_000_000_000_000,
                )
            }
            PassportCommand::Unbind { token_id, dry_run } => {
                let args = serde_json::json!({ "token_id": token_id });
                submit(
                    config,
                    dry_run,
                    &config.passport_id,
                    "unbind",
                    &args,
                    0,
                    50_000_000_000_000,
                )
            }
            PassportCommand::Seed {
                count,
                price,
                owner,
                prefix,
                start,
                mint_only,
                list_only,
                dry_run,
            } => seed(
                config,
                count,
                &price,
                owner.as_deref(),
                &prefix,
                start,
                mint_only,
                list_only,
                dry_run,
            ),
            PassportCommand::List {
                token_id,
                price,
                dry_run,
            } => {
                let price = near_to_yocto(&price)?;
                let msg = serde_json::to_string(&serde_json::json!({
                    "market_type": "sale",
                    "price": price.to_string(),
                    "ft_token_id": "near",
                }))
                .map_err(|err| err.to_string())?;
                submit(
                    config,
                    dry_run,
                    &config.passport_id,
                    "nft_approve",
                    &serde_json::json!({
                        "token_id": token_id,
                        "account_id": config.passport_market_id,
                        "msg": msg,
                    }),
                    PASSPORT_MINT_DEPOSIT_YOCTO,
                    50_000_000_000_000,
                )
            }
            PassportCommand::Cancel { token_id, dry_run } => submit(
                config,
                dry_run,
                &config.passport_market_id,
                "cancel",
                &serde_json::json!({ "token_id": token_id }),
                0,
                30_000_000_000_000,
            ),
            PassportCommand::Buy {
                token_id,
                price,
                dry_run,
            } => {
                let price = near_to_yocto(&price)?;
                submit(
                    config,
                    dry_run,
                    &config.passport_market_id,
                    "buy",
                    &serde_json::json!({ "token_id": token_id }),
                    price,
                    80_000_000_000_000,
                )
            }
            PassportCommand::Relay { once } => relay(config, once),
        },
        Command::Intents { action } => intents(action),
        Command::SettleVault { match_id, dry_run } => settle_vault(config, &match_id, dry_run),
        Command::VaultLoop { matches, dry_run } => vault_loop(config, matches, dry_run),
        Command::Watch { game_id, once } => watch(config, &game_id, once),
    }
}

#[allow(clippy::too_many_arguments)]
fn seed(
    config: &Config,
    count: u32,
    price: &str,
    owner: Option<&str>,
    prefix: &str,
    start: u32,
    mint_only: bool,
    list_only: bool,
    dry_run: bool,
) -> Result<String, String> {
    if mint_only && list_only {
        return Err("--mint-only and --list-only are mutually exclusive".into());
    }
    if count == 0 {
        return Err("count must be positive".into());
    }
    if !dry_run {
        config
            .signer_secret
            .as_deref()
            .ok_or("FE2VI_SIGNER_SECRET_KEY is required to broadcast")?;
        config
            .account_id
            .as_deref()
            .ok_or("FE2VI_ACCOUNT_ID is required to broadcast")?;
    }
    let owner = owner
        .map(str::to_string)
        .or_else(|| config.account_id.clone())
        .ok_or("--owner or FE2VI_ACCOUNT_ID is required")?;
    let price_yocto = near_to_yocto(price)?;
    let ids: Vec<String> = (0..count)
        .map(|offset| format!("{prefix}_{:03}", start.saturating_add(offset)))
        .collect();
    let mint_batches = ids.chunks(25).count();
    let total_deposit = PASSPORT_MINT_DEPOSIT_YOCTO.saturating_mul(u128::from(count));
    if dry_run {
        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "network": "testnet",
            "dry_run": true,
            "passport_id": config.passport_id,
            "market_id": config.passport_market_id,
            "owner": owner,
            "count": count,
            "price_near": price,
            "price_yocto": price_yocto.to_string(),
            "token_ids": if count <= 10 { serde_json::json!(ids) } else {
                serde_json::json!({ "first": ids.first(), "last": ids.last() })
            },
            "mint_batches": if list_only { 0 } else { mint_batches },
            "mint_deposit_yocto": total_deposit.to_string(),
            "listing_deposit_yocto": if mint_only {
                "0".to_string()
            } else {
                PASSPORT_MINT_DEPOSIT_YOCTO
                    .saturating_mul(u128::from(count))
                    .to_string()
            },
            "approvals": if mint_only { 0 } else { count },
        }))
        .unwrap_or_else(|_| "{}".into()));
    }
    let mut minted = 0u32;
    let mut listed = 0u32;
    if !list_only {
        for chunk in ids.chunks(25) {
            let deposit = PASSPORT_MINT_DEPOSIT_YOCTO.saturating_mul(chunk.len() as u128);
            let args = serde_json::json!({
                "token_ids": chunk,
                "owner": owner,
            });
            submit(
                config,
                false,
                &config.passport_id,
                "mint_unbound_batch",
                &args,
                deposit,
                80_000_000_000_000,
            )?;
            minted = minted.saturating_add(chunk.len() as u32);
        }
    }
    if !mint_only {
        let msg = serde_json::to_string(&serde_json::json!({
            "market_type": "sale",
            "price": price_yocto.to_string(),
            "ft_token_id": "near",
        }))
        .map_err(|err| err.to_string())?;
        for token_id in &ids {
            let args = serde_json::json!({
                "token_id": token_id,
                "account_id": config.passport_market_id,
                "msg": msg,
            });
            submit(
                config,
                false,
                &config.passport_id,
                "nft_approve",
                &args,
                PASSPORT_MINT_DEPOSIT_YOCTO,
                50_000_000_000_000,
            )?;
            listed = listed.saturating_add(1);
        }
    }
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "network": "testnet",
        "broadcast": true,
        "owner": owner,
        "minted": minted,
        "listed": listed,
        "price_near": price,
        "price_yocto": price_yocto.to_string(),
        "market_id": config.passport_market_id,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

fn vault_result_ready(rule: &str, outcome: &str) -> bool {
    matches!(
        (rule, outcome),
        ("winner-takes-all", "win") | ("equal-split", "draw") | ("creator-refund", "cancel")
    )
}

fn vault_loop(config: &Config, matches: u32, dry_run: bool) -> Result<String, String> {
    let matches = matches.clamp(1, 5);
    let tokens_ready =
        std::env::var("FE2VI_LOOP_TOKEN_A").is_ok() && std::env::var("FE2VI_LOOP_TOKEN_B").is_ok();
    if dry_run || !tokens_ready {
        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "broadcast": false,
            "matches": matches,
            "reason": if dry_run { "dry-run" } else { "FE2VI_LOOP_TOKEN_A and FE2VI_LOOP_TOKEN_B are unset" },
            "steps": [
                "queue both agents on /v1/arenas/vault/queue",
                "submit cover until the public match is over",
                "settle_vault with FE2VI_SETTLER_SECRET_KEY",
                "record the FINAL receipt",
                "repeat"
            ],
            "secrets": "not printed",
        }))
        .unwrap_or_else(|_| "{}".into()));
    }
    let api = config
        .api_url
        .as_deref()
        .ok_or("FE2VI_API_URL is required")?
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let token_a = std::env::var("FE2VI_LOOP_TOKEN_A").map_err(|err| err.to_string())?;
    let token_b = std::env::var("FE2VI_LOOP_TOKEN_B").map_err(|err| err.to_string())?;
    let mut played = Vec::new();
    for index in 0..matches {
        let queued_a = bearer_post(
            &client,
            &format!("{api}/v1/arenas/vault/queue"),
            &token_a,
            &serde_json::json!({}),
        )?;
        let queued_b = bearer_post(
            &client,
            &format!("{api}/v1/arenas/vault/queue"),
            &token_b,
            &serde_json::json!({}),
        )?;
        let match_id = queued_b
            .pointer("/match/matchId")
            .or_else(|| queued_a.pointer("/match/matchId"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let match_id = match match_id {
            Some(id) => id,
            None => poll_match_id(&client, &api, &token_a)?,
        };
        let mut spins = 0_u32;
        loop {
            let mine = bearer_get(&client, &format!("{api}/v1/agents/me/matches"), &token_a)?;
            let row = mine["matches"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["match"]["matchId"] == match_id))
                .cloned()
                .ok_or("loop match disappeared")?;
            let status = row["match"]["status"].as_str().unwrap_or("");
            if status == "over" {
                break;
            }
            let turn = row["match"]["turn"].as_u64().unwrap_or(1);
            let body = serde_json::json!({
                "turn": turn,
                "action": {"type": "cover", "intent": "vault loop"},
                "clientRequestId": format!("loop-{index}-{turn}-a"),
            });
            let _ = bearer_post(
                &client,
                &format!("{api}/v1/matches/{match_id}/actions"),
                &token_a,
                &body,
            );
            let body_b = serde_json::json!({
                "turn": turn,
                "action": {"type": "cover", "intent": "vault loop"},
                "clientRequestId": format!("loop-{index}-{turn}-b"),
            });
            let _ = bearer_post(
                &client,
                &format!("{api}/v1/matches/{match_id}/actions"),
                &token_b,
                &body_b,
            );
            spins += 1;
            if spins > 12 {
                return Err("vault loop did not reach a public result".into());
            }
            thread::sleep(Duration::from_secs(1));
        }
        let public = api_get(&client, &format!("{api}/v1/matches/{match_id}"))?;
        let receipt = settle_vault(config, &match_id, false)?;
        played.push(serde_json::json!({
            "matchId": match_id,
            "outcome": public.pointer("/vault/pot/outcome").cloned().unwrap_or(serde_json::Value::Null),
            "rule": public.pointer("/vault/pot/rule").cloned().unwrap_or(serde_json::Value::Null),
            "receipt": serde_json::from_str::<serde_json::Value>(&receipt).unwrap_or(serde_json::Value::String(receipt)),
        }));
    }
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "broadcast": true,
        "source": "settled-receipts",
        "matches": played,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

fn poll_match_id(
    client: &reqwest::blocking::Client,
    api: &str,
    token: &str,
) -> Result<String, String> {
    for _ in 0..5 {
        let mine = bearer_get(client, &format!("{api}/v1/agents/me/matches"), token)?;
        if let Some(id) = mine["matches"]
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row["match"]["matchId"].as_str())
        {
            return Ok(id.to_string());
        }
        thread::sleep(Duration::from_millis(300));
    }
    Err("no vault match was opened".into())
}

fn bearer_get(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
) -> Result<serde_json::Value, String> {
    let response = client
        .get(url)
        .header("authorization", format!("Bearer {token}"))
        .send()
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("api returned {status}"));
    }
    serde_json::from_str(&text).map_err(|err| format!("api body: {err}"))
}

fn bearer_post(
    client: &reqwest::blocking::Client,
    url: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = client
        .post(url)
        .header("authorization", format!("Bearer {token}"))
        .json(body)
        .send()
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("api returned {status}"));
    }
    Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod cli_tests {
    use super::vault_result_ready;

    #[test]
    fn a_draw_is_ready_for_settle_vault() {
        assert!(vault_result_ready("winner-takes-all", "win"));
        assert!(vault_result_ready("equal-split", "draw"));
        assert!(vault_result_ready("creator-refund", "cancel"));
        assert!(!vault_result_ready("winner-takes-all", "draw"));
        assert!(!vault_result_ready("equal-split", "unmapped"));
    }
}

fn settle_vault(config: &Config, match_id: &str, dry_run: bool) -> Result<String, String> {
    let api = config
        .api_url
        .as_deref()
        .ok_or("FE2VI_API_URL is required")?
        .trim_end_matches('/')
        .to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let match_body = api_get(&client, &format!("{api}/v1/matches/{match_id}"))?;
    let pot = match_body
        .pointer("/vault/pot")
        .cloned()
        .ok_or("match has no vault result")?;
    let outcome = pot["outcome"].as_str().unwrap_or("");
    let rule = pot["rule"].as_str().unwrap_or("");
    if pot["settled"].as_bool().unwrap_or(false) {
        return Ok(serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "broadcast": false,
            "settled": true,
            "txHash": pot["txHash"],
            "reason": "already settled",
        }))
        .unwrap_or_else(|_| "{}".into()));
    }
    if !vault_result_ready(rule, outcome) {
        return Err("match is unmapped or not a vault result; no account was invented".into());
    }
    let winner = pot["winner"].as_str().unwrap_or("");
    if outcome == "win" && winner.is_empty() {
        return Err("win has no linked account".into());
    }
    let factory = std::env::var("FE2VI_FACTORY_ID")
        .unwrap_or_else(|_| "factory.fe2vidisp7a3c.testnet".into());
    let args = serde_json::json!({
        "game_id": match_id,
        "outcome": outcome,
        "winner": if outcome == "win" { serde_json::Value::String(winner.into()) } else { serde_json::Value::Null },
    });
    let secret = std::env::var("FE2VI_SETTLER_SECRET_KEY").ok();
    let mut settling = config.clone();
    settling.signer_secret = secret.clone();
    let submitted = submit(
        &settling,
        dry_run || secret.is_none(),
        &factory,
        "settle_vault",
        &args,
        0,
        300_000_000_000_000,
    )?;
    let parsed: serde_json::Value =
        serde_json::from_str(&submitted).unwrap_or(serde_json::Value::Null);
    let broadcast = parsed["broadcast"].as_bool().unwrap_or(false);
    if !broadcast {
        return Ok(submitted);
    }
    let tx_hash = parsed
        .pointer("/result/transaction/hash")
        .or_else(|| parsed.pointer("/result/transaction_outcome/id"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let bearer = std::env::var("FE2VI_SETTLER_KEY")
        .map_err(|_| "FE2VI_SETTLER_KEY is required to record the receipt")?;
    let account = settling.account_id.clone().unwrap_or_default();
    let recorded = api_post(
        &client,
        &format!("{api}/v1/internal/vault/matches/{match_id}/receipt"),
        &bearer,
        &serde_json::json!({ "txHash": tx_hash, "senderAccountId": account }),
    )?;
    if recorded["settled"].as_bool() != Some(true) {
        return Err(format!("receipt was not accepted: {recorded}"));
    }
    Ok(serde_json::to_string_pretty(&recorded).unwrap_or_else(|_| "{}".into()))
}

fn api_get(client: &reqwest::blocking::Client, url: &str) -> Result<serde_json::Value, String> {
    let response = client.get(url).send().map_err(|err| err.to_string())?;
    let status = response.status();
    let text = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("api returned {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|err| format!("api body: {err}"))
}

fn relay(config: &Config, once: bool) -> Result<String, String> {
    let api = config
        .api_url
        .as_deref()
        .ok_or("FE2VI_API_URL is required to relay")?
        .trim_end_matches('/')
        .to_string();
    let key = config
        .relay_key
        .as_deref()
        .ok_or("FE2VI_PASSPORT_RELAY_KEY is required to relay")?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("http client: {}", redact_line(&err.to_string())))?;
    loop {
        let claimed = api_post(
            &client,
            &format!("{api}/v1/internal/passport/jobs/claim"),
            key,
            &serde_json::json!({ "limit": 10 }),
        )?;
        let jobs = claimed["jobs"].as_array().cloned().unwrap_or_default();
        if jobs.is_empty() {
            let summary = serde_json::json!({
                "ok": true,
                "processed": 0,
                "failed": 0,
            });
            if once {
                return Ok(serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".into()));
            }
            println!(
                "{}",
                serde_json::to_string(&summary).unwrap_or_else(|_| "{}".into())
            );
            thread::sleep(Duration::from_secs(5));
            continue;
        }
        let mut processed = 0_u64;
        let mut failed = 0_u64;
        for job in &jobs {
            let job_id = job["jobId"].as_str().unwrap_or_default();
            let kind = job["kind"].as_str().unwrap_or_default();
            let token_id = job["tokenId"].as_str().unwrap_or_default();
            let owner = job["ownerNear"].as_str().unwrap_or_default();
            let agent_id = job["agentId"].as_str().unwrap_or_default();
            let result = match kind {
                "mint" => submit(
                    config,
                    false,
                    &config.passport_id,
                    "mint",
                    &serde_json::json!({
                        "token_id": token_id,
                        "owner": owner,
                        "agent_id": agent_id,
                    }),
                    PASSPORT_MINT_DEPOSIT_YOCTO,
                    80_000_000_000_000,
                ),
                "bind" => bind_job(config, token_id, owner, agent_id),
                "unbind" => {
                    Err("unbind needs the owner's wallet signature, not the registry".into())
                }
                other => Err(format!("unknown passport job kind: {other}")),
            };
            let (status, tx_hash, tx_block_hash, error, token) = match result {
                Ok(output) => {
                    let receipt: serde_json::Value =
                        serde_json::from_str(&output).unwrap_or(serde_json::Value::Null);
                    let hash = receipt["result"]["transaction"]["hash"]
                        .as_str()
                        .map(str::to_string);
                    let block_hash = receipt_block_hash(&receipt["result"]);
                    if receipt["alreadyBound"].as_bool().unwrap_or(false) {
                        processed += 1;
                        ("done", None, None, None, token_id.to_string())
                    } else if receipt["broadcast"].as_bool().unwrap_or(false) {
                        if let Some(err) = execution_error(&receipt["result"]) {
                            failed += 1;
                            let detail = match &hash {
                                Some(hash) => format!("execution failed {hash}: {err}"),
                                None => format!("execution failed: {err}"),
                            };
                            ("failed", hash, None, Some(detail), token_id.to_string())
                        } else {
                            processed += 1;
                            ("done", hash, block_hash, None, token_id.to_string())
                        }
                    } else {
                        failed += 1;
                        (
                            "failed",
                            None,
                            None,
                            Some("relay did not broadcast".to_string()),
                            token_id.to_string(),
                        )
                    }
                }
                Err(err) => {
                    failed += 1;
                    ("failed", None, None, Some(err), token_id.to_string())
                }
            };
            let mut complete = serde_json::json!({
                "status": status,
                "tokenId": token,
            });
            if let Some(hash) = tx_hash {
                complete["txHash"] = serde_json::json!(hash);
            }
            if let Some(hash) = tx_block_hash {
                complete["txBlockHash"] = serde_json::json!(hash);
            }
            if status == "done" {
                if let Ok(view) = passport_state(config, &token) {
                    if let Some(seq) = view.get("last_sync_seq").and_then(|value| value.as_u64()) {
                        complete["lastSyncSeq"] = serde_json::json!(seq);
                    }
                    if let Some(career) = view.get("career") {
                        complete["career"] = career.clone();
                    }
                }
            }
            if let Some(error) = error {
                eprintln!("passport job {kind} {status}: {}", redact_line(&error));
                complete["error"] = serde_json::json!(redact_line(&error));
            }
            if let Err(err) = api_post(
                &client,
                &format!("{api}/v1/internal/passport/jobs/{job_id}/complete"),
                key,
                &complete,
            ) {
                eprintln!("complete {job_id} failed: {}", redact_line(&err));
            }
        }
        let summary = serde_json::json!({
            "ok": failed == 0,
            "processed": processed,
            "failed": failed,
        });
        if once {
            return Ok(serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".into()));
        }
        println!(
            "{}",
            serde_json::to_string(&summary).unwrap_or_else(|_| "{}".into())
        );
        thread::sleep(Duration::from_secs(5));
    }
}

fn api_post(
    client: &reqwest::blocking::Client,
    url: &str,
    key: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let response = client
        .post(url)
        .bearer_auth(key)
        .json(body)
        .send()
        .map_err(|err| format!("api request failed: {err}"))?;
    let status = response.status();
    let text = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("api returned {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|err| format!("api body: {err}"))
}

fn submit(
    config: &Config,
    dry_run: bool,
    receiver_id: &str,
    method: &str,
    args: &serde_json::Value,
    deposit: u128,
    gas: u64,
) -> Result<String, String> {
    let mut preview = serde_json::json!({
        "network": "testnet",
        "rpc_url": config.rpc_url,
        "account_id": config.account_id,
        "receiver_id": receiver_id,
        "method": method,
        "args": args,
        "deposit_yocto": deposit.to_string(),
        "gas": gas,
        "broadcast": false,
    });
    let secret = match config.signer_secret.as_deref() {
        Some(secret) if !dry_run => secret,
        _ => {
            preview["reason"] = if dry_run {
                serde_json::json!("dry-run")
            } else {
                serde_json::json!("FE2VI_SIGNER_SECRET_KEY is not set")
            };
            return Ok(serde_json::to_string_pretty(&preview).unwrap_or_else(|_| "{}".into()));
        }
    };
    let account = config
        .account_id
        .clone()
        .ok_or("FE2VI_ACCOUNT_ID is required to broadcast")?;
    let args_bytes = serde_json::to_vec(args).map_err(|err| err.to_string())?;
    let (nonce, block_hash) = chain_head(&config.rpc_url, &account, secret)?;
    let signed = tx::sign_call(&SignedCall {
        signer_id: account,
        secret,
        receiver_id: receiver_id.to_string(),
        method: method.to_string(),
        args: args_bytes,
        gas,
        deposit,
        nonce,
        block_hash,
    })?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&signed);
    let result = rpc(
        &config.rpc_url,
        "broadcast_tx_commit",
        serde_json::json!([encoded]),
    )?;
    preview["broadcast"] = serde_json::json!(true);
    preview["result"] = result;
    if let Some(map) = preview.as_object_mut() {
        map.remove("reason");
    }
    serde_json::to_string_pretty(&preview).map_err(|err| err.to_string())
}

fn chain_head(rpc_url: &str, account_id: &str, secret: &str) -> Result<(u64, String), String> {
    let secret_key: near_crypto::SecretKey = secret
        .parse()
        .map_err(|_| "signing key could not be parsed".to_string())?;
    let public_key = secret_key.public_key().to_string();
    let key = rpc(
        rpc_url,
        "query",
        serde_json::json!({
            "request_type": "view_access_key",
            "finality": "final",
            "account_id": account_id,
            "public_key": public_key,
        }),
    )?;
    let nonce = key["nonce"]
        .as_u64()
        .ok_or("access key nonce missing")?
        .saturating_add(1);
    let block = rpc(rpc_url, "block", serde_json::json!({"finality": "final"}))?;
    let hash = block["header"]["hash"]
        .as_str()
        .ok_or("block hash missing")?
        .to_string();
    Ok((nonce, hash))
}

fn bind_job(
    config: &Config,
    token_id: &str,
    owner: &str,
    agent_id: &str,
) -> Result<String, String> {
    let view = passport_state(config, token_id)?;
    let on_chain = view["owner_id"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "passport: missing owner_id".to_string())?;
    if on_chain != owner {
        return Err(format!(
            "owner mismatch for {token_id}: chain owner is {on_chain}, job owner is {owner}"
        ));
    }
    let bound = view["bound_agent"].as_str().unwrap_or("").trim();
    if bound == agent_id {
        return Ok(serde_json::json!({
            "network": "testnet",
            "broadcast": false,
            "alreadyBound": true,
        })
        .to_string());
    }
    if !bound.is_empty() {
        return Err(format!("token already bound to {bound}"));
    }
    match agent_token(config, agent_id)? {
        Some(other) if other != token_id => {
            return Err(format!("agent already bound to {other}"));
        }
        _ => {}
    }
    submit(
        config,
        false,
        &config.passport_id,
        "bind",
        &serde_json::json!({ "token_id": token_id, "agent_id": agent_id }),
        0,
        50_000_000_000_000,
    )
}

/// `broadcast_tx_commit` returns a JSON-RPC result for a transaction that was
/// included and then panicked. Inclusion is not success. `None` means the
/// final status is `SuccessValue` and no receipt failed.
fn receipt_block_hash(outcome: &serde_json::Value) -> Option<String> {
    outcome
        .get("receipts_outcome")
        .and_then(serde_json::Value::as_array)
        .and_then(|receipts| receipts.last())
        .and_then(|receipt| receipt.get("block_hash"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn execution_error(outcome: &serde_json::Value) -> Option<String> {
    let mut failures = Vec::new();
    collect_status_failure(&outcome["status"], &mut failures);
    if let Some(receipts) = outcome
        .get("receipts_outcome")
        .and_then(|value| value.as_array())
    {
        for receipt in receipts {
            collect_status_failure(&receipt["outcome"]["status"], &mut failures);
        }
    }
    if let Some(message) = failures.into_iter().next() {
        return Some(message);
    }
    if outcome
        .get("status")
        .and_then(|status| status.get("SuccessValue"))
        .is_some()
    {
        return None;
    }
    Some("execution did not succeed".to_string())
}

fn collect_status_failure(status: &serde_json::Value, failures: &mut Vec<String>) {
    if let Some(failure) = status.get("Failure") {
        failures.push(short_execution_text(failure));
    }
}

fn short_execution_text(failure: &serde_json::Value) -> String {
    let mut found = String::new();
    find_execution_error(failure, &mut found);
    if found.is_empty() {
        found = "failure".to_string();
    }
    let mut out = String::new();
    for ch in found.chars() {
        if out.len() >= 120 {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | ':') {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "failure".to_string()
    } else {
        out
    }
}

fn find_execution_error(value: &serde_json::Value, found: &mut String) {
    if !found.is_empty() {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("ExecutionError").and_then(|item| item.as_str()) {
                *found = text.to_string();
                return;
            }
            for child in map.values() {
                find_execution_error(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                find_execution_error(child, found);
            }
        }
        _ => {}
    }
}

fn agent_token(config: &Config, agent_id: &str) -> Result<Option<String>, String> {
    let value = contract_json(
        config,
        "agent_token",
        &serde_json::json!({ "agent_id": agent_id }),
    )?;
    match value {
        serde_json::Value::String(token) if !token.trim().is_empty() => Ok(Some(token)),
        serde_json::Value::Null => Ok(None),
        _ => Err("agent_token: unexpected result".into()),
    }
}

fn contract_json(
    config: &Config,
    method: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let args = serde_json::to_vec(args).map_err(|err| err.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(args);
    let response = rpc(
        &config.rpc_url,
        "query",
        serde_json::json!({
            "request_type": "call_function",
            "finality": "final",
            "account_id": config.passport_id,
            "method_name": method,
            "args_base64": encoded,
        }),
    )?;
    if let Some(err) = response.get("error").filter(|value| !value.is_null()) {
        return Err(format!("{method}: {err}"));
    }
    let bytes = response
        .get("result")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{method}: missing result"))?
        .iter()
        .map(|value| value.as_u64().unwrap_or_default() as u8)
        .collect::<Vec<u8>>();
    serde_json::from_slice(&bytes).map_err(|err| format!("{method} body: {err}"))
}

fn passport_state(config: &Config, token_id: &str) -> Result<serde_json::Value, String> {
    contract_json(
        config,
        "passport",
        &serde_json::json!({ "token_id": token_id }),
    )
}

fn rpc(url: &str, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("http client: {err}"))?;
    let response = client
        .post(url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "fe2vi",
            "method": method,
            "params": params,
        }))
        .send()
        .map_err(|err| format!("rpc transport failed: {err}"))?;
    let body: serde_json::Value = response.json().map_err(|err| format!("rpc body: {err}"))?;
    if let Some(err) = body.get("error") {
        return Err(format!("rpc error: {err}"));
    }
    Ok(body
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

pub fn events_url(api: &str, game_id: &str) -> String {
    format!("{}/v1/games/{game_id}/events", api.trim_end_matches('/'))
}

fn watch(config: &Config, game_id: &str, once: bool) -> Result<String, String> {
    if let Some(api) = &config.api_url {
        let url = events_url(api, game_id);
        loop {
            let body = reqwest::blocking::get(&url)
                .map_err(|err| format!("watch failed: {err}"))?
                .text()
                .map_err(|err| err.to_string())?;
            if once {
                return Ok(body);
            }
            println!("{body}");
            thread::sleep(Duration::from_secs(2));
        }
    }
    require_account(&config.heist_id, "FE2VI_HEIST_ID")?;
    let args = serde_json::to_vec(&serde_json::json!({ "game_id": game_id }))
        .map_err(|err| err.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(args);
    loop {
        let result = rpc(
            &config.rpc_url,
            "query",
            serde_json::json!({
                "request_type": "call_function",
                "finality": "final",
                "account_id": config.heist_id,
                "method_name": "get_heist",
                "args_base64": encoded,
            }),
        )?;
        let text = serde_json::to_string_pretty(&result).map_err(|err| err.to_string())?;
        if once {
            return Ok(text);
        }
        println!("{text}");
        thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_crypto::{KeyType, SecretKey};
    use near_primitives::transaction::SignedTransaction;

    #[test]
    fn refuses_mainnet_and_requires_testnet() {
        let err = assert_chain("https://rpc.mainnet.near.org").unwrap_err();
        assert!(err.contains("testnet"));
        assert_chain("https://rpc.testnet.near.org").unwrap();
        assert_chain("https://rpc.testnet.fastnear.com").unwrap();
        assert!(assert_chain("https://rpc.near.org").is_err());
        assert!(require_account("", "FE2VI_REGISTRY_ID").is_err());
    }

    #[test]
    fn parses_near_prices_exactly() {
        assert_eq!(near_to_yocto("1").unwrap(), ONE_NEAR);
        assert_eq!(near_to_yocto("0.5").unwrap(), ONE_NEAR / 2);
        assert!(near_to_yocto("0").is_err());
        assert!(near_to_yocto("-1").is_err());
        assert!(near_to_yocto("1.0000000000000000000000001").is_err());
    }

    #[test]
    fn signs_a_function_call_locally() {
        let secret = SecretKey::from_random(KeyType::ED25519);
        let secret_text = secret.to_string();
        let hash = near_primitives::hash::CryptoHash::default();
        let bytes = tx::sign_call(&SignedCall {
            signer_id: "alice.testnet".into(),
            secret: &secret_text,
            receiver_id: "registry.fe2vi.testnet".into(),
            method: "revoke".into(),
            args: br#"{"agent":"alpha.registry.fe2vi.testnet"}"#.to_vec(),
            gas: 30_000_000_000_000,
            deposit: 0,
            nonce: 7,
            block_hash: hash.to_string(),
        })
        .unwrap();
        let signed: SignedTransaction = borsh::from_slice(&bytes).unwrap();
        assert_eq!(signed.transaction.signer_id().as_str(), "alice.testnet");
        match &signed.transaction.actions()[0] {
            near_primitives::transaction::Action::FunctionCall(action) => {
                assert_eq!(action.method_name, "revoke");
            }
            _ => panic!("expected a function call"),
        }
    }

    #[test]
    fn a_broadcast_receipt_failure_is_not_success() {
        let failed = serde_json::json!({
            "status": {"Failure": {"ActionError": {"kind": {"FunctionCallError": {"ExecutionError": "Smart contract panicked: token bound"}}}}},
            "transaction": {"hash": "TxHash111"},
            "receipts_outcome": [{"outcome": {"status": {"Failure": {"ActionError": {"kind": {"FunctionCallError": {"ExecutionError": "Smart contract panicked: token bound"}}}}}}}]
        });
        let message = execution_error(&failed).unwrap();
        assert!(message.contains("token bound"));
        assert!(!message.contains("ed25519"));
        let succeeded = serde_json::json!({
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{"outcome": {"status": {"SuccessValue": ""}}}]
        });
        assert!(execution_error(&succeeded).is_none());
        let partial = serde_json::json!({
            "status": {"SuccessValue": ""},
            "receipts_outcome": [{"outcome": {"status": {"Failure": {"ActionError": {"kind": {"FunctionCallError": {"ExecutionError": "agent bound"}}}}}}}]
        });
        assert!(execution_error(&partial).unwrap().contains("agent bound"));
        assert_eq!(
            receipt_block_hash(&serde_json::json!({
                "receipts_outcome": [{"block_hash": "Ea81qwLjmq9YDNA72X5BJG48LXGw4338145EVf4XJyGv", "outcome": {"status": {"SuccessValue": ""}}}]
            })).as_deref(),
            Some("Ea81qwLjmq9YDNA72X5BJG48LXGw4338145EVf4XJyGv")
        );
    }

    #[test]
    fn logs_redact_an_ed25519_secret() {
        let line = redact_line("owner mismatch ed25519:Abcdefghijk123456789 and tail");
        assert_eq!(line, "owner mismatch ed25519:[redacted] and tail");
        assert!(!line.contains("Abcdefghijk"));
    }

    #[test]
    fn events_url_stays_on_the_given_api() {
        assert_eq!(
            events_url("http://127.0.0.1:8787/", "g1"),
            "http://127.0.0.1:8787/v1/games/g1/events"
        );
    }
}

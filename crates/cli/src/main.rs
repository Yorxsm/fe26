//! Testnet-only CLI. Signing keys are read from the environment, never from
//! the repository. With no key, every command prints the transaction it would
//! submit and does not broadcast.

mod tx;

use std::thread;
use std::time::Duration;

use base64::Engine;
use clap::{Parser, Subcommand};
use fe2vi_core::{default_platform_caps, MIN_AGENT_DEPOSIT_YOCTO};
use fe2vi_intents::{issue_session, live_gate, policy_from_caps, MOCK_USD_PER_NEAR_CENTS};

use crate::tx::SignedCall;

const DEFAULT_RPC: &str = "https://rpc.testnet.near.org";
const DEFAULT_REGISTRY: &str = "registry.fe2vi.testnet";
const DEFAULT_HEIST: &str = "heist.fe2vi.testnet";

#[derive(Parser, Debug)]
#[command(name = "fe2vi", about = "fe2VI arena tools for NEAR testnet")]
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
    /// Show the Intents policy sketch. Does not call the network or print a secret.
    Intents {
        #[command(subcommand)]
        action: IntentsCommand,
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

struct Config {
    rpc_url: String,
    account_id: Option<String>,
    signer_secret: Option<String>,
    registry_id: String,
    heist_id: String,
    api_url: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    match run(&config_from_env(), cli.command) {
        Ok(output) => println!("{output}"),
        Err(err) => {
            eprintln!("error: {err}");
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
            .unwrap_or_else(|_| DEFAULT_REGISTRY.to_string()),
        heist_id: std::env::var("FE2VI_HEIST_ID").unwrap_or_else(|_| DEFAULT_HEIST.to_string()),
        api_url: std::env::var("FE2VI_API_URL")
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

fn assert_testnet(rpc_url: &str) -> Result<(), String> {
    if rpc_url.to_ascii_lowercase().contains("mainnet") {
        Err("refusing mainnet RPC; fe2VI foundation is testnet-only".into())
    } else {
        Ok(())
    }
}

fn run(config: &Config, command: Command) -> Result<String, String> {
    assert_testnet(&config.rpc_url)?;
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
                &config.registry_id,
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
                &config.registry_id,
                "revoke",
                &args,
                0,
                50_000_000_000_000,
            )
        }
        Command::Intents { action } => intents(action),
        Command::Watch { game_id, once } => watch(config, &game_id, once),
    }
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
    fn refuses_mainnet() {
        let err = assert_testnet("https://rpc.mainnet.near.org").unwrap_err();
        assert!(err.contains("testnet-only"));
        assert_testnet("https://rpc.testnet.near.org").unwrap();
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
    fn events_url_stays_on_the_given_api() {
        assert_eq!(
            events_url("http://127.0.0.1:8787/", "g1"),
            "http://127.0.0.1:8787/v1/games/g1/events"
        );
    }
}

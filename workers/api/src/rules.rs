//! Registration proof and the schedule windows. `/v1/schedule` and these checks
//! both read `fe2vi_core::room_windows`.

use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::logic::{json, ArenaState, Outcome};

pub const RECIPIENT: &str = "fe2vi.space";
const NONCE_TTL: u64 = 600;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterNonce {
    pub account: String,
    pub nonce_hex: String,
    pub expires_unix: u64,
    pub used: bool,
}

#[derive(Clone, Debug)]
pub struct ChainPassport {
    pub contract: String,
    pub owner_id: String,
    pub bound_agent: String,
    pub keys: Vec<String>,
    pub found: bool,
}

thread_local! {
    static CHAIN: std::cell::RefCell<Option<ChainPassport>> = const { std::cell::RefCell::new(None) };
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn set_chain(value: Option<ChainPassport>) {
    CHAIN.with(|slot| *slot.borrow_mut() = value);
}

pub fn chain() -> Option<ChainPassport> {
    CHAIN.with(|slot| slot.borrow().clone())
}

pub fn room_open(now: u64, room: &str) -> bool {
    fe2vi_core::room_windows(now)
        .iter()
        .any(|window| window.room == room && window.live)
}

pub fn register_message(account: &str, token_id: &str, handle: &str) -> String {
    format!("fe2vi-register\n{account}\n{token_id}\n{handle}")
}

pub fn issue_nonce(state: &mut ArenaState, account: &str, now: u64) -> Outcome {
    let Ok(account) = crate::agents::clean_near(account) else {
        return fail(
            400,
            "validation_error",
            "nearAccount is not a testnet account.",
        );
    };
    if !account.ends_with(".testnet") {
        return fail(
            400,
            "validation_error",
            "Registration uses a NEAR testnet account.",
        );
    }
    state
        .register_nonces
        .retain(|row| !row.used && row.expires_unix > now);
    let nonce_hex = crate::agents::random_hex(32);
    state.register_nonces.push(RegisterNonce {
        account: account.clone(),
        nonce_hex: nonce_hex.clone(),
        expires_unix: now.saturating_add(NONCE_TTL),
        used: false,
    });
    json(
        200,
        &serde_json::json!({
            "account": account,
            "nonce": nonce_hex,
            "recipient": RECIPIENT,
            "ttlSecs": NONCE_TTL,
        }),
        true,
        None,
    )
}

pub fn consume_nonce(state: &mut ArenaState, account: &str, nonce_hex: &str, now: u64) -> bool {
    let Some(row) = state
        .register_nonces
        .iter_mut()
        .find(|row| row.account == account && row.nonce_hex == nonce_hex && !row.used)
    else {
        return false;
    };
    if now >= row.expires_unix {
        return false;
    }
    row.used = true;
    true
}

pub fn verify_registration(
    account: &str,
    token_id: &str,
    handle: &str,
    nonce_hex: &str,
    public_key: &str,
    signature: &str,
    keys: &[String],
) -> Result<(), &'static str> {
    let nonce = decode_hex32(nonce_hex).ok_or("bad_signature")?;
    let key = decode_key(public_key).ok_or("bad_signature")?;
    let sig = decode_sig(signature).ok_or("bad_signature")?;
    if !keys.iter().any(|item| same_key(item, public_key)) {
        return Err("bad_signature");
    }
    let message = register_message(account, token_id, handle);
    let digest = nep413_digest(&message, &nonce, RECIPIENT);
    let verifying = VerifyingKey::from_bytes(&key).map_err(|_| "bad_signature")?;
    verifying
        .verify(&digest, &sig)
        .map_err(|_| "bad_signature")?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn sign_registration(
    secret: &[u8; 32],
    account: &str,
    token_id: &str,
    handle: &str,
    nonce_hex: &str,
) -> String {
    let nonce = decode_hex32(nonce_hex).unwrap_or([0; 32]);
    let message = register_message(account, token_id, handle);
    let digest = nep413_digest(&message, &nonce, RECIPIENT);
    let signing = ed25519_dalek::SigningKey::from_bytes(secret);
    let signature = signing.sign(&digest);
    format!(
        "ed25519:{}",
        bs58::encode(signature.to_bytes()).into_string()
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn public_key(secret: &[u8; 32]) -> String {
    let signing = ed25519_dalek::SigningKey::from_bytes(secret);
    format!(
        "ed25519:{}",
        bs58::encode(signing.verifying_key().to_bytes()).into_string()
    )
}

fn nep413_digest(message: &str, nonce: &[u8; 32], recipient: &str) -> [u8; 32] {
    let mut out = Vec::new();
    out.extend_from_slice(&(2u32.pow(31) + 413).to_le_bytes());
    out.extend_from_slice(&(message.len() as u32).to_le_bytes());
    out.extend_from_slice(message.as_bytes());
    out.extend_from_slice(nonce);
    out.extend_from_slice(&(recipient.len() as u32).to_le_bytes());
    out.extend_from_slice(recipient.as_bytes());
    out.push(0);
    let digest = Sha256::digest(out);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn decode_hex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn decode_key(text: &str) -> Option<[u8; 32]> {
    let raw = text.trim().trim_start_matches("ed25519:");
    let bytes = bs58::decode(raw).into_vec().ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn decode_sig(text: &str) -> Option<Signature> {
    let raw = text.trim().trim_start_matches("ed25519:");
    let bytes = bs58::decode(raw).into_vec().ok()?;
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    Some(Signature::from_bytes(&out))
}

fn same_key(left: &str, right: &str) -> bool {
    match (decode_key(left), decode_key(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left.trim() == right.trim(),
    }
}

fn fail(status: u16, code: &str, message: &str) -> Outcome {
    json(
        status,
        &serde_json::json!({ "error": code, "message": message }),
        false,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 32] = [9u8; 32];

    #[test]
    fn a_registration_signature_matches_the_account_key() {
        let account = "buyer.testnet";
        let token = "gen_001";
        let handle = "rook";
        let nonce = "ab".repeat(32);
        let key = public_key(&SECRET);
        let signature = sign_registration(&SECRET, account, token, handle, &nonce);
        assert!(verify_registration(
            account,
            token,
            handle,
            &nonce,
            &key,
            &signature,
            std::slice::from_ref(&key)
        )
        .is_ok());
        assert_eq!(
            verify_registration(account, token, handle, &nonce, &key, &signature, &[]).unwrap_err(),
            "bad_signature"
        );
        assert!(verify_registration(
            account,
            "gen_002",
            handle,
            &nonce,
            &key,
            &signature,
            std::slice::from_ref(&key)
        )
        .is_err());
    }

    #[test]
    fn registration_rejects_a_missing_passport_and_accepts_a_signed_owner() {
        use crate::logic::{dispatch, ArenaState, Incoming};
        let mut state = ArenaState::default();
        let missing = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/agents/register?now=1791051000",
                body: r#"{"handle":"rook","name":"Rook","description":"plays","tokenId":"gen_001"}"#,
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let (status, json) = http(missing);
        assert_eq!(status, 400);
        assert_eq!(json["error"], "passport_required");

        let nonce = dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/agents/register/nonce?account=buyer.testnet&now=1791051000",
                body: "",
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let (_, nonce_body) = http(nonce);
        let nonce_hex = nonce_body["nonce"].as_str().unwrap().to_string();
        let key = public_key(&SECRET);
        let signature = sign_registration(&SECRET, "buyer.testnet", "gen_001", "rook", &nonce_hex);
        set_chain(Some(ChainPassport {
            contract: "passport.fe2virecdb1e91.testnet".into(),
            owner_id: "buyer.testnet".into(),
            bound_agent: String::new(),
            keys: vec![key.clone()],
            found: true,
        }));
        let body = format!(
            r#"{{"handle":"rook","name":"Rook","description":"plays","nearAccount":"buyer.testnet","tokenId":"gen_001","publicKey":"{key}","signature":"{signature}","nonce":"{nonce_hex}"}}"#
        );
        let created = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/agents/register?now=1791051000",
                body: &body,
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let (status, json) = http(created);
        assert_eq!(status, 201, "{json}");
        assert_eq!(json["passport"]["status"], "active");
        assert!(state.chain_jobs.iter().any(|job| {
            job.kind == "bind" && job.token_id == "gen_001" && job.args["agent_id"].is_string()
        }));
        set_chain(Some(ChainPassport {
            contract: "passport.fe2virecdb1e91.testnet".into(),
            owner_id: "other.testnet".into(),
            bound_agent: String::new(),
            keys: vec![key.clone()],
            found: true,
        }));
        let again = dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/agents/register/nonce?account=buyer.testnet&now=1791051000",
                body: "",
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let (_, nonce_body) = http(again);
        let nonce_hex = nonce_body["nonce"].as_str().unwrap();
        let signature = sign_registration(&SECRET, "buyer.testnet", "gen_002", "rook2", nonce_hex);
        let body = format!(
            r#"{{"handle":"rook2","name":"Rook","description":"plays","nearAccount":"buyer.testnet","tokenId":"gen_002","publicKey":"{key}","signature":"{signature}","nonce":"{nonce_hex}"}}"#
        );
        let denied = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/agents/register?now=1791051000",
                body: &body,
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let (status, json) = http(denied);
        assert_eq!(status, 403);
        assert_eq!(json["error"], "not_owner");
        set_chain(None);
    }

    fn http(outcome: Outcome) -> (u16, serde_json::Value) {
        match outcome {
            Outcome::Http { status, body, .. } => (
                status,
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
            ),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn the_gate_and_windows_reject_play() {
        use crate::logic::{dispatch, ArenaState, Incoming};
        let mut state = ArenaState::default();
        state.book.players.push(crate::agents::Player {
            agent_id: "agt_bare".into(),
            handle: "bare".into(),
            name: "Bare".into(),
            description: String::new(),
            owner_contact: None,
            token_hash: "hash".into(),
            token_prefix: "fe2_test".into(),
            status: "active".into(),
            created_unix: 1,
            wallets: Default::default(),
            capabilities: vec!["games".into()],
            skin_id: "ingot".into(),
            war_hits: Vec::new(),
        });
        let closed = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/arenas/vault/queue?now=1790900000",
                body: "",
                authorization: Some("Bearer fe2_test"),
                ingest_token: None,
                policy_check: None,
            },
        );
        let (status, json) = http(closed);
        assert_eq!(status, 401, "{json}");
    }

    #[test]
    fn schedule_windows_are_the_enforcement_source() {
        let open = 1_791_050_460_u64;
        assert!(room_open(open, "vault"));
        assert!(room_open(open, "war"));
        assert!(!room_open(open - 7_200, "vault"));
        let rooms = fe2vi_core::room_windows(open);
        let vault = rooms.iter().find(|row| row.room == "vault").unwrap();
        assert_eq!(room_open(open, "vault"), vault.live);
    }
}

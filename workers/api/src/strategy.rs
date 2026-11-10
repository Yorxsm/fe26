//! Token-owned playbooks. Versions are encrypted in the player book and stay
//! keyed by passport token id. A transfer does not copy an agent's bearer
//! token, prompts, or contact. The new owner reads them only after binding.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;

use crate::agents::{self, Passport, PlayerBook};
use crate::logic::{json, ArenaState, Incoming, Outcome};

const MAX_VERSIONS: u64 = 256;
const MAX_PLAYBOOK: usize = 2_000;
const MAX_NOTES: usize = 800;

thread_local! {
    static STRATEGY_KEY: RefCell<Option<[u8; 32]>> = const { RefCell::new(None) };
    static CHAIN_OWNERS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Install the at-rest key for this request. `None` refuses reads and writes.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn set_strategy_key(key: Option<[u8; 32]>) {
    STRATEGY_KEY.with(|slot| *slot.borrow_mut() = key);
}

/// Record `nft_token.owner_id` for one token. The book owner is not enough.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn set_chain_owner(token_id: &str, owner_id: &str) {
    let token_id = token_id.trim().to_string();
    let owner_id = owner_id.trim().to_ascii_lowercase();
    if token_id.is_empty() || owner_id.is_empty() {
        return;
    }
    CHAIN_OWNERS.with(|slot| {
        slot.borrow_mut().insert(token_id, owner_id);
    });
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn clear_chain_owners() {
    CHAIN_OWNERS.with(|slot| slot.borrow_mut().clear());
}

/// Hash the Worker secret into an AES-256 key. The secret itself is not stored.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn key_from_secret(raw: &str) -> Option<[u8; 32]> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let digest = Sha256::digest(raw.as_bytes());
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest);
    Some(key)
}

/// One encrypted strategy version. The playbook is not in these fields.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StrategyVersion {
    pub token_id: String,
    pub version: u64,
    pub created_unix: u64,
    pub agent_id: String,
    pub game: String,
    pub nonce_hex: String,
    pub ciphertext_b64: String,
}

#[derive(Deserialize)]
struct AppendBody {
    game: Option<String>,
    playbook: Option<String>,
    preferred: Option<String>,
    notes: Option<String>,
    prompt: Option<serde_json::Value>,
    prompts: Option<serde_json::Value>,
    credential: Option<serde_json::Value>,
    credentials: Option<serde_json::Value>,
    #[serde(rename = "apiToken")]
    api_token: Option<serde_json::Value>,
    #[serde(rename = "privateKey")]
    private_key: Option<serde_json::Value>,
}

pub fn read(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let access = match access(state, &agent_id) {
        Ok(access) => access,
        Err(outcome) => return outcome,
    };
    let Some(key) = current_key() else {
        return fail(
            503,
            "strategy_unconfigured",
            "Strategy storage is not configured.",
        );
    };
    let versions = match decrypt_all(&state.book, &access.token_id, &key) {
        Ok(versions) => versions,
        Err(()) => {
            return fail(
                500,
                "strategy_unreadable",
                "A stored strategy version could not be opened.",
            )
        }
    };
    let (count, updated) = public_meta(&state.book, &access.token_id);
    json(
        200,
        &serde_json::json!({
            "tokenId": access.token_id,
            "ownerNear": access.owner_near,
            "versionCount": count,
            "updatedAt": updated_at(updated),
            "versions": versions,
        }),
        false,
        None,
    )
}

pub fn append(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = agents::authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let access = match access(state, &agent_id) {
        Ok(access) => access,
        Err(outcome) => return outcome,
    };
    let Some(key) = current_key() else {
        return fail(
            503,
            "strategy_unconfigured",
            "Strategy storage is not configured.",
        );
    };
    let body: AppendBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send game, and optionally playbook, preferred, and notes.",
            )
        }
    };
    if body.prompt.is_some()
        || body.prompts.is_some()
        || body.credential.is_some()
        || body.credentials.is_some()
        || body.api_token.is_some()
        || body.private_key.is_some()
    {
        return fail(
            400,
            "validation_error",
            "Agent prompts and credentials do not go in the passport strategy.",
        );
    }
    let Some(game) = clean_game(body.game.as_deref()) else {
        return fail(
            400,
            "validation_error",
            "Game is 1 to 32 lowercase letters, numbers, or dashes.",
        );
    };
    let playbook = match clean_text(body.playbook.as_deref(), MAX_PLAYBOOK) {
        Ok(value) => value,
        Err(()) => {
            return fail(
                400,
                "validation_error",
                "Playbook is up to 2000 characters.",
            )
        }
    };
    let notes = match clean_text(body.notes.as_deref(), MAX_NOTES) {
        Ok(value) => value,
        Err(()) => return fail(400, "validation_error", "Notes are up to 800 characters."),
    };
    let preferred = match clean_text(body.preferred.as_deref(), 32) {
        Ok(value) => value,
        Err(()) => return fail(400, "validation_error", "Preferred is up to 32 characters."),
    };
    let previous = match latest_doc(&state.book, &access.token_id, &key) {
        Ok(doc) => doc,
        Err(()) => {
            return fail(
                500,
                "strategy_unreadable",
                "A stored strategy version could not be opened.",
            )
        }
    };
    let doc = merge_doc(
        &previous,
        &GameUpdate {
            game: &game,
            playbook: &playbook,
            preferred: &preferred,
            notes: &notes,
            from_play: false,
            points: 0,
            action: "",
        },
    );
    let version = match push_version(state, &access, &agent_id, &game, &doc, &key, now) {
        Ok(version) => version,
        Err(outcome) => return outcome,
    };
    json(
        201,
        &serde_json::json!({
            "tokenId": access.token_id,
            "version": version,
            "versionCount": version,
            "game": game,
        }),
        true,
        None,
    )
}

/// The bound agent plays `playbook` by sending action type `playbook`.
pub fn action_for_play(state: &ArenaState, agent_id: &str, game: &str, requested: &str) -> String {
    if requested != "playbook" {
        return requested.to_string();
    }
    let Some(preferred) = preferred_action(state, agent_id, game) else {
        return requested.to_string();
    };
    if game == "vault"
        && !crate::vault::VaultMatch::legal_actions()
            .iter()
            .any(|action| *action == preferred)
    {
        return requested.to_string();
    }
    preferred
}

/// After a legal play, append a match note onto the token's strategy.
pub fn note_play(state: &mut ArenaState, agent_id: &str, note: &PlayNote<'_>) {
    let Ok(access) = access(state, agent_id) else {
        return;
    };
    let Some(key) = current_key() else {
        return;
    };
    let previous = match latest_doc(&state.book, &access.token_id, &key) {
        Ok(doc) => doc,
        Err(()) => return,
    };
    let text = if note.submitted_only {
        format!("{} submitted {}", note.match_id, note.action)
    } else {
        format!("{} {} scored {}", note.match_id, note.action, note.points)
    };
    let doc = merge_doc(
        &previous,
        &GameUpdate {
            game: note.game,
            playbook: "",
            preferred: "",
            notes: &text,
            from_play: true,
            points: note.points,
            action: note.action,
        },
    );
    let _ = push_version(state, &access, agent_id, note.game, &doc, &key, note.now);
}

pub struct PlayNote<'a> {
    pub game: &'a str,
    pub action: &'a str,
    pub points: i32,
    pub match_id: &'a str,
    pub now: u64,
    pub submitted_only: bool,
}

struct GameUpdate<'a> {
    game: &'a str,
    playbook: &'a str,
    preferred: &'a str,
    notes: &'a str,
    from_play: bool,
    points: i32,
    action: &'a str,
}

pub fn public_meta(book: &PlayerBook, token_id: &str) -> (u64, Option<u64>) {
    let mut count = 0_u64;
    let mut latest = None;
    for row in &book.strategies {
        if row.token_id != token_id {
            continue;
        }
        count = count.saturating_add(1);
        latest = Some(latest.map_or(row.created_unix, |unix: u64| unix.max(row.created_unix)));
    }
    (count, latest)
}

pub fn stamp_public(body: &mut serde_json::Value, book: &PlayerBook, token_id: &str) {
    let (count, updated) = public_meta(book, token_id);
    let Some(object) = body
        .get_mut("passport")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    object.insert("strategyVersionCount".into(), serde_json::json!(count));
    object.insert("strategyUpdatedAt".into(), updated_at(updated));
    let hashes = strategy_hashes(book, token_id);
    object.insert(
        "strategyHash".into(),
        hashes
            .last()
            .map(|row| serde_json::json!(row.1.clone()))
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "strategyHashes".into(),
        serde_json::json!(hashes
            .iter()
            .map(|(version, hash)| serde_json::json!({"version": version, "sha256": hash}))
            .collect::<Vec<_>>()),
    );
}

/// sha256 of each encrypted version, hex encoded. The playbook is not hashed
/// in plaintext and is not included here.
pub fn strategy_hashes(book: &PlayerBook, token_id: &str) -> Vec<(u64, String)> {
    let mut rows = Vec::new();
    for row in &book.strategies {
        if row.token_id != token_id {
            continue;
        }
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(row.ciphertext_b64.trim())
        else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        rows.push((row.version, hex_encode(&Sha256::digest(bytes))));
    }
    rows.sort_by_key(|(version, _)| *version);
    rows
}

/// A chain view wins when it is a 64-character hex digest. Plaintext is ignored.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn apply_chain_playbook(body: &mut serde_json::Value, chain_hash: Option<&str>) {
    let Some(hash) = chain_hash else {
        return;
    };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return;
    }
    let Some(object) = body
        .get_mut("passport")
        .and_then(|value| value.as_object_mut())
    else {
        return;
    };
    object.insert(
        "strategyHash".into(),
        serde_json::json!(hash.to_ascii_lowercase()),
    );
    object.insert("strategyHashSource".into(), serde_json::json!("chain"));
}

struct Access {
    token_id: String,
    owner_near: String,
}

fn access(state: &ArenaState, agent_id: &str) -> Result<Access, Outcome> {
    if !state.chain_network.is_empty() && state.chain_network != "testnet" {
        return Err(fail(
            403,
            "chain_mismatch",
            "Passport strategy is testnet only.",
        ));
    }
    let Some(passport) = bound_passport(state, agent_id) else {
        return Err(fail(
            403,
            "not_bound",
            "Bind your agent to this passport before using its strategy.",
        ));
    };
    let token_id = passport.token_id.clone().unwrap_or_default();
    let Some(chain_owner) = chain_owner(&token_id) else {
        return Err(fail(
            503,
            "strategy_owner_unverified",
            "The passport owner could not be read from nft_token.",
        ));
    };
    let wallet = state
        .book
        .players
        .iter()
        .find(|player| player.agent_id == agent_id)
        .and_then(|player| player.wallets.near_account.clone())
        .unwrap_or_default();
    if wallet != chain_owner || passport.owner_near != chain_owner {
        return Err(fail(
            403,
            "not_owner",
            "Only the current on-chain owner can use this strategy.",
        ));
    }
    Ok(Access {
        token_id,
        owner_near: chain_owner,
    })
}

fn bound_passport<'a>(state: &'a ArenaState, agent_id: &str) -> Option<&'a Passport> {
    state.book.passports.iter().find(|passport| {
        passport.agent_id == agent_id
            && passport.status == "active"
            && passport
                .token_id
                .as_deref()
                .is_some_and(|token| !token.is_empty())
    })
}

fn chain_owner(token_id: &str) -> Option<String> {
    CHAIN_OWNERS.with(|slot| slot.borrow().get(token_id).cloned())
}

fn current_key() -> Option<[u8; 32]> {
    STRATEGY_KEY.with(|slot| *slot.borrow())
}

fn preferred_action(state: &ArenaState, agent_id: &str, game: &str) -> Option<String> {
    let access = access(state, agent_id).ok()?;
    let key = current_key()?;
    let doc = latest_doc(&state.book, &access.token_id, &key).ok()?;
    let preferred = doc
        .pointer(&format!("/games/{game}/preferred"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if preferred.is_empty() {
        None
    } else {
        Some(preferred)
    }
}

fn latest_doc(book: &PlayerBook, token_id: &str, key: &[u8; 32]) -> Result<serde_json::Value, ()> {
    let Some(row) = book
        .strategies
        .iter()
        .filter(|row| row.token_id == token_id)
        .max_by_key(|row| row.version)
    else {
        return Ok(serde_json::json!({ "games": {} }));
    };
    decrypt_doc(row, key)
}

fn decrypt_all(
    book: &PlayerBook,
    token_id: &str,
    key: &[u8; 32],
) -> Result<Vec<serde_json::Value>, ()> {
    let mut rows: Vec<&StrategyVersion> = book
        .strategies
        .iter()
        .filter(|row| row.token_id == token_id)
        .collect();
    rows.sort_by_key(|row| row.version);
    let mut out = Vec::new();
    for row in rows {
        let doc = decrypt_doc(row, key)?;
        out.push(serde_json::json!({
            "version": row.version,
            "createdAt": agents::rfc3339(row.created_unix),
            "agentId": row.agent_id,
            "game": row.game,
            "strategy": doc,
        }));
    }
    Ok(out)
}

fn merge_doc(previous: &serde_json::Value, update: &GameUpdate<'_>) -> serde_json::Value {
    let game = update.game;
    let playbook = update.playbook;
    let preferred = update.preferred;
    let notes = update.notes;
    let from_play = update.from_play;
    let points = update.points;
    let action = update.action;
    let mut doc = if previous
        .get("games")
        .and_then(|value| value.as_object())
        .is_some()
    {
        previous.clone()
    } else {
        serde_json::json!({ "games": {} })
    };
    let prior = doc
        .pointer(&format!("/games/{game}"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let mut playbook_text = prior
        .get("playbook")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut preferred_text = prior
        .get("preferred")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut plays = prior
        .get("plays")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if !playbook.is_empty() {
        playbook_text = playbook.to_string();
    }
    if !preferred.is_empty() {
        preferred_text = preferred.to_string();
    }
    if from_play {
        plays = plays.saturating_add(1);
    }
    let notes_text = if notes.is_empty() {
        prior
            .get("notes")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string()
    } else {
        notes.to_string()
    };
    let mut game_doc = serde_json::json!({
        "playbook": playbook_text,
        "preferred": preferred_text,
        "plays": plays,
        "notes": notes_text,
    });
    if from_play {
        if let Some(object) = game_doc.as_object_mut() {
            object.insert("lastAction".into(), serde_json::json!(action));
            object.insert("lastPoints".into(), serde_json::json!(points));
        }
    } else if let Some(last) = prior.get("lastAction") {
        if let Some(object) = game_doc.as_object_mut() {
            object.insert("lastAction".into(), last.clone());
            if let Some(points) = prior.get("lastPoints") {
                object.insert("lastPoints".into(), points.clone());
            }
        }
    }
    if let Some(games) = doc.get_mut("games").and_then(|value| value.as_object_mut()) {
        games.insert(game.to_string(), game_doc);
    }
    doc
}

fn push_version(
    state: &mut ArenaState,
    access: &Access,
    agent_id: &str,
    game: &str,
    doc: &serde_json::Value,
    key: &[u8; 32],
    now: u64,
) -> Result<u64, Outcome> {
    let version = state
        .book
        .strategies
        .iter()
        .filter(|row| row.token_id == access.token_id)
        .map(|row| row.version)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    if version > MAX_VERSIONS {
        return Err(fail(
            409,
            "strategy_full",
            "This passport has the maximum number of strategy versions.",
        ));
    }
    let plaintext = serde_json::to_vec(doc).map_err(|_| {
        fail(
            500,
            "strategy_seal_failed",
            "The strategy could not be stored.",
        )
    })?;
    let nonce = nonce_for(&access.token_id, version, now, agent_id);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| {
        fail(
            500,
            "strategy_seal_failed",
            "The strategy could not be stored.",
        )
    })?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: access.token_id.as_bytes(),
            },
        )
        .map_err(|_| {
            fail(
                500,
                "strategy_seal_failed",
                "The strategy could not be stored.",
            )
        })?;
    state.book.strategies.push(StrategyVersion {
        token_id: access.token_id.clone(),
        version,
        created_unix: now,
        agent_id: agent_id.to_string(),
        game: game.to_string(),
        nonce_hex: hex_encode(&nonce),
        ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    });
    Ok(version)
}

fn decrypt_doc(row: &StrategyVersion, key: &[u8; 32]) -> Result<serde_json::Value, ()> {
    let nonce = hex_decode(&row.nonce_hex).ok_or(())?;
    if nonce.len() != 12 {
        return Err(());
    }
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(row.ciphertext_b64.trim())
        .map_err(|_| ())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| ())?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: row.token_id.as_bytes(),
            },
        )
        .map_err(|_| ())?;
    serde_json::from_slice(&plaintext).map_err(|_| ())
}

fn nonce_for(token_id: &str, version: u64, unix: u64, agent_id: &str) -> [u8; 12] {
    let digest = Sha256::digest(format!("{token_id}|{version}|{unix}|{agent_id}").as_bytes());
    let mut nonce = [0_u8; 12];
    nonce.copy_from_slice(&digest[..12]);
    nonce
}

fn clean_game(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if (1..=32).contains(&value.len())
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        Some(value.to_string())
    } else {
        None
    }
}

fn clean_text(value: Option<&str>, max: usize) -> Result<String, ()> {
    let value = value.unwrap_or("").trim();
    if value.chars().count() > max || value.chars().any(|ch| ch.is_control()) {
        Err(())
    } else {
        Ok(value.to_string())
    }
}

fn updated_at(unix: Option<u64>) -> serde_json::Value {
    match unix {
        Some(unix) => serde_json::Value::String(agents::rfc3339(unix)),
        None => serde_json::Value::Null,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_nibble(bytes[index])?;
        let low = hex_nibble(bytes[index + 1])?;
        out.push((high << 4) | low);
        index += 2;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn fail(status: u16, error: &str, message: &str) -> Outcome {
    json(
        status,
        &serde_json::json!({ "error": error, "message": message }),
        false,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{dispatch, Incoming};
    use crate::passport::reconcile_passport;

    const NOW: u64 = 1_790_900_000;
    const TOKEN: &str = "disp_strategy";

    struct Guard;

    impl Drop for Guard {
        fn drop(&mut self) {
            set_strategy_key(None);
            clear_chain_owners();
        }
    }

    fn call(
        state: &mut ArenaState,
        method: &str,
        path: &str,
        body: &str,
        token: Option<&str>,
    ) -> (u16, serde_json::Value) {
        let auth = token.map(|token| format!("Bearer {token}"));
        let outcome = dispatch(
            state,
            &Incoming {
                method,
                path,
                body,
                authorization: auth.as_deref(),
                ingest_token: None,
                policy_check: None,
            },
        );
        match outcome {
            Outcome::Http { status, body, .. } => (
                status,
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
            ),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    fn register(state: &mut ArenaState, handle: &str, near: &str) -> (String, String) {
        let body = format!(
            r#"{{"handle":"{handle}","name":"{handle}","description":"{handle} prompt stays","ownerContact":"{handle}-contact","wallets":{{"nearAccount":"{near}"}}}}"#
        );
        let (status, json) = call(
            state,
            "POST",
            &format!("/v1/agents/register?now={NOW}"),
            &body,
            None,
        );
        assert_eq!(status, 201, "{json}");
        (
            json["agentId"].as_str().unwrap().to_string(),
            json["apiToken"].as_str().unwrap().to_string(),
        )
    }

    fn bind_book(state: &mut ArenaState, agent_id: &str, owner: &str) {
        state.book.passports.push(agents::Passport {
            passport_id: "ppt_strategy".into(),
            token_id: Some(TOKEN.into()),
            agent_id: agent_id.to_string(),
            owner_near: owner.to_string(),
            creator_near: "seller.testnet".into(),
            status: "active".into(),
            created_unix: NOW,
            updated_unix: NOW,
            last_sync_seq: 1,
            career: None,
        });
    }

    fn arm(owner: &str) -> Guard {
        set_strategy_key(key_from_secret("test-strategy-key"));
        set_chain_owner(TOKEN, owner);
        Guard
    }

    #[test]
    fn owner_reads_strategy_and_stranger_is_forbidden() {
        let _guard = arm("seller.testnet");
        let mut state = ArenaState {
            chain_network: "testnet".into(),
            ..ArenaState::default()
        };
        let (seller_id, seller) = register(&mut state, "seller", "seller.testnet");
        let (_other_id, other) = register(&mut state, "looker", "looker.testnet");
        bind_book(&mut state, &seller_id, "seller.testnet");
        let seller_hash = state
            .book
            .players
            .iter()
            .find(|player| player.agent_id == seller_id)
            .unwrap()
            .token_hash
            .clone();
        let append = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            r#"{"game":"vault","playbook":"hold the east door","preferred":"scout","notes":"opening book"}"#,
            Some(&seller),
        );
        assert_eq!(append.0, 201, "{}", append.1);
        assert_eq!(append.1["version"], 1);
        let stored = &state.book.strategies[0].ciphertext_b64;
        assert!(!stored.contains("hold the east door"));
        assert!(!stored.contains("opening book"));
        let read = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&seller),
        );
        assert_eq!(read.0, 200, "{}", read.1);
        assert_eq!(read.1["versionCount"], 1);
        assert_eq!(
            read.1["versions"][0]["strategy"]["games"]["vault"]["playbook"],
            "hold the east door"
        );
        set_chain_owner(TOKEN, "buyer.testnet");
        let not_owner = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&seller),
        );
        assert_eq!(not_owner.0, 403, "{}", not_owner.1);
        assert_eq!(not_owner.1["error"], "not_owner");
        set_chain_owner(TOKEN, "seller.testnet");
        let denied = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&other),
        );
        assert_eq!(denied.0, 403, "{}", denied.1);
        assert_eq!(denied.1["error"], "not_bound");
        state.book.passports.push(agents::Passport {
            passport_id: "ppt_other".into(),
            token_id: Some("disp_other".into()),
            agent_id: state
                .book
                .players
                .iter()
                .find(|player| player.handle == "looker")
                .unwrap()
                .agent_id
                .clone(),
            owner_near: "looker.testnet".into(),
            creator_near: "looker.testnet".into(),
            status: "active".into(),
            created_unix: NOW,
            updated_unix: NOW,
            last_sync_seq: 1,
            career: None,
        });
        set_chain_owner("disp_other", "looker.testnet");
        let still = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&other),
        );
        assert_eq!(still.0, 200, "{}", still.1);
        assert_eq!(still.1["tokenId"], "disp_other");
        assert_eq!(still.1["versionCount"], 0);
        assert!(!still.1.to_string().contains("hold the east door"));
        let public = call(
            &mut state,
            "GET",
            &format!("/v1/passports/{TOKEN}?now={NOW}"),
            "",
            None,
        );
        assert_eq!(public.0, 200, "{}", public.1);
        assert_eq!(public.1["passport"]["strategyVersionCount"], 1);
        assert!(public.1["passport"]["strategyUpdatedAt"].is_string());
        let hash = public.1["passport"]["strategyHash"].as_str().unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let hashes = strategy_hashes(&state.book, TOKEN);
        assert_eq!(hash, hashes[0].1);
        assert!(!public.1.to_string().contains("hold the east door"));
        let mut chained = public.1.clone();
        apply_chain_playbook(&mut chained, Some("not-a-digest"));
        assert_eq!(chained["passport"]["strategyHash"], hash);
        let chain_hash = "ab".repeat(32);
        apply_chain_playbook(&mut chained, Some(&chain_hash));
        assert_eq!(chained["passport"]["strategyHash"], chain_hash);
        assert_eq!(chained["passport"]["strategyHashSource"], "chain");
        assert!(!chained.to_string().contains("hold the east door"));
        assert!(!public.1.to_string().contains("seller prompt stays"));
        assert_eq!(
            state
                .book
                .players
                .iter()
                .find(|player| player.agent_id == seller_id)
                .unwrap()
                .token_hash,
            seller_hash
        );
    }

    #[test]
    fn transfer_flips_access_and_append_goes_to_the_new_owner() {
        let _guard = arm("seller.testnet");
        let mut state = ArenaState {
            chain_network: "testnet".into(),
            ..ArenaState::default()
        };
        let (seller_id, seller) = register(&mut state, "seller", "seller.testnet");
        let (buyer_id, buyer) = register(&mut state, "buyer", "buyer.testnet");
        bind_book(&mut state, &seller_id, "seller.testnet");
        let seller_hash = state.book.players[0].token_hash.clone();
        let seller_contact = state.book.players[0].owner_contact.clone();
        let wrote = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            r#"{"game":"vault","playbook":"hold the east door","preferred":"scout","notes":"opening book"}"#,
            Some(&seller),
        );
        assert_eq!(wrote.0, 201, "{}", wrote.1);
        let view = serde_json::json!({
            "owner_id": "buyer.testnet",
            "bound_agent": null,
            "creator": "seller.testnet",
            "last_sync_seq": 2
        });
        assert!(reconcile_passport(
            &mut state.book.passports[0],
            &view,
            NOW + 5,
            u64::MAX,
            &mut state.book.passport_jobs,
        ));
        set_chain_owner(TOKEN, "buyer.testnet");
        let old = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&seller),
        );
        assert_eq!(old.0, 403, "{}", old.1);
        let old_write = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            r#"{"game":"vault","playbook":"seller keeps writing"}"#,
            Some(&seller),
        );
        assert_eq!(old_write.0, 403, "{}", old_write.1);
        let unbound = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&buyer),
        );
        assert_eq!(unbound.0, 403, "{}", unbound.1);
        state.book.passports[0].status = "active".into();
        state.book.passports[0].agent_id = buyer_id.clone();
        state.book.passports[0].owner_near = "buyer.testnet".into();
        let inherited = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&buyer),
        );
        assert_eq!(inherited.0, 200, "{}", inherited.1);
        assert_eq!(
            inherited.1["versions"][0]["strategy"]["games"]["vault"]["playbook"],
            "hold the east door"
        );
        assert_eq!(inherited.1["versions"][0]["agentId"], seller_id);
        let next = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            r#"{"game":"vault","notes":"learned the east door"}"#,
            Some(&buyer),
        );
        assert_eq!(next.0, 201, "{}", next.1);
        assert_eq!(next.1["version"], 2);
        let again = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&buyer),
        );
        assert_eq!(again.0, 200, "{}", again.1);
        assert_eq!(again.1["versionCount"], 2);
        assert_eq!(again.1["versions"][1]["agentId"], buyer_id);
        assert_eq!(
            again.1["versions"][1]["strategy"]["games"]["vault"]["playbook"],
            "hold the east door"
        );
        assert_eq!(
            again.1["versions"][1]["strategy"]["games"]["vault"]["notes"],
            "learned the east door"
        );
        let seller_again = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&seller),
        );
        assert_eq!(seller_again.0, 403, "{}", seller_again.1);
        let seller_player = state
            .book
            .players
            .iter()
            .find(|player| player.agent_id == seller_id)
            .unwrap();
        assert_eq!(seller_player.token_hash, seller_hash);
        assert_eq!(seller_player.owner_contact, seller_contact);
        assert_eq!(seller_player.description, "seller prompt stays");
        let buyer_player = state
            .book
            .players
            .iter()
            .find(|player| player.agent_id == buyer_id)
            .unwrap();
        assert_ne!(buyer_player.token_hash, seller_hash);
        assert!(!again.1.to_string().contains("seller prompt stays"));
        assert!(!again.1.to_string().contains(&seller));
    }

    #[test]
    fn play_uses_the_token_strategy_and_appends_a_version() {
        let _guard = arm("seller.testnet");
        let mut state = ArenaState {
            chain_network: "testnet".into(),
            ..ArenaState::default()
        };
        let (seller_id, seller) = register(&mut state, "seller", "seller.testnet");
        let (_other_id, other) = register(&mut state, "seated", "seated.testnet");
        bind_book(&mut state, &seller_id, "seller.testnet");
        let wrote = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            r#"{"game":"vault","playbook":"hold the east door","preferred":"scout"}"#,
            Some(&seller),
        );
        assert_eq!(wrote.0, 201, "{}", wrote.1);
        let queue = format!("/v1/arenas/vault/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&seller)).0, 200);
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&other)).0, 200);
        let match_id = state.book.matches[0].match_id.clone();
        let act = format!("/v1/matches/{match_id}/actions?now={NOW}");
        let played = call(
            &mut state,
            "POST",
            &act,
            r#"{"turn":1,"action":{"type":"playbook"},"clientRequestId":"use-book"}"#,
            Some(&seller),
        );
        assert_eq!(played.0, 200, "{}", played.1);
        let pending = state.book.matches[0]
            .vault
            .as_ref()
            .unwrap()
            .pending
            .iter()
            .find(|row| row.agent_id == seller_id)
            .unwrap();
        assert_eq!(pending.action, "scout");
        assert_eq!(state.book.strategies.len(), 2);
        assert_eq!(state.book.strategies[1].agent_id, seller_id);
        assert_eq!(state.book.strategies[1].token_id, TOKEN);
        let read = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/passport/strategy?now={NOW}"),
            "",
            Some(&seller),
        );
        assert_eq!(read.0, 200, "{}", read.1);
        assert_eq!(read.1["versionCount"], 2);
        let vault = &read.1["versions"][1]["strategy"]["games"]["vault"];
        assert_eq!(vault["playbook"], "hold the east door");
        assert_eq!(vault["preferred"], "scout");
        assert_eq!(vault["plays"], 1);
        assert_eq!(vault["lastAction"], "scout");
        assert!(vault["notes"].as_str().unwrap().contains("submitted scout"));
        assert!(!state.book.strategies[1]
            .ciphertext_b64
            .contains("hold the east door"));
    }
}

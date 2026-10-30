//! Registered agents. Tokens are stored as sha256 only. Points come from the
//! same constants the mock crew uses. There is no rate limit and no registration cap.

use fe2vi_core::{
    award_in_window, challenges, score_actions, ActionKind, BoardWindow, PointRoom, PublicRow,
    SubmittedAction, POINT_SCOUT,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::logic::{http, json, query_param, ArenaState, Incoming, Outcome};
use crate::skins;

const TURN_SECS: u64 = 60;
const API_ORIGIN: &str = "https://api.fe2vi.space";
const SITE_ORIGIN: &str = "https://fe2vi.space";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PlayerBook {
    #[serde(default)]
    pub players: Vec<Player>,
    #[serde(default)]
    pub queue: Vec<Queued>,
    #[serde(default)]
    pub matches: Vec<PlayMatch>,
    #[serde(default)]
    pub seen: Vec<Seen>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub agent_id: String,
    pub handle: String,
    pub name: String,
    pub description: String,
    pub owner_contact: Option<String>,
    pub token_hash: String,
    pub token_prefix: String,
    pub status: String,
    pub created_unix: u64,
    #[serde(default)]
    pub wallets: Wallets,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "default_skin_id")]
    pub skin_id: String,
}

fn default_skin_id() -> String {
    "ingot".to_string()
}

/// Public chain addresses only. The arena never stores a private key, an API
/// key, or a grant token for an agent.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wallets {
    #[serde(default)]
    pub evm_address: Option<String>,
    #[serde(default)]
    pub near_account: Option<String>,
    #[serde(default)]
    pub hyperliquid_account: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Queued {
    pub arena_id: String,
    pub agent_id: String,
    pub queued_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayMatch {
    pub match_id: String,
    pub arena_id: String,
    pub status: String,
    pub turn: u32,
    pub deadline_unix: u64,
    pub seats: Vec<String>,
    pub actions: Vec<Played>,
    #[serde(default)]
    pub created_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Played {
    pub agent_id: String,
    pub turn: u32,
    pub action_type: String,
    pub points: u32,
    pub unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seen {
    pub agent_id: String,
    pub client_request_id: String,
    pub status: u16,
    pub body: String,
}

#[derive(Deserialize)]
struct RegisterBody {
    handle: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "ownerContact")]
    owner_contact: Option<String>,
    wallets: Option<WalletBody>,
    capabilities: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct WalletBody {
    #[serde(rename = "evmAddress")]
    evm_address: Option<String>,
    #[serde(rename = "nearAccount")]
    near_account: Option<String>,
    #[serde(rename = "hyperliquidAccount")]
    hyperliquid_account: Option<String>,
}

#[derive(Deserialize)]
struct ActionBody {
    turn: Option<u32>,
    action: Option<ActionIn>,
    #[serde(rename = "clientRequestId")]
    client_request_id: Option<String>,
}

#[derive(Deserialize)]
struct ActionIn {
    #[serde(rename = "type")]
    kind: Option<String>,
}

pub fn dispatch_player(state: &mut ArenaState, incoming: &Incoming<'_>) -> Option<Outcome> {
    let path = path_only(incoming.path);
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let now = crate::logic::query_now(incoming.path);
    let changed = settle(state, now);
    let outcome = match (incoming.method, parts.as_slice()) {
        ("GET", ["openapi.json"]) => Some(document("application/json", &openapi())),
        ("GET", ["llms.txt"]) => Some(document("text/plain; charset=utf-8", onboarding_text())),
        ("GET", ["v1", "arenas"]) => Some(json(200, &arenas_body(), false, None)),
        ("GET", ["v1", "arenas", id]) => Some(arena_one(id)),
        ("POST", ["v1", "agents", "register"]) => Some(register(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "rotate-token"]) => Some(rotate(state, incoming)),
        ("POST", ["v1", "agents", "me", "wallet"]) => Some(link_wallet(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "skin"]) => Some(equip_skin(state, incoming, now)),
        ("GET", ["v1", "agents", "me"]) => Some(me(state, incoming, now)),
        ("GET", ["v1", "skins"]) => Some(json(200, &serde_json::json!({ "skins": skins::catalog().iter().map(skins::skin_json).collect::<Vec<_>>() }), false, None)),
        ("GET", ["v1", "agents", "me", "matches"]) => Some(my_matches(state, incoming, now)),
        ("GET", ["v1", "agents", handle]) if is_handle(handle) => Some(public_agent(state, handle, now)),
        ("GET", ["v1", "matches"]) => Some(recent_matches(state, incoming)),
        ("POST", ["v1", "arenas", id, "queue"]) => Some(join_queue(state, incoming, id, now)),
        ("DELETE", ["v1", "arenas", id, "queue"]) => Some(leave_queue(state, incoming, id)),
        ("GET", ["v1", "matches", id]) => Some(spectate(state, id)),
        ("POST", ["v1", "matches", id, "actions"]) => Some(act(state, incoming, id, now)),
        _ => None,
    };
    outcome.map(|item| remember(item, changed))
}

fn remember(outcome: Outcome, changed: bool) -> Outcome {
    if !changed {
        return outcome;
    }
    match outcome {
        Outcome::Http { status, content_type, body, fanout, location, .. } => Outcome::Http {
            status,
            content_type,
            body,
            mutated: true,
            fanout,
            location,
        },
        other => other,
    }
}

pub fn onboarding_text() -> &'static str {
    include_str!("../../../templates/ONBOARDING.md")
}

pub fn registered_rows(state: &ArenaState, now: u64, window: BoardWindow, room: Option<PointRoom>) -> Vec<PublicRow> {
    state
        .book
        .players
        .iter()
        .map(|player| {
            let scores = room_scores(state, &player.agent_id, now, window);
            let points = match room {
                Some(PointRoom::Vault) => scores.vault,
                Some(PointRoom::Cup) => scores.cup,
                Some(PointRoom::Village) => scores.village,
                Some(PointRoom::War) => scores.war,
                None => scores.total,
            };
            PublicRow {
                rank: 0,
                agent: player.handle.clone(),
                name: player.name.clone(),
                colour: "#d4d2cc".to_string(),
                points,
                games: scores.games,
                vault: scores.vault,
                cup: scores.cup,
                village: scores.village,
                war: scores.war,
            }
        })
        .collect()
}

pub fn registered_recent(state: &ArenaState, handle: &str, now: u64, window: BoardWindow) -> Option<Vec<serde_json::Value>> {
    let player = state.book.players.iter().find(|player| player.handle == handle)?;
    let mut rows = Vec::new();
    for play in &state.book.matches {
        for action in &play.actions {
            if action.agent_id != player.agent_id || !award_in_window(now, window, action.unix) {
                continue;
            }
            rows.push(serde_json::json!({
                "room": arena_room(&play.arena_id),
                "points": action.points,
                "unix": action.unix,
                "label": action.action_type,
            }));
        }
    }
    rows.sort_by(|left, right| right["unix"].as_u64().cmp(&left["unix"].as_u64()));
    rows.truncate(8);
    Some(rows)
}

struct Scores {
    total: u32,
    games: u32,
    vault: u32,
    cup: u32,
    village: u32,
    war: u32,
}

fn room_scores(state: &ArenaState, agent_id: &str, now: u64, window: BoardWindow) -> Scores {
    let mut scores = Scores { total: 0, games: 0, vault: 0, cup: 0, village: 0, war: 0 };
    for play in &state.book.matches {
        for action in &play.actions {
            if action.agent_id != agent_id || !award_in_window(now, window, action.unix) {
                continue;
            }
            scores.games = scores.games.saturating_add(1);
            scores.total = scores.total.saturating_add(action.points);
            match play.arena_id.as_str() {
                "vault" => scores.vault = scores.vault.saturating_add(action.points),
                "cup" => scores.cup = scores.cup.saturating_add(action.points),
                "village" => scores.village = scores.village.saturating_add(action.points),
                "war" => scores.war = scores.war.saturating_add(action.points),
                _ => {}
            }
        }
    }
    scores
}

fn clean_evm(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.len() == 42 && value.starts_with("0x") && value[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err("EVM and Hyperliquid addresses are 0x plus 40 hex characters.")
    }
}

fn clean_near(value: &str) -> Result<String, &'static str> {
    let value = value.trim().to_ascii_lowercase();
    let ok = (2..=64).contains(&value.len())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && value.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && value.chars().last().is_some_and(|c| c.is_ascii_alphanumeric());
    if ok { Ok(value) } else { Err("NEAR accounts are 2 to 64 lowercase characters.") }
}

fn clean_wallets(body: Option<WalletBody>) -> Result<Wallets, &'static str> {
    let mut wallets = Wallets::default();
    let Some(body) = body else { return Ok(wallets); };
    wallets.evm_address = match body.evm_address.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(clean_evm(value)?),
        None => None,
    };
    wallets.near_account = match body.near_account.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(clean_near(value)?),
        None => None,
    };
    wallets.hyperliquid_account = match body.hyperliquid_account.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Some(clean_evm(value)?),
        None => None,
    };
    Ok(wallets)
}

fn clean_capabilities(input: Option<Vec<String>>) -> Vec<String> {
    const ALLOWED: &[&str] = &["games", "paper-trading", "live-trading", "market-making", "research"];
    let mut out = Vec::new();
    for value in input.unwrap_or_default() {
        let value = value.trim().to_ascii_lowercase();
        if ALLOWED.contains(&value.as_str()) && !out.contains(&value) {
            out.push(value);
        }
        if out.len() == ALLOWED.len() {
            break;
        }
    }
    if out.is_empty() {
        out.push("games".to_string());
    }
    out
}

fn register(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let body: RegisterBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send handle, name, and description."),
    };
    let Some(handle) = body.handle.as_deref().map(str::trim).filter(|value| is_handle(value)) else {
        return fail(400, "validation_error", "Handle is 3 to 30 characters, lowercase, starting with a letter.");
    };
    let Some(name) = clean_text(body.name.as_deref(), 1, 40) else {
        return fail(400, "validation_error", "Name is required, up to 40 characters.");
    };
    let Some(description) = clean_text(Some(body.description.as_deref().unwrap_or("")), 0, 280) else {
        return fail(400, "validation_error", "Description is up to 280 characters.");
    };
    let owner = match body.owner_contact.as_deref() {
        None => None,
        Some(value) if value.trim().is_empty() => None,
        Some(value) => match clean_text(Some(value), 1, 120) {
            Some(text) => Some(text),
            None => return fail(400, "validation_error", "Owner contact is up to 120 characters."),
        },
    };
    let wallets = match clean_wallets(body.wallets) {
        Ok(wallets) => wallets,
        Err(message) => return fail(400, "validation_error", message),
    };
    let capabilities = clean_capabilities(body.capabilities);
    if state.book.players.iter().any(|player| player.handle == handle) {
        return fail(409, "handle_taken", "That handle is already registered.");
    }
    let token = fresh_token();
    let agent_id = format!("agt_{}", random_hex(8));
    let prefix = token.chars().take(8).collect::<String>();
    state.book.players.push(Player {
        agent_id: agent_id.clone(),
        handle: handle.to_string(),
        name: name.clone(),
        description,
        owner_contact: owner,
        token_hash: token_hash(&token),
        token_prefix: prefix.clone(),
        status: "active".to_string(),
        created_unix: now,
        wallets: wallets.clone(),
        capabilities: capabilities.clone(),
        skin_id: "ingot".to_string(),
    });
    json(
        201,
        &serde_json::json!({
            "agentId": agent_id,
            "handle": handle,
            "name": name,
            "apiToken": token,
            "tokenPrefix": prefix,
            "points": 0,
            "wallets": wallets,
            "capabilities": capabilities,
            "profileUrl": format!("{SITE_ORIGIN}/agents/{handle}"),
            "onboardingUrl": format!("{SITE_ORIGIN}/skill/SKILL.md"),
        }),
        true,
        None,
    )
}

fn rotate(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let Some(player) = authed_mut(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    let token = fresh_token();
    let prefix = token.chars().take(8).collect::<String>();
    player.token_hash = token_hash(&token);
    player.token_prefix = prefix.clone();
    json(
        200,
        &serde_json::json!({ "apiToken": token, "tokenPrefix": prefix }),
        true,
        None,
    )
}

fn link_wallet(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let body: WalletBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send evmAddress, nearAccount, or hyperliquidAccount."),
    };
    let Some(player) = authed_mut(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    if let Some(value) = body.evm_address.as_deref() {
        player.wallets.evm_address = if value.trim().is_empty() {
            None
        } else {
            match clean_evm(value) {
                Ok(address) => Some(address),
                Err(message) => return fail(400, "validation_error", message),
            }
        };
    }
    if let Some(value) = body.near_account.as_deref() {
        player.wallets.near_account = if value.trim().is_empty() {
            None
        } else {
            match clean_near(value) {
                Ok(account) => Some(account),
                Err(message) => return fail(400, "validation_error", message),
            }
        };
    }
    if let Some(value) = body.hyperliquid_account.as_deref() {
        player.wallets.hyperliquid_account = if value.trim().is_empty() {
            None
        } else {
            match clean_evm(value) {
                Ok(address) => Some(address),
                Err(message) => return fail(400, "validation_error", message),
            }
        };
    }
    let wallets = player.wallets.clone();
    let handle = player.handle.clone();
    json(
        200,
        &serde_json::json!({
            "handle": handle,
            "wallets": wallets,
            "tradeMode": "paper",
            "updatedAt": rfc3339(now),
        }),
        true,
        None,
    )
}

fn equip_skin(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    #[derive(Deserialize)]
    struct SkinBody {
        #[serde(rename = "skinId")]
        skin_id: Option<String>,
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    let body: SkinBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send skinId."),
    };
    let Some(skin) = body.skin_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).and_then(skins::find) else {
        return fail(404, "not_found", "No skin with that id.");
    };
    let points = room_scores(state, &agent_id, now, BoardWindow::All).total;
    if points < skin.unlock_points {
        return fail(403, "skin_locked", "That skin needs more points.");
    }
    if let Some(player) = state.book.players.iter_mut().find(|player| player.agent_id == agent_id) {
        player.skin_id = skin.id.to_string();
    }
    json(
        200,
        &serde_json::json!({
            "skin": skins::skin_json(skin),
            "points": points,
            "updatedAt": rfc3339(now),
        }),
        true,
        None,
    )
}

fn me(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    let (agent_id, handle, name, status, wallets, capabilities, skin_id) = {
        let player = state.book.players.iter().find(|player| player.agent_id == agent_id).unwrap();
        (player.agent_id.clone(), player.handle.clone(), player.name.clone(), player.status.clone(), player.wallets.clone(), player.capabilities.clone(), player.skin_id.clone())
    };
    let skin = skins::find(&skin_id).unwrap_or_else(skins::default_skin);
    let scores = room_scores(state, &agent_id, now, BoardWindow::All);
    let rank = rank_of(state, &handle, now);
    json(
        200,
        &serde_json::json!({
            "agentId": agent_id,
            "handle": handle,
            "name": name,
            "points": scores.total,
            "rank": rank,
            "status": status,
            "wallets": wallets,
            "capabilities": capabilities,
            "skin": skins::skin_json(skin),
            "tradeMode": "paper",
        }),
        false,
        None,
    )
}

fn public_agent(state: &ArenaState, handle: &str, now: u64) -> Outcome {
    let Some(player) = state.book.players.iter().find(|player| player.handle == handle).cloned() else {
        return fail(404, "not_found", "No agent with that handle.");
    };
    let scores = room_scores(state, &player.agent_id, now, BoardWindow::All);
    let skin = skins::find(&player.skin_id).unwrap_or_else(skins::default_skin);
    json(
        200,
        &serde_json::json!({
            "agentId": player.agent_id,
            "handle": player.handle,
            "name": player.name,
            "description": player.description,
            "points": scores.total,
            "rank": rank_of(state, handle, now),
            "status": player.status,
            "wallets": player.wallets,
            "capabilities": player.capabilities,
            "skin": skins::skin_json(skin),
            "profileUrl": format!("{SITE_ORIGIN}/agents/{handle}"),
        }),
        false,
        None,
    )
}

fn join_queue(state: &mut ArenaState, incoming: &Incoming<'_>, arena_id: &str, now: u64) -> Outcome {
    if arena(arena_id).is_none() {
        return fail(404, "not_found", "Unknown arena.");
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    if state.book.matches.iter().any(|play| play.status == "live" && play.arena_id == arena_id && play.seats.iter().any(|seat| seat == &agent_id)) {
        return fail(409, "invalid_state_transition", "You are already in a live match in this arena.");
    }
    if !state.book.queue.iter().any(|seat| seat.arena_id == arena_id && seat.agent_id == agent_id) {
        state.book.queue.push(Queued { arena_id: arena_id.to_string(), agent_id: agent_id.clone(), queued_unix: now });
    }
    pair_queue(state, arena_id, now);
    json(200, &serde_json::json!({ "queued": true, "arenaId": arena_id }), true, None)
}

fn leave_queue(state: &mut ArenaState, incoming: &Incoming<'_>, arena_id: &str) -> Outcome {
    if arena(arena_id).is_none() {
        return fail(404, "not_found", "Unknown arena.");
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    state.book.queue.retain(|seat| !(seat.arena_id == arena_id && seat.agent_id == agent_id));
    json(200, &serde_json::json!({ "queued": false, "arenaId": arena_id }), true, None)
}

fn recent_matches(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let limit = query_param(incoming.path, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 50);
    let mut rows: Vec<&PlayMatch> = state.book.matches.iter().collect();
    rows.sort_by(|left, right| {
        right
            .status
            .eq("live")
            .cmp(&left.status.eq("live"))
            .then_with(|| right.created_unix.cmp(&left.created_unix))
    });
    let matches: Vec<serde_json::Value> = rows
        .into_iter()
        .take(limit)
        .map(|play| {
            serde_json::json!({
                "matchId": play.match_id,
                "arenaId": play.arena_id,
                "status": play.status,
                "turn": play.turn,
                "seats": play.seats.iter().map(|seat| handle_of(state, seat)).collect::<Vec<_>>(),
                "actions": play.actions.len(),
                "createdAt": if play.created_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(rfc3339(play.created_unix)) },
            })
        })
        .collect();
    json(200, &serde_json::json!({ "matches": matches }), false, None)
}

fn my_matches(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    let mut rows: Vec<serde_json::Value> = state
        .book
        .matches
        .iter()
        .filter(|play| play.status == "live" && play.seats.iter().any(|seat| seat == &agent_id))
        .map(|play| match_for_seat(state, play, Some(&agent_id), now))
        .collect();
    rows.reverse();
    json(200, &serde_json::json!({ "matches": rows }), false, None)
}

fn spectate(state: &ArenaState, match_id: &str) -> Outcome {
    let Some(play) = state.book.matches.iter().find(|play| play.match_id == match_id) else {
        return fail(404, "not_found", "No match with that id.");
    };
    let mut actions: Vec<serde_json::Value> = play
        .actions
        .iter()
        .rev()
        .map(|action| {
            serde_json::json!({
                "turn": action.turn,
                "type": action.action_type,
                "points": action.points,
                "handle": handle_of(state, &action.agent_id),
                "at": rfc3339(action.unix),
            })
        })
        .collect();
    if actions.is_empty() {
        actions = Vec::new();
    }
    let scores: Vec<serde_json::Value> = play
        .seats
        .iter()
        .map(|seat| {
            serde_json::json!({
                "handle": handle_of(state, seat),
                "points": play.actions.iter().filter(|action| action.agent_id == *seat).map(|action| action.points).sum::<u32>(),
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "matchId": play.match_id,
            "arenaId": play.arena_id,
            "status": play.status,
            "turn": play.turn,
            "deadlineAt": rfc3339(play.deadline_unix),
            "seats": play.seats.iter().map(|seat| handle_of(state, seat)).collect::<Vec<_>>(),
            "scores": scores,
            "actions": actions,
        }),
        false,
        None,
    )
}

fn act(state: &mut ArenaState, incoming: &Incoming<'_>, match_id: &str, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(401, "unauthorized", "Send Authorization: Bearer fe2_ and your token.");
    };
    let body: ActionBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send turn, action.type, and clientRequestId."),
    };
    let Some(request_id) = body.client_request_id.as_deref().map(str::trim).filter(|value| is_request_id(value)) else {
        return fail(400, "validation_error", "clientRequestId is 1 to 80 letters, numbers, or dashes.");
    };
    if let Some(prior) = state.book.seen.iter().find(|seen| seen.agent_id == agent_id && seen.client_request_id == request_id) {
        return http(prior.status, "application/json", prior.body.clone(), false, None);
    }
    let Some(turn) = body.turn else {
        return fail(400, "validation_error", "Send the turn number you are answering.");
    };
    let Some(kind) = body.action.as_ref().and_then(|action| action.kind.as_deref()).map(str::trim).filter(|value| !value.is_empty()) else {
        return fail(400, "validation_error", "Send action.type.");
    };
    let Some(index) = state.book.matches.iter().position(|play| play.match_id == match_id) else {
        return fail(404, "not_found", "No match with that id.");
    };
    if state.book.matches[index].status != "live" {
        return fail(409, "match_over", "This match is over.");
    }
    let seat = state.book.matches[index].seats.iter().position(|seat| seat == &agent_id);
    let Some(seat) = seat else {
        return fail(403, "forbidden", "You are not seated in this match.");
    };
    if state.book.matches[index].turn != turn {
        return fail(409, "stale_turn", "The turn moved. Poll matches and send the current turn.");
    }
    let current = current_seat(&state.book.matches[index]);
    if current != seat {
        return fail(403, "not_your_turn", "Wait for your seat.");
    }
    let arena_id = state.book.matches[index].arena_id.clone();
    if !legal_actions(&arena_id).iter().any(|legal| *legal == kind) {
        return fail(400, "illegal_action", "That action is not legal in this arena.");
    }
    let opponent = state.book.matches[index]
        .actions
        .iter()
        .rev()
        .find(|action| action.agent_id != agent_id)
        .map(|action| action.action_type.clone());
    let points = score_kind(&arena_id, kind, opponent.as_deref());
    let play = &mut state.book.matches[index];
    play.actions.push(Played {
        agent_id: agent_id.clone(),
        turn,
        action_type: kind.to_string(),
        points,
        unix: now,
    });
    play.turn = play.turn.saturating_add(1);
    if play.turn as usize > play.seats.len() {
        play.status = "over".to_string();
    } else {
        play.deadline_unix = now.saturating_add(TURN_SECS);
    }
    let next = play.deadline_unix;
    let body = serde_json::json!({
        "accepted": true,
        "turn": turn,
        "points": points,
        "nextDeadlineAt": rfc3339(next),
    })
    .to_string();
    state.book.seen.push(Seen {
        agent_id,
        client_request_id: request_id.to_string(),
        status: 200,
        body: body.clone(),
    });
    http(200, "application/json", body, true, None)
}

fn pair_queue(state: &mut ArenaState, arena_id: &str, now: u64) {
    loop {
        let waiting: Vec<usize> = state
            .book
            .queue
            .iter()
            .enumerate()
            .filter(|(_, seat)| seat.arena_id == arena_id)
            .map(|(index, _)| index)
            .take(2)
            .collect();
        if waiting.len() < 2 {
            break;
        }
        let second = state.book.queue.remove(waiting[1]);
        let first = state.book.queue.remove(waiting[0]);
        state.book.matches.push(PlayMatch {
            match_id: format!("mt_{}", random_hex(8)),
            arena_id: arena_id.to_string(),
            status: "live".to_string(),
            turn: 1,
            deadline_unix: now.saturating_add(TURN_SECS),
            seats: vec![first.agent_id, second.agent_id],
            actions: Vec::new(),
            created_unix: now,
        });
    }
}

fn settle(state: &mut ArenaState, now: u64) -> bool {
    let mut changed = false;
    for play in &mut state.book.matches {
        while play.status == "live" && now >= play.deadline_unix {
            let seat = current_seat(play);
            let agent_id = play.seats.get(seat).cloned().unwrap_or_default();
            let turn = play.turn;
            play.actions.push(Played {
                agent_id,
                turn,
                action_type: "pass".to_string(),
                points: 0,
                unix: now,
            });
            changed = true;
            play.turn = play.turn.saturating_add(1);
            if play.turn as usize > play.seats.len() {
                play.status = "over".to_string();
            } else {
                play.deadline_unix = now.saturating_add(TURN_SECS);
            }
        }
    }
    changed
}

fn current_seat(play: &PlayMatch) -> usize {
    if play.seats.is_empty() {
        return 0;
    }
    play.turn.saturating_sub(1) as usize % play.seats.len()
}

fn match_for_seat(state: &ArenaState, play: &PlayMatch, agent_id: Option<&str>, _now: u64) -> serde_json::Value {
    let your_seat = agent_id.and_then(|id| play.seats.iter().position(|seat| seat == id));
    let info = arena(&play.arena_id);
    serde_json::json!({
        "match": {
            "matchId": play.match_id,
            "arenaId": play.arena_id,
            "status": play.status,
            "turn": play.turn,
            "deadlineAt": rfc3339(play.deadline_unix),
            "yourSeat": your_seat,
        },
        "arena": {
            "name": info.map(|row| row.name).unwrap_or("Arena"),
            "rulesUrl": info.map(|row| row.rules_url).unwrap_or(SITE_ORIGIN),
        },
        "state": {
            "scores": play.seats.iter().map(|seat| serde_json::json!({
                "handle": handle_of(state, seat),
                "points": play.actions.iter().filter(|action| action.agent_id == *seat).map(|action| action.points).sum::<u32>(),
            })).collect::<Vec<_>>(),
        },
        "legalActions": legal_actions(&play.arena_id),
    })
}

fn rank_of(state: &ArenaState, handle: &str, now: u64) -> u32 {
    let mut rows = fe2vi_core::public_leaderboard(now, BoardWindow::All, None);
    rows.extend(registered_rows(state, now, BoardWindow::All, None));
    rows.sort_by(|left, right| right.points.cmp(&left.points).then_with(|| left.name.cmp(&right.name)).then_with(|| left.agent.cmp(&right.agent)));
    let mut last_points = None;
    let mut last_rank = 0u32;
    for (index, row) in rows.iter().enumerate() {
        let rank = if last_points == Some(row.points) { last_rank } else { index as u32 + 1 };
        last_points = Some(row.points);
        last_rank = rank;
        if row.agent == handle {
            return rank;
        }
    }
    0
}

struct ArenaInfo {
    id: &'static str,
    name: &'static str,
    rules_url: &'static str,
    summary: &'static str,
}

fn arenas() -> &'static [ArenaInfo] {
    &[
        ArenaInfo { id: "vault", name: "Vault", rules_url: "https://fe2vi.space/games/heist-night/GAME.md", summary: "Scout, hide, steal, or sabotage. One action on your turn." },
        ArenaInfo { id: "cup", name: "Cup", rules_url: "https://fe2vi.space/games/trading-cup/GAME.md", summary: "Play a round." },
        ArenaInfo { id: "village", name: "Trading Village", rules_url: "https://fe2vi.space/village", summary: "Fill once on your turn." },
        ArenaInfo { id: "war", name: "War Room", rules_url: "https://fe2vi.space/war-room", summary: "Claim a sandbox flag this arena hosts." },
    ]
}

fn arena(id: &str) -> Option<&'static ArenaInfo> {
    arenas().iter().find(|row| row.id == id)
}

fn arena_one(id: &str) -> Outcome {
    let Some(row) = arena(id) else {
        return fail(404, "not_found", "Unknown arena.");
    };
    json(200, &arena_json(row), false, None)
}

fn arenas_body() -> serde_json::Value {
    serde_json::json!({ "arenas": arenas().iter().map(arena_json).collect::<Vec<_>>() })
}

fn arena_json(row: &ArenaInfo) -> serde_json::Value {
    serde_json::json!({
        "arenaId": row.id,
        "name": row.name,
        "rulesUrl": row.rules_url,
        "summary": row.summary,
        "legalActions": legal_actions(row.id),
    })
}

fn legal_actions(arena_id: &str) -> Vec<&'static str> {
    match arena_id {
        "vault" => vec!["scout", "hide", "steal", "sabotage"],
        "cup" => vec!["round"],
        "village" => vec!["fill"],
        "war" => challenges().iter().map(|challenge| challenge.id).collect(),
        _ => Vec::new(),
    }
}

fn score_kind(arena_id: &str, kind: &str, opponent_last: Option<&str>) -> u32 {
    match arena_id {
        "vault" => vault_points(kind, opponent_last).unwrap_or(0),
        "cup" if kind == "round" => POINT_SCOUT,
        "village" if kind == "fill" => POINT_SCOUT,
        "war" => challenges().iter().find(|challenge| challenge.id == kind).map(|challenge| challenge.points).unwrap_or(0),
        _ => 0,
    }
}

fn vault_points(kind: &str, opponent_last: Option<&str>) -> Option<u32> {
    let action_kind = match kind {
        "scout" => ActionKind::Scout,
        "hide" => ActionKind::Hide,
        "steal" => ActionKind::Steal,
        "sabotage" => ActionKind::Sabotage,
        _ => return None,
    };
    let mut actions = Vec::new();
    if action_kind == ActionKind::Sabotage && opponent_last == Some("steal") {
        actions.push(SubmittedAction { player: "other".into(), kind: ActionKind::Steal, target: None });
        actions.push(SubmittedAction { player: "self".into(), kind: ActionKind::Sabotage, target: Some("other".into()) });
    } else {
        let target = if action_kind == ActionKind::Sabotage { Some("other".to_string()) } else { None };
        actions.push(SubmittedAction { player: "self".into(), kind: action_kind, target });
    }
    let scores = score_actions(&actions);
    Some(u32::try_from(*scores.get("self").unwrap_or(&0)).unwrap_or(0))
}

fn arena_room(arena_id: &str) -> String {
    match arena_id {
        "vault" | "cup" | "village" | "war" => arena_id.to_string(),
        _ => "vault".to_string(),
    }
}

fn handle_of(state: &ArenaState, agent_id: &str) -> String {
    state.book.players.iter().find(|player| player.agent_id == agent_id).map(|player| player.handle.clone()).unwrap_or_else(|| agent_id.to_string())
}

fn authed_id(state: &ArenaState, incoming: &Incoming<'_>) -> Option<String> {
    let token = bearer(incoming.authorization)?;
    let hash = token_hash(token);
    state.book.players.iter().find(|player| player.token_hash == hash).map(|player| player.agent_id.clone())
}

fn authed_mut<'a>(state: &'a mut ArenaState, incoming: &Incoming<'_>) -> Option<&'a mut Player> {
    let token = bearer(incoming.authorization)?;
    let hash = token_hash(token);
    state.book.players.iter_mut().find(|player| player.token_hash == hash)
}

fn bearer(header: Option<&str>) -> Option<&str> {
    let value = header?.strip_prefix("Bearer ")?.trim();
    token_shape(value).then_some(value)
}

fn token_shape(token: &str) -> bool {
    let Some(hex) = token.strip_prefix("fe2_") else { return false };
    hex.len() == 32 && hex.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn token_hash(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

fn is_handle(handle: &str) -> bool {
    let bytes = handle.as_bytes();
    (3..=30).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_request_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=80).contains(&bytes.len()) && bytes.iter().all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

fn clean_text(value: Option<&str>, min: usize, max: usize) -> Option<String> {
    let text = value?.trim();
    let count = text.chars().count();
    if count < min || count > max || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(text.to_string())
}

fn fail(status: u16, code: &str, message: &str) -> Outcome {
    json(status, &serde_json::json!({ "error": code, "message": message }), false, None)
}

fn document(content_type: &'static str, body: &str) -> Outcome {
    http(200, content_type, body.to_string(), false, None)
}

fn path_only(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn fresh_token() -> String {
    format!("fe2_{}", random_hex(16))
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    fill_random(&mut buf);
    to_hex(&buf)
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn fill_random(buf: &mut [u8]) {
    #[cfg(target_arch = "wasm32")]
    {
        use worker::wasm_bindgen::JsCast;
        let global = worker::js_sys::global();
        let crypto = worker::js_sys::Reflect::get(&global, &worker::wasm_bindgen::JsValue::from_str("crypto")).expect("crypto");
        let array = worker::js_sys::Uint8Array::new_with_length(buf.len() as u32);
        let method = worker::js_sys::Reflect::get(&crypto, &worker::wasm_bindgen::JsValue::from_str("getRandomValues")).expect("getRandomValues");
        let func = method.unchecked_into::<worker::js_sys::Function>();
        let _ = func.call1(&crypto, &array);
        array.copy_to(buf);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom").expect("urandom").read_exact(buf).expect("urandom");
    }
}

pub fn rfc3339(unix: u64) -> String {
    let days = unix / 86_400;
    let secs = unix % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z",
        hour = secs / 3600,
        minute = (secs % 3600) / 60,
        second = secs % 60,
    )
}

fn civil_from_days(days: u64) -> (i32, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

fn openapi() -> String {
    serde_json::json!({
        "openapi": "3.0.3",
        "info": { "title": "fe2VI", "version": "0.1.0" },
        "servers": [{ "url": API_ORIGIN }],
        "components": {
            "securitySchemes": { "bearer": { "type": "http", "scheme": "bearer" } },
            "schemas": {
                "Error": {
                    "type": "object",
                    "properties": {
                        "error": { "type": "string" },
                        "message": { "type": "string" }
                    }
                }
            }
        },
        "paths": {
            "/v1/agents/register": { "post": { "operationId": "register" } },
            "/v1/agents/me": { "get": { "operationId": "me", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/rotate-token": { "post": { "operationId": "rotateToken", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/wallet": { "post": { "operationId": "linkWallet", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/skin": { "post": { "operationId": "equipSkin", "security": [{ "bearer": [] }] } },
            "/v1/skins": { "get": { "operationId": "skins" } },
            "/v1/agents/me/matches": { "get": { "operationId": "myMatches", "security": [{ "bearer": [] }] } },
            "/v1/matches": { "get": { "operationId": "recentMatches" } },
            "/v1/markets": { "get": { "operationId": "markets" } },
            "/v1/markets/candles": { "get": { "operationId": "candles" } },
            "/v1/accounts/hyperliquid": { "get": { "operationId": "hyperliquidAccount" } },
            "/v1/agents/{handle}": { "get": { "operationId": "agent" } },
            "/v1/arenas": { "get": { "operationId": "arenas" } },
            "/v1/arenas/{arenaId}": { "get": { "operationId": "arena" } },
            "/v1/arenas/{arenaId}/queue": {
                "post": { "operationId": "joinQueue", "security": [{ "bearer": [] }] },
                "delete": { "operationId": "leaveQueue", "security": [{ "bearer": [] }] }
            },
            "/v1/matches/{matchId}": { "get": { "operationId": "match" } },
            "/v1/matches/{matchId}/actions": { "post": { "operationId": "act", "security": [{ "bearer": [] }] } },
            "/v1/leaderboard": { "get": { "operationId": "leaderboard" } },
            "/openapi.json": { "get": { "operationId": "openapi" } },
            "/llms.txt": { "get": { "operationId": "llms" } }
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{dispatch, Incoming};

    const NOW: u64 = 1_790_900_000;

    fn call<'a>(state: &mut ArenaState, method: &'a str, path: &'a str, body: &'a str, token: Option<&'a str>) -> (u16, serde_json::Value) {
        let auth = token.map(|token| format!("Bearer {token}"));
        let outcome = dispatch(
            state,
            &Incoming {
                method,
                path,
                body,
                authorization: auth.as_deref(),
                ingest_token: None,
            },
        );
        match outcome {
            Outcome::Http { status, body, .. } => (status, serde_json::from_str(&body).unwrap_or(serde_json::Value::Null)),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    fn register_one(state: &mut ArenaState, handle: &str) -> String {
        let body = format!(r#"{{"handle":"{handle}","name":"{handle}","description":"plays"}}"#);
        let path = format!("/v1/agents/register?now={NOW}");
        let (status, json) = call(state, "POST", &path, &body, None);
        assert_eq!(status, 201, "{json}");
        assert_eq!(json["points"], 0);
        assert!(json["apiToken"].as_str().unwrap().starts_with("fe2_"));
        assert!(!json.to_string().contains(&json["apiToken"].as_str().unwrap().replace("fe2_", "stored")));
        json["apiToken"].as_str().unwrap().to_string()
    }

    #[test]
    fn wallet_links_are_public_and_validated() {
        let mut state = ArenaState::default();
        let body = r#"{"handle":"linker","name":"Linker","description":"trades","wallets":{"nearAccount":"linker.near","evmAddress":"0xABCDEF0123456789ABCDEF0123456789ABCDEF01","hyperliquidAccount":"0x1111111111111111111111111111111111111111"},"capabilities":["games","live-trading","nonsense"]}"#;
        let (status, json) = call(&mut state, "POST", &format!("/v1/agents/register?now={NOW}"), body, None);
        assert_eq!(status, 201, "{json}");
        assert_eq!(json["wallets"]["nearAccount"], "linker.near");
        assert_eq!(json["wallets"]["evmAddress"], "0xabcdef0123456789abcdef0123456789abcdef01");
        assert_eq!(json["capabilities"], serde_json::json!(["games", "live-trading"]));
        let token = json["apiToken"].as_str().unwrap().to_string();

        let (bad, err) = call(&mut state, "POST", &format!("/v1/agents/me/wallet?now={NOW}"), r#"{"evmAddress":"0x1234"}"#, Some(&token));
        assert_eq!(bad, 400);
        assert_eq!(err["error"], "validation_error");

        let (ok, json) = call(&mut state, "POST", &format!("/v1/agents/me/wallet?now={NOW}"), r#"{"nearAccount":"new.near"}"#, Some(&token));
        assert_eq!(ok, 200, "{json}");
        assert_eq!(json["wallets"]["nearAccount"], "new.near");
        assert_eq!(json["wallets"]["evmAddress"], "0xabcdef0123456789abcdef0123456789abcdef01");
        assert_eq!(json["tradeMode"], "paper");

        let (anon, _) = call(&mut state, "POST", &format!("/v1/agents/me/wallet?now={NOW}"), r#"{"nearAccount":"x.near"}"#, None);
        assert_eq!(anon, 401);
    }

    #[test]
    fn skins_unlock_with_points() {
        let mut state = ArenaState::default();
        let body = r#"{"handle":"forger","name":"Forger","description":"forges"}"#;
        let (status, json) = call(&mut state, "POST", &format!("/v1/agents/register?now={NOW}"), body, None);
        assert_eq!(status, 201, "{json}");
        let token = json["apiToken"].as_str().unwrap().to_string();
        assert_eq!(json["skin"], serde_json::Value::Null);

        let (locked, err) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/skin?now={NOW}"),
            r#"{"skinId":"gold-leaf"}"#,
            Some(&token),
        );
        assert_eq!(locked, 403);
        assert_eq!(err["error"], "skin_locked");

        let agent_id = state.book.players[0].agent_id.clone();
        state.book.matches.push(PlayMatch {
            match_id: "mt_skin".into(),
            arena_id: "vault".into(),
            status: "over".into(),
            turn: 3,
            deadline_unix: NOW,
            seats: vec![agent_id.clone(), "other".into()],
            actions: vec![Played {
                agent_id: agent_id.clone(),
                turn: 1,
                action_type: "steal".into(),
                points: 600,
                unix: NOW,
            }],
            created_unix: NOW,
        });

        let (ok, json) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/skin?now={NOW}"),
            r#"{"skinId":"gold-leaf"}"#,
            Some(&token),
        );
        assert_eq!(ok, 200, "{json}");
        assert_eq!(json["skin"]["id"], "gold-leaf");
        assert_eq!(json["skin"]["rarity"], "Mythic");
        assert_eq!(state.book.players[0].skin_id, "gold-leaf");

        let (public, agent) = call(&mut state, "GET", &format!("/v1/agents/forger?now={NOW}"), "", None);
        assert_eq!(public, 200);
        assert_eq!(agent["skin"]["id"], "gold-leaf");

        let (unknown, err) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/skin?now={NOW}"),
            r#"{"skinId":"nope"}"#,
            Some(&token),
        );
        assert_eq!(unknown, 404);
        assert_eq!(err["error"], "not_found");
    }

    #[test]
    fn register_queue_poll_act_and_score() {
        let mut state = ArenaState::default();
        let (bad, err) = call(&mut state, "POST", "/v1/agents/register", r#"{"handle":"No","name":"A","description":""}"#, None);
        assert_eq!(bad, 400);
        assert_eq!(err["error"], "validation_error");
        let alpha = register_one(&mut state, "alpha");
        let again = call(&mut state, "POST", "/v1/agents/register", r#"{"handle":"alpha","name":"Other","description":""}"#, None);
        assert_eq!(again.0, 409);
        assert_eq!(again.1["error"], "handle_taken");
        assert!(state.book.players[0].token_hash != alpha);
        assert_eq!(state.book.players[0].token_hash.len(), 64);

        let (denied, _) = call(&mut state, "GET", "/v1/agents/me", "", Some("fe2_0123456789abcdef0123456789abcdef"));
        assert_eq!(denied, 401);

        let me = call(&mut state, "GET", &format!("/v1/agents/me?now={NOW}"), "", Some(&alpha));
        assert_eq!(me.0, 200);
        assert_eq!(me.1["handle"], "alpha");
        assert_eq!(me.1["points"], 0);

        let bravo = register_one(&mut state, "bravo");
        let queue = format!("/v1/arenas/vault/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&alpha)).0, 200);
        assert!(state.book.matches.is_empty());
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&bravo)).0, 200);
        assert_eq!(state.book.matches.len(), 1);

        let listed = call(&mut state, "GET", &format!("/v1/agents/me/matches?now={NOW}"), "", Some(&alpha));
        assert_eq!(listed.0, 200);
        let play = &listed.1["matches"][0];
        assert_eq!(play["match"]["yourSeat"], 0);
        assert_eq!(play["match"]["turn"], 1);
        assert_eq!(play["arena"]["name"], "Vault");
        assert!(play["legalActions"].as_array().unwrap().iter().any(|item| item == "scout"));
        let match_id = play["match"]["matchId"].as_str().unwrap().to_string();

        let act_path = format!("/v1/matches/{match_id}/actions?now={NOW}");
        let acted = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"scout"},"clientRequestId":"req-1"}"#,
            Some(&alpha),
        );
        assert_eq!(acted.0, 200, "{}", acted.1);
        assert_eq!(acted.1["accepted"], true);
        assert_eq!(acted.1["points"], fe2vi_core::POINT_SCOUT);
        let replay = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"scout"},"clientRequestId":"req-1"}"#,
            Some(&alpha),
        );
        assert_eq!(replay.0, 200);
        assert_eq!(state.book.matches[0].actions.len(), 1);
        let stale = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"steal"},"clientRequestId":"req-2"}"#,
            Some(&bravo),
        );
        assert_eq!(stale.0, 409);
        assert_eq!(stale.1["error"], "stale_turn");
        let scored = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":2,"action":{"type":"steal"},"clientRequestId":"req-3"}"#,
            Some(&bravo),
        );
        assert_eq!(scored.0, 200);
        assert_eq!(scored.1["points"], fe2vi_core::POINT_STEAL);

        let board = call(&mut state, "GET", &format!("/v1/leaderboard?window=all&now={NOW}"), "", None);
        assert_eq!(board.0, 200);
        let rows = board.1["rows"].as_array().unwrap();
        let alpha_row = rows.iter().find(|row| row["agent"] == "alpha").unwrap();
        let bravo_row = rows.iter().find(|row| row["agent"] == "bravo").unwrap();
        assert_eq!(alpha_row["points"], fe2vi_core::POINT_SCOUT);
        assert_eq!(bravo_row["points"], fe2vi_core::POINT_STEAL);
        assert_eq!(alpha_row["vault"], fe2vi_core::POINT_SCOUT);
        assert!(rows.iter().any(|row| row["name"] == "Mason"));

        let profile = call(&mut state, "GET", &format!("/v1/agents/alpha?now={NOW}"), "", None);
        assert_eq!(profile.0, 200);
        assert_eq!(profile.1["points"], fe2vi_core::POINT_SCOUT);
        assert!(profile.1.get("apiToken").is_none());

        let spec = call(&mut state, "GET", "/openapi.json", "", None);
        assert_eq!(spec.0, 200);
        let text = spec.1.to_string();
        assert!(!text.contains("429"));
        for path in ["/v1/agents/register", "/v1/agents/me/matches", "/v1/matches/{matchId}/actions", "/llms.txt"] {
            assert!(text.contains(path), "{path}");
        }
        let skill = dispatch(
            &mut state,
            &Incoming { method: "GET", path: "/llms.txt", body: "", authorization: None, ingest_token: None },
        );
        match skill {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 200);
                assert!(body.contains("https://fe2vi.space/skill/SKILL.md"));
                assert!(body.contains("https://api.fe2vi.space"));
                assert!(!body.contains("fe2vi.xyz"));
                assert!(!body.to_ascii_lowercase().contains("rate limit"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn deadlines_pass_and_tokens_rotate() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let bravo = register_one(&mut state, "bravo");
        let queue = format!("/v1/arenas/cup/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&alpha)).0, 200);
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&bravo)).0, 200);
        let later = NOW + TURN_SECS + 5;
        let listed = call(&mut state, "GET", &format!("/v1/agents/me/matches?now={later}"), "", Some(&bravo));
        assert_eq!(listed.1["matches"][0]["match"]["turn"], 2);
        assert_eq!(state.book.matches[0].actions[0].action_type, "pass");
        let rotated = call(&mut state, "POST", "/v1/agents/me/rotate-token", "{}", Some(&alpha));
        assert_eq!(rotated.0, 200);
        let next = rotated.1["apiToken"].as_str().unwrap().to_string();
        assert_ne!(next, alpha);
        assert_eq!(call(&mut state, "GET", "/v1/agents/me", "", Some(&alpha)).0, 401);
        assert_eq!(call(&mut state, "GET", "/v1/agents/me", "", Some(&next)).0, 200);
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    }
}

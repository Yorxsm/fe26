//! Registered agents. Tokens are stored as sha256 only. Points come from the
//! same constants the mock crew uses. The player book is persisted as SQL rows.

use fe2vi_core::{
    award_in_window, challenges, score_actions, ActionKind, BoardWindow, PointRoom, PublicRow,
    SubmittedAction,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::logic::{http, json, query_param, ArenaState, Fanout, Incoming, Outcome};
use crate::skins;
use crate::vault::VaultMatch;
use std::collections::{HashMap, HashSet};

const TURN_SECS: u64 = 60;
const API_ORIGIN: &str = "https://api.fe2vi.space";
const SITE_ORIGIN: &str = "https://fe2vi.space";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PlayerBook {
    #[serde(default)]
    pub players: Vec<Player>,
    #[serde(default)]
    pub queue: Vec<Queued>,
    #[serde(default)]
    pub matches: Vec<PlayMatch>,
    #[serde(default)]
    pub seen: Vec<Seen>,
    #[serde(default)]
    pub war_claims: Vec<WarClaim>,
    #[serde(default)]
    pub lobby: Vec<LobbyPost>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub passports: Vec<Passport>,
    #[serde(default)]
    pub passport_jobs: Vec<PassportJob>,
}

/// Off-chain mirror of one passport NFT. The chain is authoritative; this row
/// lets the arena gate scored entry points without an RPC call per request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Passport {
    pub passport_id: String,
    #[serde(default)]
    pub token_id: Option<String>,
    pub agent_id: String,
    pub owner_near: String,
    pub creator_near: String,
    pub status: String,
    pub created_unix: u64,
    pub updated_unix: u64,
    #[serde(default)]
    pub last_sync_seq: u64,
    /// Chain career snapshot supplied by the relay. Not a score the API invented.
    #[serde(default)]
    pub career: Option<serde_json::Value>,
}

/// Work queue for the passport relay. Jobs are written by the API and claimed
/// by the signer; the API never holds a NEAR private key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PassportJob {
    pub job_id: String,
    pub kind: String,
    pub passport_id: String,
    pub agent_id: String,
    pub owner_near: String,
    pub agent_near: String,
    pub status: String,
    pub attempts: u32,
    pub created_unix: u64,
    pub updated_unix: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Unix timestamps of War proof attempts, including misses.
    #[serde(default)]
    pub war_hits: Vec<u64>,
}

fn default_skin_id() -> String {
    "ingot".to_string()
}

/// Public chain addresses only. The arena never stores a private key, an API
/// key, or a grant token for an agent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wallets {
    #[serde(default)]
    pub evm_address: Option<String>,
    #[serde(default)]
    pub near_account: Option<String>,
    #[serde(default)]
    pub hyperliquid_account: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Queued {
    pub arena_id: String,
    pub agent_id: String,
    pub queued_unix: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub vault: Option<VaultMatch>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Played {
    pub agent_id: String,
    pub turn: u32,
    pub action_type: String,
    pub points: u32,
    pub unix: u64,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    pub agent_id: String,
    pub client_request_id: String,
    pub status: u16,
    pub body: String,
    /// Set for match actions so the row can be dropped when that match ends.
    #[serde(default)]
    pub match_id: String,
    /// Unix seconds. Zero means a legacy row with no age, which is kept.
    #[serde(default)]
    pub created_unix: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WarClaim {
    pub agent_id: String,
    pub challenge_id: String,
    pub stage: String,
    pub points: u32,
    pub unix: u64,
    pub proof_hash: String,
    #[serde(default)]
    pub intent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyPost {
    pub id: String,
    pub agent_id: String,
    pub kind: String,
    pub topic: String,
    pub body: String,
    pub match_id: Option<String>,
    pub unix: u64,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub reports: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub target_type: String,
    pub target_id: String,
    pub viewer_id: String,
    pub reaction: String,
    pub unix: u64,
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
    target: Option<String>,
    intent: Option<String>,
}

#[derive(Deserialize)]
struct WarSubmitBody {
    proof: Option<String>,
    #[serde(rename = "clientRequestId")]
    client_request_id: Option<String>,
    intent: Option<String>,
}

#[derive(Deserialize)]
struct ReactionBody {
    reaction: Option<String>,
    #[serde(rename = "viewerId")]
    viewer_id: Option<String>,
}

#[derive(Deserialize)]
struct ReportBody {
    #[serde(rename = "viewerId")]
    viewer_id: Option<String>,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct LobbyPostBody {
    kind: Option<String>,
    topic: Option<String>,
    body: Option<String>,
    #[serde(rename = "matchId")]
    match_id: Option<String>,
    #[serde(rename = "clientRequestId")]
    client_request_id: Option<String>,
}

pub fn dispatch_player(state: &mut ArenaState, incoming: &Incoming<'_>) -> Option<Outcome> {
    let path = path_only(incoming.path);
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let now = crate::logic::query_now(incoming.path);
    let changed = settle(state, now) || prune_seen(state, now);
    let outcome = match (incoming.method, parts.as_slice()) {
        ("GET", ["openapi.json"]) => Some(document("application/json", &openapi())),
        ("GET", ["llms.txt"]) => Some(document("text/plain; charset=utf-8", onboarding_text())),
        ("GET", ["v1", "arenas"]) => Some(json(200, &arenas_body(), false, None)),
        ("GET", ["v1", "arenas", id]) => Some(arena_one(id)),
        ("POST", ["v1", "agents", "register"]) => Some(register(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "rotate-token"]) => Some(rotate(state, incoming)),
        ("POST", ["v1", "agents", "me", "wallet"]) => Some(link_wallet(state, incoming, now)),
        ("POST", ["v1", "agents", "me", "skin"]) => Some(equip_skin(state, incoming, now)),
        ("GET", ["v1", "war", "challenges", "sandbox-page"]) => Some(sandbox_page()),
        ("GET", ["v1", "war", "challenges", "sandbox-page", "flag"]) => {
            Some(sandbox_flag(state, incoming))
        }
        ("GET", ["v1", "war", "challenges", "referee"]) => Some(referee_brief()),
        ("POST", ["v1", "war", "challenges", "referee", "solve"]) => {
            Some(referee_solve(state, incoming))
        }
        ("GET", ["v1", "war", "challenges", "hash-puzzle"]) => Some(hash_puzzle(state, incoming)),
        ("POST", ["v1", "war", "challenges", id, "submit"]) => {
            Some(submit_war(state, incoming, id, now))
        }
        ("GET", ["v1", "lobby", "posts"]) => Some(lobby_posts(state, incoming)),
        ("GET", ["v1", "lobby", "ws"]) => Some(Outcome::Websocket {
            game_id: "lobby".to_string(),
            history: lobby_history(state),
        }),
        ("POST", ["v1", "lobby", "posts"]) => Some(post_lobby(state, incoming, now)),
        ("POST", ["v1", "lobby", "posts", id, "react"]) => {
            Some(react(state, incoming, "lobby_post", id, now))
        }
        ("POST", ["v1", "lobby", "posts", id, "report"]) => Some(report_lobby(state, incoming, id)),
        ("DELETE", ["v1", "lobby", "posts", id]) => Some(delete_lobby(state, incoming, id)),
        ("POST", ["v1", "matches", id, "react"]) => Some(react(state, incoming, "match", id, now)),
        ("GET", ["v1", "agents", "me"]) => Some(me(state, incoming, now)),
        ("GET", ["v1", "skins"]) => Some(json(
            200,
            &serde_json::json!({ "skins": skins::catalog().iter().map(skins::skin_json).collect::<Vec<_>>() }),
            false,
            None,
        )),
        ("GET", ["v1", "agents", "me", "matches"]) => Some(my_matches(state, incoming, now)),
        ("GET", ["v1", "agents"]) => Some(agent_index(state, incoming, now)),
        ("GET", ["v1", "agents", handle]) if is_handle(handle) => {
            Some(public_agent(state, handle, now))
        }
        ("GET", ["v1", "matches"]) => Some(recent_matches(state, incoming)),
        ("POST", ["v1", "arenas", id, "queue"]) => Some(join_queue(state, incoming, id, now)),
        ("DELETE", ["v1", "arenas", id, "queue"]) => Some(leave_queue(state, incoming, id)),
        ("GET", ["v1", "matches", id]) => Some(spectate(state, incoming, id)),
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
        Outcome::Http {
            status,
            content_type,
            body,
            fanout,
            location,
            ..
        } => Outcome::Http {
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

pub fn registered_rows(
    state: &ArenaState,
    now: u64,
    window: BoardWindow,
    room: Option<PointRoom>,
) -> Vec<PublicRow> {
    state
        .book
        .players
        .iter()
        .filter_map(|player| {
            if !crate::passport::scored(state, &player.agent_id) {
                return None;
            }
            let scores = room_scores(state, &player.agent_id, now, window);
            let (points, games, wins) = match room {
                Some(PointRoom::Vault) => (scores.vault, scores.vault_games, scores.vault_wins),
                Some(PointRoom::Cup) => (scores.cup, scores.cup_games, scores.cup_wins),
                Some(PointRoom::Village) => {
                    (scores.village, scores.village_games, scores.village_wins)
                }
                Some(PointRoom::War) => (scores.war, scores.war_games, scores.war_wins),
                None => (scores.total, scores.games, scores.wins),
            };
            // A War flag adds points and is not a match, so games can be 0.
            if points == 0 {
                return None;
            }
            Some(PublicRow {
                rank: 0,
                agent: player.handle.clone(),
                name: player.name.clone(),
                colour: "#d4d2cc".to_string(),
                points,
                games,
                wins,
                win_rate: fe2vi_core::win_rate(wins, games),
                vault: scores.vault,
                cup: scores.cup,
                village: scores.village,
                war: scores.war,
            })
        })
        .collect()
}

pub fn registered_recent(
    state: &ArenaState,
    handle: &str,
    now: u64,
    window: BoardWindow,
) -> Option<Vec<serde_json::Value>> {
    let player = state
        .book
        .players
        .iter()
        .find(|player| player.handle == handle)?;
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
    for claim in &state.book.war_claims {
        if claim.agent_id != player.agent_id || !award_in_window(now, window, claim.unix) {
            continue;
        }
        rows.push(serde_json::json!({
            "room": "war",
            "points": claim.points,
            "unix": claim.unix,
            "label": format!("war:{}:{}", claim.challenge_id, claim.stage),
        }));
    }
    for award in &state.live.awards {
        if award.agent_id != player.agent_id || !award_in_window(now, window, award.unix) {
            continue;
        }
        rows.push(serde_json::json!({
            "room": award.room,
            "points": award.points,
            "unix": award.unix,
            "label": award.label,
        }));
    }
    rows.sort_by(|left, right| right["unix"].as_u64().cmp(&left["unix"].as_u64()));
    rows.truncate(8);
    Some(rows)
}

struct Scores {
    total: u32,
    games: u32,
    wins: u32,
    vault: u32,
    cup: u32,
    village: u32,
    war: u32,
    vault_games: u32,
    cup_games: u32,
    village_games: u32,
    war_games: u32,
    vault_wins: u32,
    cup_wins: u32,
    village_wins: u32,
    war_wins: u32,
}

fn add_result(scores: &mut Scores, room: &str, points: u32) {
    let win = points == fe2vi_core::MATCH_WIN && room != "war";
    scores.games = scores.games.saturating_add(1);
    scores.total = scores.total.saturating_add(points);
    if win {
        scores.wins = scores.wins.saturating_add(1);
    }
    match room {
        "cup" => {
            scores.cup = scores.cup.saturating_add(points);
            scores.cup_games = scores.cup_games.saturating_add(1);
            if win {
                scores.cup_wins = scores.cup_wins.saturating_add(1);
            }
        }
        "village" => {
            scores.village = scores.village.saturating_add(points);
            scores.village_games = scores.village_games.saturating_add(1);
            if win {
                scores.village_wins = scores.village_wins.saturating_add(1);
            }
        }
        "war" => {
            scores.war = scores.war.saturating_add(points);
            scores.war_games = scores.war_games.saturating_add(1);
        }
        _ => {
            scores.vault = scores.vault.saturating_add(points);
            scores.vault_games = scores.vault_games.saturating_add(1);
            if win {
                scores.vault_wins = scores.vault_wins.saturating_add(1);
            }
        }
    }
}

fn seat_total(play: &PlayMatch, agent_id: &str) -> i32 {
    if let Some(vault) = &play.vault {
        return vault.score(agent_id);
    }
    play.actions
        .iter()
        .filter(|action| action.agent_id == agent_id)
        .map(|action| action.points as i32)
        .sum()
}

fn room_scores(state: &ArenaState, agent_id: &str, now: u64, window: BoardWindow) -> Scores {
    let mut scores = Scores {
        total: 0,
        games: 0,
        wins: 0,
        vault: 0,
        cup: 0,
        village: 0,
        war: 0,
        vault_games: 0,
        cup_games: 0,
        village_games: 0,
        war_games: 0,
        vault_wins: 0,
        cup_wins: 0,
        village_wins: 0,
        war_wins: 0,
    };
    for play in &state.book.matches {
        let seated = play.seats.iter().any(|seat| seat == agent_id)
            || play
                .vault
                .as_ref()
                .is_some_and(|vault| vault.seats.iter().any(|seat| seat.agent_id == agent_id));
        if !seated {
            continue;
        }
        let finished =
            play.status == "over" || play.vault.as_ref().is_some_and(|vault| vault.ended);
        if !finished {
            continue;
        }
        let at = play
            .actions
            .last()
            .map(|action| action.unix)
            .unwrap_or(play.created_unix);
        if !award_in_window(now, window, at) {
            continue;
        }
        let mine = seat_total(play, agent_id);
        let best_other = play
            .seats
            .iter()
            .filter(|seat| seat.as_str() != agent_id)
            .map(|seat| seat_total(play, seat))
            .max()
            .unwrap_or(i32::MIN);
        add_result(
            &mut scores,
            &play.arena_id,
            fe2vi_core::result_points(mine, best_other),
        );
    }
    for claim in &state.book.war_claims {
        if claim.agent_id != agent_id || !award_in_window(now, window, claim.unix) {
            continue;
        }
        // A flag is not a match. Points still land in the War column.
        scores.total = scores.total.saturating_add(claim.points);
        scores.war = scores.war.saturating_add(claim.points);
    }
    for award in &state.live.awards {
        if award.agent_id != agent_id || !award_in_window(now, window, award.unix) {
            continue;
        }
        add_result(&mut scores, &award.room, award.points);
    }
    scores
}

pub(crate) fn clean_evm(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.len() == 42
        && value.starts_with("0x")
        && value[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(value.to_ascii_lowercase())
    } else {
        Err("EVM and Hyperliquid addresses are 0x plus 40 hex characters.")
    }
}

pub(crate) fn clean_near(value: &str) -> Result<String, &'static str> {
    let value = value.trim().to_ascii_lowercase();
    let ok = (2..=64).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && value
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric());
    if ok {
        Ok(value)
    } else {
        Err("NEAR accounts are 2 to 64 lowercase characters.")
    }
}

fn clean_wallets(body: Option<WalletBody>) -> Result<Wallets, &'static str> {
    let mut wallets = Wallets::default();
    let Some(body) = body else {
        return Ok(wallets);
    };
    wallets.evm_address = match body
        .evm_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(clean_evm(value)?),
        None => None,
    };
    wallets.near_account = match body
        .near_account
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(clean_near(value)?),
        None => None,
    };
    wallets.hyperliquid_account = match body
        .hyperliquid_account
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(clean_evm(value)?),
        None => None,
    };
    Ok(wallets)
}

fn clean_capabilities(input: Option<Vec<String>>) -> Vec<String> {
    const ALLOWED: &[&str] = &["games", "live-trading", "market-making", "research"];
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
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send handle, name, and description.",
            )
        }
    };
    let Some(handle) = body
        .handle
        .as_deref()
        .map(str::trim)
        .filter(|value| is_handle(value))
    else {
        return fail(
            400,
            "validation_error",
            "Handle is 3 to 30 characters, lowercase, starting with a letter.",
        );
    };
    let Some(name) = clean_text(body.name.as_deref(), 1, 40) else {
        return fail(
            400,
            "validation_error",
            "Name is required, up to 40 characters.",
        );
    };
    let Some(description) = clean_text(Some(body.description.as_deref().unwrap_or("")), 0, 280)
    else {
        return fail(
            400,
            "validation_error",
            "Description is up to 280 characters.",
        );
    };
    let owner = match body.owner_contact.as_deref() {
        None => None,
        Some(value) if value.trim().is_empty() => None,
        Some(value) => match clean_text(Some(value), 1, 120) {
            Some(text) => Some(text),
            None => {
                return fail(
                    400,
                    "validation_error",
                    "Owner contact is up to 120 characters.",
                )
            }
        },
    };
    let wallets = match clean_wallets(body.wallets) {
        Ok(wallets) => wallets,
        Err(message) => return fail(400, "validation_error", message),
    };
    let capabilities = clean_capabilities(body.capabilities);
    if state
        .book
        .players
        .iter()
        .any(|player| player.handle == handle)
    {
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
        war_hits: Vec::new(),
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
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
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
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send evmAddress, nearAccount, or hyperliquidAccount.",
            )
        }
    };
    let Some(player) = authed_mut(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
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
            "tradeMode": "live",
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
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: SkinBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send skinId."),
    };
    let Some(skin) = body
        .skin_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(skins::find)
    else {
        return fail(404, "not_found", "No skin with that id.");
    };
    let points = room_scores(state, &agent_id, now, BoardWindow::All).total;
    if points < skin.unlock_points {
        return fail(403, "skin_locked", "That skin needs more points.");
    }
    if let Some(player) = state
        .book
        .players
        .iter_mut()
        .find(|player| player.agent_id == agent_id)
    {
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
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let (agent_id, handle, name, status, wallets, capabilities, skin_id) = {
        let player = state
            .book
            .players
            .iter()
            .find(|player| player.agent_id == agent_id)
            .unwrap();
        (
            player.agent_id.clone(),
            player.handle.clone(),
            player.name.clone(),
            player.status.clone(),
            player.wallets.clone(),
            player.capabilities.clone(),
            player.skin_id.clone(),
        )
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
            "tradeMode": "live",
            "tradingAccount": state.live.public_link(&handle),
        }),
        false,
        None,
    )
}

fn agent_index(state: &ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let limit = query_param(incoming.path, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let mut rows: Vec<serde_json::Value> = state
        .book
        .players
        .iter()
        .filter_map(|player| {
            if !crate::passport::scored(state, &player.agent_id) {
                return None;
            }
            let scores = room_scores(state, &player.agent_id, now, BoardWindow::All);
            if scores.total == 0 {
                return None;
            }
            let skin = skins::find(&player.skin_id).unwrap_or_else(skins::default_skin);
            Some(serde_json::json!({
                "agentId": player.agent_id,
                "handle": player.handle,
                "name": player.name,
                "description": player.description,
                "points": scores.total,
                "rank": rank_of(state, &player.handle, now),
                "status": player.status,
                "capabilities": player.capabilities,
                "skin": skins::skin_json(skin),
                "wallets": player.wallets,
            }))
        })
        .collect();
    rows.sort_by(|left, right| {
        right["points"]
            .as_u64()
            .cmp(&left["points"].as_u64())
            .then_with(|| left["handle"].as_str().cmp(&right["handle"].as_str()))
    });
    rows.truncate(limit);
    json(200, &serde_json::json!({ "agents": rows }), false, None)
}

fn public_agent(state: &ArenaState, handle: &str, now: u64) -> Outcome {
    let Some(player) = state
        .book
        .players
        .iter()
        .find(|player| player.handle == handle)
        .cloned()
    else {
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
            "tradeMode": "live",
            "tradingAccount": state.live.public_link(handle),
            "profileUrl": format!("{SITE_ORIGIN}/agents/{handle}"),
        }),
        false,
        None,
    )
}

fn submit_war(
    state: &mut ArenaState,
    incoming: &Incoming<'_>,
    challenge_id: &str,
    now: u64,
) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    if let Some(outcome) = crate::passport::require_active(state, &agent_id) {
        return outcome;
    }
    let body: WarSubmitBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send proof and clientRequestId."),
    };
    let Some(request_id) = body
        .client_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| is_request_id(value))
    else {
        return fail(
            400,
            "validation_error",
            "clientRequestId is 1 to 80 letters, numbers, or dashes.",
        );
    };
    if let Some(prior) = state
        .book
        .seen
        .iter()
        .find(|seen| seen.agent_id == agent_id && seen.client_request_id == request_id)
    {
        return http(
            prior.status,
            "application/json",
            prior.body.clone(),
            false,
            None,
        );
    }
    if let Some(player) = state
        .book
        .players
        .iter_mut()
        .find(|player| player.agent_id == agent_id)
    {
        let window = crate::limits::WAR_SUBMIT_WINDOW_SECS;
        player
            .war_hits
            .retain(|unix| now.saturating_sub(*unix) < window);
        if player.war_hits.len() as u32 >= crate::limits::WAR_SUBMITS_PER_MINUTE {
            return fail(429, "rate_limited", "Too many War proofs. Wait and retry.");
        }
        player.war_hits.push(now);
    }
    let Some(challenge) = challenges().iter().find(|row| row.id == challenge_id) else {
        return fail(404, "not_found", "No such War challenge.");
    };
    if now < challenge.start_unix || now > challenge.end_unix {
        return fail(
            410,
            "challenge_expired",
            "That challenge is outside the current season.",
        );
    }
    let Some(proof) = body
        .proof
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return fail(400, "validation_error", "Send the challenge proof.");
    };
    if proof.chars().count() > 1024 {
        return fail(
            400,
            "validation_error",
            "The proof is longer than 1024 characters.",
        );
    }
    let Some(secret) = configured_war_secret() else {
        return fail(
            503,
            "war_unconfigured",
            "War flags are not configured. Set FE2VI_WAR_SECRET on the worker.",
        );
    };
    let expected = war_flag(&secret, challenge_id, &agent_id);
    if !ct_eq(proof, &expected) {
        return fail(
            403,
            "invalid_proof",
            "That proof does not match this challenge. Targets are fe2VI sandboxes only.",
        );
    }
    if state
        .book
        .war_claims
        .iter()
        .any(|claim| claim.agent_id == agent_id && claim.challenge_id == challenge_id)
    {
        return fail(
            409,
            "already_claimed",
            "This agent already claimed that challenge this season.",
        );
    }
    let intent = body
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(160).collect::<String>());
    let proof_hash = to_hex(&Sha256::digest(proof.as_bytes()));
    state.book.war_claims.push(WarClaim {
        agent_id: agent_id.clone(),
        challenge_id: challenge.id.to_string(),
        stage: "exploit".to_string(),
        points: challenge.points,
        unix: now,
        proof_hash,
        intent,
    });
    let body = serde_json::json!({
        "accepted": true,
        "challengeId": challenge.id,
        "stage": "exploit",
        "points": challenge.points,
    })
    .to_string();
    state.book.seen.push(Seen {
        agent_id,
        client_request_id: request_id.to_string(),
        status: 200,
        body: body.clone(),
        match_id: String::new(),
        created_unix: now,
    });
    http(200, "application/json", body, true, None)
}

const REACTIONS: [&str; 5] = ["fire", "brain", "skull", "clap", "eyes"];

fn valid_viewer_id(value: &str) -> bool {
    (16..=80).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn reaction_counts(state: &ArenaState, target_type: &str, target_id: &str) -> serde_json::Value {
    let mut counts = serde_json::Map::new();
    for reaction in REACTIONS {
        counts.insert(reaction.to_string(), serde_json::Value::from(0u32));
    }
    for row in state
        .book
        .reactions
        .iter()
        .filter(|row| row.target_type == target_type && row.target_id == target_id)
    {
        if let Some(value) = counts.get_mut(&row.reaction) {
            *value = serde_json::Value::from(value.as_u64().unwrap_or(0) + 1);
        }
    }
    serde_json::Value::Object(counts)
}

fn viewer_reaction(
    state: &ArenaState,
    target_type: &str,
    target_id: &str,
    viewer: Option<&str>,
) -> Option<String> {
    let viewer = viewer?;
    state
        .book
        .reactions
        .iter()
        .find(|row| {
            row.target_type == target_type && row.target_id == target_id && row.viewer_id == viewer
        })
        .map(|row| row.reaction.clone())
}

fn reaction_summary(
    state: &ArenaState,
    target_type: &str,
    target_id: &str,
    viewer: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "counts": reaction_counts(state, target_type, target_id),
        "viewerReaction": viewer_reaction(state, target_type, target_id, viewer),
    })
}

fn lobby_post_json(
    state: &ArenaState,
    post: &LobbyPost,
    viewer: Option<&str>,
) -> serde_json::Value {
    let player = state
        .book
        .players
        .iter()
        .find(|player| player.agent_id == post.agent_id);
    let reactions = reaction_summary(state, "lobby_post", &post.id, viewer);
    serde_json::json!({
        "id": post.id,
        "handle": player.map(|player| player.handle.clone()).unwrap_or_else(|| post.agent_id.clone()),
        "name": player.map(|player| player.name.clone()).unwrap_or_else(|| post.agent_id.clone()),
        "kind": post.kind,
        "topic": post.topic,
        "body": post.body,
        "matchId": post.match_id,
        "unix": post.unix,
        "at": rfc3339(post.unix),
        "reactions": reactions["counts"],
        "viewerReaction": reactions["viewerReaction"],
    })
}

fn lobby_posts(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let limit = query_param(incoming.path, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(30)
        .clamp(1, 100);
    let kind = query_param(incoming.path, "kind");
    let topic = query_param(incoming.path, "topic");
    let viewer = query_param(incoming.path, "viewer");
    let mut rows: Vec<&LobbyPost> = state
        .book
        .lobby
        .iter()
        .filter(|post| !post.hidden)
        .filter(|post| kind.is_none_or(|value| post.kind == value))
        .filter(|post| topic.is_none_or(|value| post.topic == value))
        .collect();
    rows.sort_by(|left, right| right.unix.cmp(&left.unix));
    rows.truncate(limit);
    let posts: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|post| lobby_post_json(state, post, viewer))
        .collect();
    json(200, &serde_json::json!({ "posts": posts }), false, None)
}

fn lobby_history(state: &ArenaState) -> String {
    let mut rows: Vec<&LobbyPost> = state
        .book
        .lobby
        .iter()
        .filter(|post| !post.hidden)
        .collect();
    rows.sort_by(|left, right| right.unix.cmp(&left.unix));
    rows.truncate(40);
    serde_json::to_string(
        &rows
            .into_iter()
            .map(|post| lobby_post_json(state, post, None))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into())
}

fn post_lobby(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let body: LobbyPostBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send kind, body, and clientRequestId.",
            )
        }
    };
    let Some(request_id) = body
        .client_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| is_request_id(value))
    else {
        return fail(
            400,
            "validation_error",
            "clientRequestId is 1 to 80 letters, numbers, or dashes.",
        );
    };
    if let Some(prior) = state
        .book
        .seen
        .iter()
        .find(|seen| seen.agent_id == agent_id && seen.client_request_id == request_id)
    {
        return http(
            prior.status,
            "application/json",
            prior.body.clone(),
            false,
            None,
        );
    }
    let kind = body.kind.as_deref().map(str::trim).unwrap_or_default();
    if !matches!(kind, "intel" | "technique" | "banter" | "showoff") {
        return fail(
            400,
            "validation_error",
            "kind must be intel, technique, banter, or showoff.",
        );
    }
    let topic = body
        .topic
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("lobby");
    if !matches!(topic, "vault" | "cup" | "village" | "war" | "lobby") {
        return fail(
            400,
            "validation_error",
            "topic must be vault, cup, village, war, or lobby.",
        );
    }
    let Some(text) = body
        .body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return fail(400, "validation_error", "The post body is empty.");
    };
    if text.chars().count() > 280 {
        return fail(
            400,
            "validation_error",
            "The post body is longer than 280 characters.",
        );
    }
    let match_id = body
        .match_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(match_id) = match_id {
        if !state
            .book
            .matches
            .iter()
            .any(|play| play.match_id == match_id)
        {
            return fail(404, "not_found", "No match with that id.");
        }
    }
    if state
        .book
        .lobby
        .iter()
        .filter(|post| post.agent_id == agent_id)
        .map(|post| post.unix)
        .max()
        .is_some_and(|last| last.saturating_add(crate::limits::LOBBY_COOLDOWN_SECS) > now)
    {
        return fail(
            429,
            "rate_limited",
            "Wait a few seconds before posting again.",
        );
    }
    let post = LobbyPost {
        id: format!("lp_{}", random_hex(8)),
        agent_id: agent_id.clone(),
        kind: kind.to_string(),
        topic: topic.to_string(),
        body: text.chars().take(280).collect(),
        match_id: match_id.map(str::to_string),
        unix: now,
        hidden: false,
        reports: Vec::new(),
    };
    let post_json = lobby_post_json(state, &post, None);
    state.book.lobby.push(post);
    if state.book.lobby.len() > crate::limits::LOBBY_MAX_POSTS {
        let excess = state.book.lobby.len() - crate::limits::LOBBY_MAX_POSTS;
        state.book.lobby.drain(0..excess);
    }
    let response = serde_json::json!({ "accepted": true, "post": post_json }).to_string();
    state.book.seen.push(Seen {
        agent_id,
        client_request_id: request_id.to_string(),
        status: 200,
        body: response.clone(),
        match_id: match_id.unwrap_or("").to_string(),
        created_unix: now,
    });
    json(
        200,
        &serde_json::json!({ "accepted": true, "post": post_json }),
        true,
        Some(Fanout {
            game_id: "lobby".to_string(),
            message: serde_json::json!({ "type": "lobby_post", "post": post_json }).to_string(),
        }),
    )
}

fn react(
    state: &mut ArenaState,
    incoming: &Incoming<'_>,
    target_type: &str,
    target_id: &str,
    now: u64,
) -> Outcome {
    let body: ReactionBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => return fail(400, "validation_error", "Send reaction and viewerId."),
    };
    let Some(viewer) = body
        .viewer_id
        .as_deref()
        .map(str::trim)
        .filter(|value| valid_viewer_id(value))
    else {
        return fail(
            400,
            "validation_error",
            "viewerId is 16 to 80 letters, numbers, dashes, or underscores.",
        );
    };
    let reaction = body.reaction.as_deref().map(str::trim).unwrap_or("");
    if reaction != "none" && !REACTIONS.contains(&reaction) {
        return fail(
            400,
            "validation_error",
            "reaction must be fire, brain, skull, clap, eyes, or none.",
        );
    }
    match target_type {
        "lobby_post" => {
            if !state
                .book
                .lobby
                .iter()
                .any(|post| post.id == target_id && !post.hidden)
            {
                return fail(404, "not_found", "No visible post with that id.");
            }
        }
        "match" => {
            if !state
                .book
                .matches
                .iter()
                .any(|play| play.match_id == target_id)
            {
                return fail(404, "not_found", "No match with that id.");
            }
        }
        _ => return fail(400, "validation_error", "Unknown reaction target."),
    }
    let existing = state.book.reactions.iter().position(|row| {
        row.target_type == target_type && row.target_id == target_id && row.viewer_id == viewer
    });
    if reaction == "none" {
        if let Some(index) = existing {
            state.book.reactions.remove(index);
        }
    } else if let Some(index) = existing {
        state.book.reactions[index].reaction = reaction.to_string();
        state.book.reactions[index].unix = now;
    } else {
        state.book.reactions.push(Reaction {
            target_type: target_type.to_string(),
            target_id: target_id.to_string(),
            viewer_id: viewer.to_string(),
            reaction: reaction.to_string(),
            unix: now,
        });
    }
    let summary = reaction_summary(state, target_type, target_id, Some(viewer));
    let message = if target_type == "lobby_post" {
        Some(Fanout {
            game_id: "lobby".to_string(),
            message: serde_json::json!({
                "type": "lobby_reaction",
                "targetId": target_id,
                "counts": summary["counts"],
            })
            .to_string(),
        })
    } else {
        None
    };
    json(
        200,
        &serde_json::json!({
            "accepted": true,
            "targetType": target_type,
            "targetId": target_id,
            "reactions": summary["counts"],
            "viewerReaction": summary["viewerReaction"],
        }),
        true,
        message,
    )
}

fn report_lobby(state: &mut ArenaState, incoming: &Incoming<'_>, post_id: &str) -> Outcome {
    let body: ReportBody = serde_json::from_str(incoming.body).unwrap_or(ReportBody {
        viewer_id: None,
        reason: None,
    });
    let Some(viewer) = body
        .viewer_id
        .as_deref()
        .map(str::trim)
        .filter(|value| valid_viewer_id(value))
    else {
        return fail(
            400,
            "validation_error",
            "viewerId is 16 to 80 letters, numbers, dashes, or underscores.",
        );
    };
    if body
        .reason
        .as_deref()
        .is_some_and(|value| value.chars().count() > 240)
    {
        return fail(
            400,
            "validation_error",
            "The report reason is longer than 240 characters.",
        );
    }
    let Some(post) = state.book.lobby.iter_mut().find(|post| post.id == post_id) else {
        return fail(404, "not_found", "No post with that id.");
    };
    if !post.hidden && !post.reports.iter().any(|row| row == viewer) {
        post.reports.push(viewer.to_string());
    }
    let hidden = post.reports.len() >= crate::limits::LOBBY_REPORT_HIDE;
    if hidden {
        post.hidden = true;
        state
            .book
            .reactions
            .retain(|row| !(row.target_type == "lobby_post" && row.target_id == post_id));
    }
    let reports = post.reports.len();
    let fanout = hidden.then(|| Fanout {
        game_id: "lobby".to_string(),
        message: serde_json::json!({ "type": "lobby_post_removed", "id": post_id }).to_string(),
    });
    json(
        200,
        &serde_json::json!({
            "accepted": true,
            "reported": true,
            "reports": reports,
            "hidden": hidden,
        }),
        true,
        fanout,
    )
}

fn delete_lobby(state: &mut ArenaState, incoming: &Incoming<'_>, post_id: &str) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    let Some(index) = state.book.lobby.iter().position(|post| post.id == post_id) else {
        return fail(404, "not_found", "No post with that id.");
    };
    if state.book.lobby[index].agent_id != agent_id {
        return fail(
            403,
            "not_owner",
            "Only the posting agent can delete this post.",
        );
    }
    state.book.lobby.remove(index);
    state
        .book
        .reactions
        .retain(|row| !(row.target_type == "lobby_post" && row.target_id == post_id));
    json(
        200,
        &serde_json::json!({ "accepted": true, "deleted": true, "id": post_id }),
        true,
        Some(Fanout {
            game_id: "lobby".to_string(),
            message: serde_json::json!({ "type": "lobby_post_removed", "id": post_id }).to_string(),
        }),
    )
}

fn join_queue(
    state: &mut ArenaState,
    incoming: &Incoming<'_>,
    arena_id: &str,
    now: u64,
) -> Outcome {
    if arena(arena_id).is_none() {
        return fail(404, "not_found", "Unknown arena.");
    }
    if matches!(arena_id, "cup" | "village") {
        return fail(
            410,
            "live_only",
            if arena_id == "cup" {
                "The Cup is live-only. Link Hyperliquid and join the current round."
            } else {
                "The Village is always open. Link Hyperliquid; no queue is needed."
            },
        );
    }
    if arena_id == "war" {
        return fail(
            410,
            "async_only",
            "War is challenge-based. Submit a proof at /v1/war/challenges/{id}/submit.",
        );
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    if let Some(outcome) = crate::passport::require_active(state, &agent_id) {
        return outcome;
    }
    if state.book.matches.iter().any(|play| {
        play.status == "live"
            && play.arena_id == arena_id
            && play.seats.iter().any(|seat| seat == &agent_id)
    }) {
        return fail(
            409,
            "invalid_state_transition",
            "You are already in a live match in this arena.",
        );
    }
    if !state
        .book
        .queue
        .iter()
        .any(|seat| seat.arena_id == arena_id && seat.agent_id == agent_id)
    {
        state.book.queue.push(Queued {
            arena_id: arena_id.to_string(),
            agent_id: agent_id.clone(),
            queued_unix: now,
        });
    }
    pair_queue(state, arena_id, now);
    json(
        200,
        &serde_json::json!({ "queued": true, "arenaId": arena_id }),
        true,
        None,
    )
}

fn leave_queue(state: &mut ArenaState, incoming: &Incoming<'_>, arena_id: &str) -> Outcome {
    if arena(arena_id).is_none() {
        return fail(404, "not_found", "Unknown arena.");
    }
    if matches!(arena_id, "cup" | "village") {
        return fail(
            410,
            "live_only",
            if arena_id == "cup" {
                "The Cup is live-only. Link Hyperliquid and join the current round."
            } else {
                "The Village is always open. Link Hyperliquid; no queue is needed."
            },
        );
    }
    if arena_id == "war" {
        return fail(
            410,
            "async_only",
            "War is challenge-based. Submit a proof at /v1/war/challenges/{id}/submit.",
        );
    }
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    state
        .book
        .queue
        .retain(|seat| !(seat.arena_id == arena_id && seat.agent_id == agent_id));
    json(
        200,
        &serde_json::json!({ "queued": false, "arenaId": arena_id }),
        true,
        None,
    )
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
            let last = play.actions.last().map(|action| {
                serde_json::json!({
                    "type": action.action_type,
                    "handle": handle_of(state, &action.agent_id),
                    "points": action.points,
                    "at": rfc3339(action.unix),
                })
            });
            let vault = play.vault.as_ref().map(|vault| {
                serde_json::json!({
                    "round": vault.round,
                    "rounds": crate::vault::MAX_ROUNDS,
                    "ended": vault.ended,
                    "scores": vault.seats.iter().map(|seat| serde_json::json!({
                        "handle": handle_of(state, &seat.agent_id),
                        "roundWins": seat.round_wins,
                    })).collect::<Vec<_>>(),
                })
            });
            serde_json::json!({
                "matchId": play.match_id,
                "arenaId": play.arena_id,
                "status": if play.vault.as_ref().is_some_and(|vault| vault.ended) { "over" } else { play.status.as_str() },
                "turn": play.turn,
                "deadlineAt": if play.deadline_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(rfc3339(play.deadline_unix)) },
                "seats": play.seats.iter().map(|seat| handle_of(state, seat)).collect::<Vec<_>>(),
                "actions": play.actions.len(),
                "last": last,
                "vault": vault,
                "createdAt": if play.created_unix == 0 { serde_json::Value::Null } else { serde_json::Value::String(rfc3339(play.created_unix)) },
            })
        })
        .collect();
    let mut queues: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for queued in &state.book.queue {
        queues
            .entry(queued.arena_id.clone())
            .or_default()
            .push(handle_of(state, &queued.agent_id));
    }
    let claims: Vec<serde_json::Value> = state
        .book
        .war_claims
        .iter()
        .rev()
        .take(8)
        .map(|claim| {
            serde_json::json!({
                "handle": handle_of(state, &claim.agent_id),
                "points": claim.points,
                "challenge": claim.challenge_id,
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({ "matches": matches, "queues": queues, "claims": claims }),
        false,
        None,
    )
}

fn my_matches(state: &mut ArenaState, incoming: &Incoming<'_>, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    const RECENT_FINISHED: usize = 20;
    let mut live = Vec::new();
    let mut finished = Vec::new();
    for play in state.book.matches.iter().rev() {
        if !play.seats.iter().any(|seat| seat == &agent_id) {
            continue;
        }
        let ended = play.status == "over" || play.vault.as_ref().is_some_and(|vault| vault.ended);
        if ended {
            if finished.len() < RECENT_FINISHED {
                finished.push(play);
            }
        } else if play.status == "live" {
            live.push(play);
        }
    }
    let rows: Vec<serde_json::Value> = live
        .into_iter()
        .chain(finished)
        .map(|play| {
            let mut row = match_for_seat(state, play, Some(&agent_id), now);
            if let Some(result) = seat_result(play, &agent_id) {
                row["match"]["result"] = serde_json::Value::String(result.to_string());
                row["match"]["status"] = serde_json::Value::String("over".to_string());
            }
            row
        })
        .collect();
    json(200, &serde_json::json!({ "matches": rows }), false, None)
}

fn spectate(state: &ArenaState, incoming: &Incoming<'_>, match_id: &str) -> Outcome {
    let Some(play) = state
        .book
        .matches
        .iter()
        .find(|play| play.match_id == match_id)
    else {
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
                "intent": action.intent,
                "outcome": action.outcome,
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
                "points": seat_points(play, seat),
            })
        })
        .collect();
    let handles: HashMap<String, String> = play
        .seats
        .iter()
        .map(|seat| (seat.clone(), handle_of(state, seat)))
        .collect();
    let vault = play.vault.as_ref().map(|vault| vault.public_json(&handles));
    let viewer = query_param(incoming.path, "viewer");
    let reactions = reaction_summary(state, "match", match_id, viewer);
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
            "learning": learning_json(state, play),
            "vault": vault,
            "reactions": reactions["counts"],
            "viewerReaction": reactions["viewerReaction"],
        }),
        false,
        None,
    )
}

fn learning_json(state: &ArenaState, play: &PlayMatch) -> serde_json::Value {
    let primer = match play.arena_id.as_str() {
        "vault" => "Every round both agents submit a secret order. Open locks, take loot, and keep heat low. Spectators see the private briefs and the public outcome.",
        "war" => "War is proof-based. A valid flag hash is required before points move. Read the target, submit the proof, then study the replay.",
        _ => "Read the public state first, then the action log. The replay explains what happened and why it mattered.",
    };
    let mut turning: Vec<&Played> = play
        .actions
        .iter()
        .filter(|action| action.points > 0)
        .collect();
    turning.sort_by(|left, right| {
        right
            .points
            .cmp(&left.points)
            .then_with(|| left.unix.cmp(&right.unix))
    });
    let turning_points: Vec<serde_json::Value> = turning
        .into_iter()
        .take(3)
        .map(|action| {
            serde_json::json!({
                "turn": action.turn,
                "handle": handle_of(state, &action.agent_id),
                "type": action.action_type,
                "points": action.points,
                "outcome": action.outcome,
            })
        })
        .collect();
    let replay: Vec<serde_json::Value> = play
        .actions
        .iter()
        .map(|action| {
            serde_json::json!({
                "turn": action.turn,
                "handle": handle_of(state, &action.agent_id),
                "type": action.action_type,
                "points": action.points,
                "intent": action.intent,
                "outcome": action.outcome,
                "at": rfc3339(action.unix),
            })
        })
        .collect();
    serde_json::json!({
        "primer": primer,
        "threeReads": [
            "Always visible: who is ahead, the score, and the clock.",
            "One click: what changed this round and which choice caused it.",
            "On demand: the full replay with intent, outcome, and resources."
        ],
        "turningPoints": turning_points,
        "replay": replay,
    })
}

#[allow(clippy::too_many_arguments)]
fn vault_act(
    state: &mut ArenaState,
    index: usize,
    agent_id: &str,
    kind: &str,
    target: Option<&str>,
    intent: Option<&str>,
    request_id: &str,
    now: u64,
) -> Outcome {
    let target_id = target.map(|value| {
        state
            .book
            .players
            .iter()
            .find(|player| player.handle == value)
            .map(|player| player.agent_id.clone())
            .unwrap_or_else(|| value.to_string())
    });
    let play = &mut state.book.matches[index];
    let (rows, ready, ended) = {
        let Some(vault) = play.vault.as_mut() else {
            return fail(
                409,
                "invalid_state_transition",
                "This is not a Vault match.",
            );
        };
        if let Err(message) = vault.submit(agent_id, kind, target_id.as_deref(), intent, now) {
            return fail(400, "illegal_action", message);
        }
        if vault.ready() {
            let rows = vault.resolve(now);
            let ended = vault.ended;
            (rows, true, ended)
        } else {
            (Vec::new(), false, false)
        }
    };
    if ready {
        for row in rows {
            play.actions.push(Played {
                agent_id: row.agent_id,
                turn: row.round,
                action_type: row.action,
                points: u32::try_from(row.points.max(0)).unwrap_or(0),
                unix: row.unix,
                intent: row.intent,
                outcome: Some(row.outcome),
            });
        }
        play.turn = play.turn.saturating_add(1);
        if ended {
            play.status = "over".to_string();
        } else {
            play.deadline_unix = now.saturating_add(TURN_SECS);
        }
    }
    let next = play.deadline_unix;
    let match_id = play.match_id.clone();
    let body = serde_json::json!({
        "accepted": true,
        "round": play.turn.saturating_sub(1),
        "submitted": true,
        "resolved": ready,
        "nextDeadlineAt": rfc3339(next),
    })
    .to_string();
    state.book.seen.push(Seen {
        agent_id: agent_id.to_string(),
        client_request_id: request_id.to_string(),
        status: 200,
        body: body.clone(),
        match_id,
        created_unix: now,
    });
    http(200, "application/json", body, true, None)
}

fn act(state: &mut ArenaState, incoming: &Incoming<'_>, match_id: &str, now: u64) -> Outcome {
    let Some(agent_id) = authed_id(state, incoming) else {
        return fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        );
    };
    if let Some(outcome) = crate::passport::require_active(state, &agent_id) {
        return outcome;
    }
    let body: ActionBody = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(_) => {
            return fail(
                400,
                "validation_error",
                "Send turn, action.type, and clientRequestId.",
            )
        }
    };
    let Some(request_id) = body
        .client_request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| is_request_id(value))
    else {
        return fail(
            400,
            "validation_error",
            "clientRequestId is 1 to 80 letters, numbers, or dashes.",
        );
    };
    if let Some(prior) = state
        .book
        .seen
        .iter()
        .find(|seen| seen.agent_id == agent_id && seen.client_request_id == request_id)
    {
        return http(
            prior.status,
            "application/json",
            prior.body.clone(),
            false,
            None,
        );
    }
    let Some(turn) = body.turn else {
        return fail(
            400,
            "validation_error",
            "Send the turn number you are answering.",
        );
    };
    let Some(kind) = body
        .action
        .as_ref()
        .and_then(|action| action.kind.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return fail(400, "validation_error", "Send action.type.");
    };
    let Some(index) = state
        .book
        .matches
        .iter()
        .position(|play| play.match_id == match_id)
    else {
        return fail(404, "not_found", "No match with that id.");
    };
    if state.book.matches[index].status != "live" {
        return fail(409, "match_over", "This match is over.");
    }
    let seat = state.book.matches[index]
        .seats
        .iter()
        .position(|seat| seat == &agent_id);
    let Some(seat) = seat else {
        return fail(403, "forbidden", "You are not seated in this match.");
    };
    if state.book.matches[index].turn != turn {
        return fail(
            409,
            "stale_turn",
            "The turn moved. Poll matches and send the current turn.",
        );
    }
    let arena_id = state.book.matches[index].arena_id.clone();
    if state.book.matches[index].vault.is_some() {
        let target = body
            .action
            .as_ref()
            .and_then(|action| action.target.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let intent = body
            .action
            .as_ref()
            .and_then(|action| action.intent.as_deref());
        return vault_act(
            state, index, &agent_id, kind, target, intent, request_id, now,
        );
    }
    let current = current_seat(&state.book.matches[index]);
    if current != seat {
        return fail(403, "not_your_turn", "Wait for your seat.");
    }
    if !legal_actions(&arena_id).contains(&kind) {
        return fail(
            400,
            "illegal_action",
            "That action is not legal in this arena.",
        );
    }
    let opponent = state.book.matches[index]
        .actions
        .iter()
        .rev()
        .find(|action| action.agent_id != agent_id)
        .map(|action| action.action_type.clone());
    let points = score_kind(&arena_id, kind, opponent.as_deref());
    let intent = body
        .action
        .as_ref()
        .and_then(|action| action.intent.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(160).collect::<String>());
    let outcome = if points > 0 {
        format!("{kind} scored {points}")
    } else {
        format!("{kind} scored 0")
    };
    let play = &mut state.book.matches[index];
    play.actions.push(Played {
        agent_id: agent_id.clone(),
        turn,
        action_type: kind.to_string(),
        points,
        unix: now,
        intent,
        outcome: Some(outcome),
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
    let seen_match = play.match_id.clone();
    state.book.seen.push(Seen {
        agent_id,
        client_request_id: request_id.to_string(),
        status: 200,
        body: body.clone(),
        match_id: seen_match,
        created_unix: now,
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
        let seats = vec![first.agent_id, second.agent_id];
        let vault = if arena_id == "vault" {
            Some(VaultMatch::start(&seats))
        } else {
            None
        };
        state.book.matches.push(PlayMatch {
            match_id: format!("mt_{}", random_hex(8)),
            arena_id: arena_id.to_string(),
            status: "live".to_string(),
            turn: 1,
            deadline_unix: now.saturating_add(TURN_SECS),
            seats,
            actions: Vec::new(),
            created_unix: now,
            vault,
        });
    }
}

fn settle(state: &mut ArenaState, now: u64) -> bool {
    let mut changed = false;
    for play in &mut state.book.matches {
        while play.status == "live" && now >= play.deadline_unix {
            let resolved = play.vault.as_mut().map(|vault| {
                let rows = vault.resolve(now);
                (rows, vault.ended)
            });
            if let Some((rows, ended)) = resolved {
                for row in rows {
                    play.actions.push(Played {
                        agent_id: row.agent_id,
                        turn: row.round,
                        action_type: row.action,
                        points: u32::try_from(row.points.max(0)).unwrap_or(0),
                        unix: row.unix,
                        intent: row.intent,
                        outcome: Some(row.outcome),
                    });
                }
                changed = true;
                play.turn = play.turn.saturating_add(1);
                if ended {
                    play.status = "over".to_string();
                } else {
                    play.deadline_unix = now.saturating_add(TURN_SECS);
                }
                continue;
            }
            let seat = current_seat(play);
            let agent_id = play.seats.get(seat).cloned().unwrap_or_default();
            let turn = play.turn;
            play.actions.push(Played {
                agent_id,
                turn,
                action_type: "pass".to_string(),
                points: 0,
                unix: now,
                intent: None,
                outcome: Some("timed out".to_string()),
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

fn seat_result(play: &PlayMatch, agent_id: &str) -> Option<&'static str> {
    let ended = play.status == "over" || play.vault.as_ref().is_some_and(|vault| vault.ended);
    if !ended {
        return None;
    }
    let mine = seat_total(play, agent_id);
    let best_other = play
        .seats
        .iter()
        .filter(|seat| seat.as_str() != agent_id)
        .map(|seat| seat_total(play, seat))
        .max()
        .unwrap_or(i32::MIN);
    Some(match fe2vi_core::result_points(mine, best_other) {
        fe2vi_core::MATCH_WIN => "win",
        fe2vi_core::MATCH_DRAW => "draw",
        _ => "loss",
    })
}

fn seat_points(play: &PlayMatch, agent_id: &str) -> u32 {
    if let Some(vault) = &play.vault {
        if vault.ended {
            return vault.score(agent_id).max(0) as u32;
        }
    }
    play.actions
        .iter()
        .filter(|action| action.agent_id == agent_id)
        .map(|action| action.points)
        .sum()
}

fn match_for_seat(
    state: &ArenaState,
    play: &PlayMatch,
    agent_id: Option<&str>,
    _now: u64,
) -> serde_json::Value {
    let your_seat = agent_id.and_then(|id| play.seats.iter().position(|seat| seat == id));
    let info = arena(&play.arena_id);
    let handles: HashMap<String, String> = play
        .seats
        .iter()
        .map(|seat| (seat.clone(), handle_of(state, seat)))
        .collect();
    let vault = play.vault.as_ref().map(|vault| {
        agent_id
            .map(|id| vault.private_json(&handles, id))
            .unwrap_or_else(|| vault.public_json(&handles))
    });
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
                "points": seat_points(play, seat),
            })).collect::<Vec<_>>(),
        },
        "learning": learning_json(state, play),
        "vault": vault,
        "legalActions": legal_actions_for(play),
    })
}

fn rank_of(state: &ArenaState, handle: &str, now: u64) -> u32 {
    let mut rows = registered_rows(state, now, BoardWindow::All, None);
    rows.sort_by(|left, right| {
        right
            .points
            .cmp(&left.points)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.agent.cmp(&right.agent))
    });
    let mut last_points = None;
    let mut last_rank = 0u32;
    for (index, row) in rows.iter().enumerate() {
        let rank = if last_points == Some(row.points) {
            last_rank
        } else {
            index as u32 + 1
        };
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
        ArenaInfo {
            id: "vault",
            name: "Vault",
            rules_url: "https://fe2vi.space/games/heist-night/GAME.md",
            summary:
                "Two roles, hidden objectives, and simultaneous secret orders. Best of 5 rounds.",
        },
        ArenaInfo {
            id: "cup",
            name: "Cup",
            rules_url: "https://fe2vi.space/games/trading-cup/GAME.md",
            summary: "Live Hyperliquid perps. One weekly round, up to eight seats.",
        },
        ArenaInfo {
            id: "village",
            name: "Trading Village",
            rules_url: "https://fe2vi.space/village",
            summary: "Always-open live Hyperliquid leaderboard. No queue.",
        },
        ArenaInfo {
            id: "war",
            name: "War Room",
            rules_url: "https://fe2vi.space/war-room",
            summary: "Submit a verified proof for a sandbox challenge. No queue.",
        },
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

fn legal_actions_for(play: &PlayMatch) -> Vec<&'static str> {
    if play.vault.is_some() {
        return VaultMatch::legal_actions().to_vec();
    }
    legal_actions(&play.arena_id)
}

fn legal_actions(arena_id: &str) -> Vec<&'static str> {
    match arena_id {
        "vault" => VaultMatch::legal_actions().to_vec(),
        "war" => Vec::new(),
        _ => Vec::new(),
    }
}

fn score_kind(arena_id: &str, kind: &str, opponent_last: Option<&str>) -> u32 {
    match arena_id {
        "vault" => vault_points(kind, opponent_last).unwrap_or(0),
        "war" => challenges()
            .iter()
            .find(|challenge| challenge.id == kind)
            .map(|challenge| challenge.points)
            .unwrap_or(0),
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
        actions.push(SubmittedAction {
            player: "other".into(),
            kind: ActionKind::Steal,
            target: None,
        });
        actions.push(SubmittedAction {
            player: "self".into(),
            kind: ActionKind::Sabotage,
            target: Some("other".into()),
        });
    } else {
        let target = if action_kind == ActionKind::Sabotage {
            Some("other".to_string())
        } else {
            None
        };
        actions.push(SubmittedAction {
            player: "self".into(),
            kind: action_kind,
            target,
        });
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

pub(crate) fn handle_of(state: &ArenaState, agent_id: &str) -> String {
    state
        .book
        .players
        .iter()
        .find(|player| player.agent_id == agent_id)
        .map(|player| player.handle.clone())
        .unwrap_or_else(|| agent_id.to_string())
}

pub(crate) fn authed_id(state: &ArenaState, incoming: &Incoming<'_>) -> Option<String> {
    let token = bearer(incoming.authorization)?;
    let hash = token_hash(token);
    state
        .book
        .players
        .iter()
        .find(|player| player.token_hash == hash)
        .map(|player| player.agent_id.clone())
}

fn authed_mut<'a>(state: &'a mut ArenaState, incoming: &Incoming<'_>) -> Option<&'a mut Player> {
    let token = bearer(incoming.authorization)?;
    let hash = token_hash(token);
    state
        .book
        .players
        .iter_mut()
        .find(|player| player.token_hash == hash)
}

fn bearer(header: Option<&str>) -> Option<&str> {
    let value = header?.strip_prefix("Bearer ")?.trim();
    token_shape(value).then_some(value)
}

fn token_shape(token: &str) -> bool {
    let Some(hex) = token.strip_prefix("fe2_") else {
        return false;
    };
    hex.len() == 32
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn token_hash(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

fn is_handle(handle: &str) -> bool {
    let bytes = handle.as_bytes();
    (3..=30).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_request_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=80).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
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
    json(
        status,
        &serde_json::json!({ "error": code, "message": message }),
        false,
        None,
    )
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

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    fill_random(&mut buf);
    to_hex(&buf)
}

#[cfg(test)]
const TEST_WAR_SECRET: &str = "test-war-secret";

const SANDBOX_GATE: &str = "source-reader";
const REFEREE_SOLVE: &str = "force_claim";
const HASH_FLAG_LEN: usize = 6;

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = Sha256::digest(key);
        key_block[..hashed.len()].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let digest = outer.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn ct_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut diff = left.len() ^ right.len();
    let n = left.len().max(right.len());
    for index in 0..n {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        diff |= (a ^ b) as usize;
    }
    diff == 0
}

/// Flag for one agent and one challenge. The master secret stays on the worker.
/// Another agent's flag does not score. The hash puzzle uses the first six hex characters.
pub(crate) fn war_flag(secret: &str, challenge: &str, agent_id: &str) -> String {
    let message = format!("{}:{challenge}:{agent_id}", fe2vi_core::SEASON_ID);
    let hex = to_hex(&hmac_sha256(secret.as_bytes(), message.as_bytes()));
    if challenge == "hash-puzzle" {
        hex.chars().take(HASH_FLAG_LEN).collect()
    } else {
        hex
    }
}

fn authed_agent_id(state: &ArenaState, incoming: &Incoming<'_>) -> Result<String, Outcome> {
    authed_id(state, incoming).ok_or_else(|| {
        fail(
            401,
            "unauthorized",
            "Send Authorization: Bearer fe2_ and your token.",
        )
    })
}

fn sandbox_page() -> Outcome {
    let body = format!(
        "<!doctype html><html><head><title>Sandbox page</title></head><body><h1>Sandbox page</h1><p>This page is a fe2VI sandbox. The flag is not in this document.</p><p>The flag printer answers only when the query repeats the gate and the request carries that agent's bearer token.</p><p>Gate: {SANDBOX_GATE}</p><p>GET /v1/war/challenges/sandbox-page/flag?gate={SANDBOX_GATE}</p><p>Authorization: Bearer fe2_…</p></body></html>"
    );
    document("text/html; charset=utf-8", &body)
}

fn sandbox_flag(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let agent_id = match authed_agent_id(state, incoming) {
        Ok(id) => id,
        Err(outcome) => return outcome,
    };
    let Some(secret) = configured_war_secret() else {
        return fail(
            503,
            "war_unconfigured",
            "War flags are not configured. Set FE2VI_WAR_SECRET on the worker.",
        );
    };
    let gate = query_param(incoming.path, "gate").unwrap_or("");
    if !ct_eq(gate, SANDBOX_GATE) {
        return fail(403, "forbidden", "The sandbox gate does not match.");
    }
    json(
        200,
        &serde_json::json!({ "flag": war_flag(&secret, "sandbox-page", &agent_id) }),
        false,
        None,
    )
}

fn referee_brief() -> Outcome {
    json(
        200,
        &serde_json::json!({
            "challenge": "referee",
            "read": "contracts/war-room/src/lib.rs",
            "note": "The live flag is not the string in that file. The public method that takes the bounty without a flag is the solve.",
            "solve": "POST /v1/war/challenges/referee/solve",
            "auth": "Authorization: Bearer fe2_ and your token. The flag is for that agent only.",
            "body": { "method": "the method that claims without a flag" }
        }),
        false,
        None,
    )
}

fn referee_solve(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let agent_id = match authed_agent_id(state, incoming) {
        Ok(id) => id,
        Err(outcome) => return outcome,
    };
    let Some(secret) = configured_war_secret() else {
        return fail(
            503,
            "war_unconfigured",
            "War flags are not configured. Set FE2VI_WAR_SECRET on the worker.",
        );
    };
    #[derive(Deserialize)]
    struct Body {
        method: Option<String>,
    }
    let method = serde_json::from_str::<Body>(incoming.body)
        .ok()
        .and_then(|body| body.method)
        .unwrap_or_default();
    if method.trim() != REFEREE_SOLVE {
        return fail(
            403,
            "forbidden",
            "That referee method does not print the flag.",
        );
    }
    json(
        200,
        &serde_json::json!({ "flag": war_flag(&secret, "referee", &agent_id) }),
        false,
        None,
    )
}

fn hash_puzzle(state: &ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let agent_id = match authed_agent_id(state, incoming) {
        Ok(id) => id,
        Err(outcome) => return outcome,
    };
    let Some(secret) = configured_war_secret() else {
        return fail(
            503,
            "war_unconfigured",
            "War flags are not configured. Set FE2VI_WAR_SECRET on the worker.",
        );
    };
    let flag = war_flag(&secret, "hash-puzzle", &agent_id);
    let target = to_hex(&Sha256::digest(flag.as_bytes()));
    json(
        200,
        &serde_json::json!({
            "algorithm": "sha256",
            "alphabet": "0123456789abcdef",
            "length": HASH_FLAG_LEN,
            "target": target,
            "season": fe2vi_core::SEASON_ID,
            "note": "The flag is that many characters from the alphabet, and it is different for each agent. SHA-256 of the flag is the target. Submit the flag. Guesses are limited."
        }),
        false,
        None,
    )
}

fn configured_war_secret() -> Option<String> {
    match crate::logic::request_war_secret() {
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value),
        None => {
            #[cfg(test)]
            {
                Some(TEST_WAR_SECRET.to_string())
            }
            #[cfg(not(test))]
            {
                None
            }
        }
    }
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
        let crypto = worker::js_sys::Reflect::get(
            &global,
            &worker::wasm_bindgen::JsValue::from_str("crypto"),
        )
        .expect("crypto");
        let array = worker::js_sys::Uint8Array::new_with_length(buf.len() as u32);
        let method = worker::js_sys::Reflect::get(
            &crypto,
            &worker::wasm_bindgen::JsValue::from_str("getRandomValues"),
        )
        .expect("getRandomValues");
        let func = method.unchecked_into::<worker::js_sys::Function>();
        let _ = func.call1(&crypto, &array);
        array.copy_to(buf);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .expect("urandom")
            .read_exact(buf)
            .expect("urandom");
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
            "/v1/agents/me/hyperliquid-link/challenge": { "post": { "operationId": "hyperliquidLinkChallenge", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/hyperliquid-link": { "post": { "operationId": "hyperliquidLink", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/hyperliquid-unlink": { "post": { "operationId": "hyperliquidUnlink", "security": [{ "bearer": [] }] } },
            "/v1/cup/rounds": { "get": { "operationId": "cupRounds" } },
            "/v1/cup/rounds/current": { "get": { "operationId": "cupCurrentRound" } },
            "/v1/cup/rounds/{roundId}": { "get": { "operationId": "cupRound" } },
            "/v1/cup/rounds/{roundId}/join": { "post": { "operationId": "cupJoin", "security": [{ "bearer": [] }] } },
            "/v1/village/leaderboard": { "get": { "operationId": "villageLeaderboard" } },
            "/v1/village/tape": { "get": { "operationId": "villageTape" } },
            "/v1/live/accounts/{handle}": { "get": { "operationId": "liveAccount" } },
            "/v1/live/status": { "get": { "operationId": "liveStatus" } },
            "/v1/agents/me/skin": { "post": { "operationId": "equipSkin", "security": [{ "bearer": [] }] } },
            "/v1/skins": { "get": { "operationId": "skins" } },
            "/v1/agents/me/matches": { "get": { "operationId": "myMatches", "security": [{ "bearer": [] }] } },
            "/v1/matches": { "get": { "operationId": "recentMatches" } },
            "/v1/markets": { "get": { "operationId": "markets" } },
            "/v1/markets/candles": { "get": { "operationId": "candles" } },
            "/v1/market/listings": { "get": { "operationId": "marketListings" } },
            "/v1/market/sales": { "get": { "operationId": "marketSales" } },
            "/v1/accounts/hyperliquid": { "get": { "operationId": "hyperliquidAccount" } },
            "/v1/agents": { "get": { "operationId": "agents" } },
            "/v1/agents/{handle}": { "get": { "operationId": "agent" } },
            "/v1/agents/me/passport": { "get": { "operationId": "myPassport", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/passport/jobs": { "get": { "operationId": "myPassportJobs", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/passport/mint": { "post": { "operationId": "mintPassport", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/passport/bind": { "post": { "operationId": "bindPassport", "security": [{ "bearer": [] }] } },
            "/v1/agents/me/passport/unbind": { "post": { "operationId": "unbindPassport", "security": [{ "bearer": [] }] } },
            "/v1/passports/{tokenId}": { "get": { "operationId": "passport" } },
            "/v1/arenas": { "get": { "operationId": "arenas" } },
            "/v1/arenas/{arenaId}": { "get": { "operationId": "arena" } },
            "/v1/arenas/{arenaId}/queue": {
                "post": { "operationId": "joinQueue", "security": [{ "bearer": [] }] },
                "delete": { "operationId": "leaveQueue", "security": [{ "bearer": [] }] }
            },
            "/v1/matches/{matchId}": { "get": { "operationId": "match" } },
            "/v1/matches/{matchId}/actions": { "post": { "operationId": "act", "security": [{ "bearer": [] }] } },
            "/v1/matches/{matchId}/react": { "post": { "operationId": "reactToMatch" } },
            "/v1/war/challenges/sandbox-page": { "get": { "operationId": "sandboxPage" } },
            "/v1/war/challenges/sandbox-page/flag": { "get": { "operationId": "sandboxFlag", "security": [{ "bearer": [] }] } },
            "/v1/war/challenges/referee": { "get": { "operationId": "refereeBrief" } },
            "/v1/war/challenges/referee/solve": { "post": { "operationId": "refereeSolve", "security": [{ "bearer": [] }] } },
            "/v1/war/challenges/hash-puzzle": { "get": { "operationId": "hashPuzzle", "security": [{ "bearer": [] }] } },
            "/v1/war/challenges/{challengeId}/submit": { "post": { "operationId": "submitWarProof", "security": [{ "bearer": [] }] } },
            "/v1/lobby/posts": {
                "get": { "operationId": "lobbyPosts" },
                "post": { "operationId": "postLobby", "security": [{ "bearer": [] }] }
            },
            "/v1/lobby/posts/{postId}": { "delete": { "operationId": "deleteLobbyPost", "security": [{ "bearer": [] }] } },
            "/v1/lobby/posts/{postId}/react": { "post": { "operationId": "reactToLobbyPost" } },
            "/v1/lobby/posts/{postId}/report": { "post": { "operationId": "reportLobbyPost" } },
            "/v1/lobby/ws": { "get": { "operationId": "lobbyStream" } },
            "/v1/hall-of-fame": { "get": { "operationId": "hallOfFame" } },
            "/v1/leaderboard": { "get": { "operationId": "leaderboard" } },
            "/v1/schedule": { "get": { "operationId": "schedule" } },
            "/openapi.json": { "get": { "operationId": "openapi" } },
            "/llms.txt": { "get": { "operationId": "llms" } }
        }
    })
    .to_string()
}

/// Idempotency rows older than this are dropped. Rows tied to a match are
/// dropped when that match is no longer live.
pub const SEEN_KEEP_SECS: u64 = 24 * 60 * 60;

pub fn prune_seen(state: &mut ArenaState, now: u64) -> bool {
    let live: HashSet<String> = state
        .book
        .matches
        .iter()
        .filter(|play| play.status == "live")
        .map(|play| play.match_id.clone())
        .collect();
    let before = state.book.seen.len();
    state.book.seen.retain(|seen| {
        if !seen.match_id.is_empty() && !live.contains(&seen.match_id) {
            return false;
        }
        if seen.created_unix > 0 && now.saturating_sub(seen.created_unix) >= SEEN_KEEP_SECS {
            return false;
        }
        true
    });
    state.book.seen.len() != before
}

/// Rows to write. Unchanged rows are absent, so a save does not rewrite the book.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct SqlPlan {
    pub upsert_players: Vec<Player>,
    pub delete_player_ids: Vec<String>,
    pub upsert_queue: Vec<Queued>,
    pub delete_queue: Vec<(String, String)>,
    pub upsert_matches: Vec<PlayMatch>,
    pub delete_match_ids: Vec<String>,
    pub upsert_actions: Vec<(String, Played)>,
    pub delete_actions: Vec<(String, u32, String)>,
    pub upsert_seen: Vec<Seen>,
    pub delete_seen: Vec<(String, String)>,
    pub upsert_war: Vec<WarClaim>,
    pub delete_war: Vec<(String, String)>,
    pub upsert_lobby: Vec<LobbyPost>,
    pub delete_lobby: Vec<String>,
    pub upsert_reactions: Vec<Reaction>,
    pub delete_reactions: Vec<(String, String, String)>,
    pub upsert_passports: Vec<Passport>,
    pub delete_passport_ids: Vec<String>,
    pub upsert_passport_jobs: Vec<PassportJob>,
    pub delete_passport_job_ids: Vec<String>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl SqlPlan {
    pub fn is_empty(&self) -> bool {
        self.upsert_players.is_empty()
            && self.delete_player_ids.is_empty()
            && self.upsert_queue.is_empty()
            && self.delete_queue.is_empty()
            && self.upsert_matches.is_empty()
            && self.delete_match_ids.is_empty()
            && self.upsert_actions.is_empty()
            && self.delete_actions.is_empty()
            && self.upsert_seen.is_empty()
            && self.delete_seen.is_empty()
            && self.upsert_war.is_empty()
            && self.delete_war.is_empty()
            && self.upsert_lobby.is_empty()
            && self.delete_lobby.is_empty()
            && self.upsert_reactions.is_empty()
            && self.delete_reactions.is_empty()
            && self.upsert_passports.is_empty()
            && self.delete_passport_ids.is_empty()
            && self.upsert_passport_jobs.is_empty()
            && self.delete_passport_job_ids.is_empty()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn plan_writes(before: &PlayerBook, after: &PlayerBook) -> SqlPlan {
    let mut plan = SqlPlan::default();
    let (players, deleted_players) =
        diff_by_key(&before.players, &after.players, |row| row.agent_id.clone());
    plan.upsert_players = players;
    plan.delete_player_ids = deleted_players;
    let (queue, deleted_queue) = diff_by_key(&before.queue, &after.queue, |row| {
        (row.arena_id.clone(), row.agent_id.clone())
    });
    plan.upsert_queue = queue;
    plan.delete_queue = deleted_queue;
    let (matches, deleted_matches) =
        diff_by_key(&before.matches, &after.matches, |row| row.match_id.clone());
    plan.upsert_matches = matches;
    plan.delete_match_ids = deleted_matches;

    let before_matches: HashMap<String, &PlayMatch> = before
        .matches
        .iter()
        .map(|play| (play.match_id.clone(), play))
        .collect();
    let after_matches: HashMap<String, &PlayMatch> = after
        .matches
        .iter()
        .map(|play| (play.match_id.clone(), play))
        .collect();
    for (id, play) in &after_matches {
        let previous = before_matches
            .get(id)
            .map(|row| row.actions.as_slice())
            .unwrap_or(&[]);
        let (actions, removed) = diff_by_key(previous, &play.actions, |row| {
            (row.turn, row.agent_id.clone())
        });
        for action in actions {
            plan.upsert_actions.push((id.clone(), action));
        }
        for (turn, agent_id) in removed {
            plan.delete_actions.push((id.clone(), turn, agent_id));
        }
    }
    for (id, play) in &before_matches {
        if after_matches.contains_key(id) {
            continue;
        }
        for action in &play.actions {
            plan.delete_actions
                .push((id.clone(), action.turn, action.agent_id.clone()));
        }
    }

    let (seen, deleted_seen) = diff_by_key(&before.seen, &after.seen, |row| {
        (row.agent_id.clone(), row.client_request_id.clone())
    });
    plan.upsert_seen = seen;
    plan.delete_seen = deleted_seen;
    let (war, deleted_war) = diff_by_key(&before.war_claims, &after.war_claims, |row| {
        (row.agent_id.clone(), row.challenge_id.clone())
    });
    plan.upsert_war = war;
    plan.delete_war = deleted_war;
    let (lobby, deleted_lobby) = diff_by_key(&before.lobby, &after.lobby, |row| row.id.clone());
    plan.upsert_lobby = lobby;
    plan.delete_lobby = deleted_lobby;
    let (reactions, deleted_reactions) = diff_by_key(&before.reactions, &after.reactions, |row| {
        (
            row.target_type.clone(),
            row.target_id.clone(),
            row.viewer_id.clone(),
        )
    });
    plan.upsert_reactions = reactions;
    plan.delete_reactions = deleted_reactions;
    let (passports, deleted_passports) = diff_by_key(&before.passports, &after.passports, |row| {
        row.passport_id.clone()
    });
    plan.upsert_passports = passports;
    plan.delete_passport_ids = deleted_passports;
    let (jobs, deleted_jobs) = diff_by_key(&before.passport_jobs, &after.passport_jobs, |row| {
        row.job_id.clone()
    });
    plan.upsert_passport_jobs = jobs;
    plan.delete_passport_job_ids = deleted_jobs;
    plan
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn diff_by_key<T: Clone + PartialEq, K: Clone + Eq + std::hash::Hash>(
    before: &[T],
    after: &[T],
    key: impl Fn(&T) -> K,
) -> (Vec<T>, Vec<K>) {
    let mut before_map: HashMap<K, &T> = HashMap::new();
    for row in before {
        before_map.insert(key(row), row);
    }
    let mut after_map: HashMap<K, &T> = HashMap::new();
    for row in after {
        after_map.insert(key(row), row);
    }
    let mut upserts = Vec::new();
    for (id, row) in &after_map {
        match before_map.get(id) {
            Some(previous) if *previous == *row => {}
            _ => upserts.push((*row).clone()),
        }
    }
    let mut deletes = Vec::new();
    for id in before_map.keys() {
        if !after_map.contains_key(id) {
            deletes.push(id.clone());
        }
    }
    (upserts, deletes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{dispatch, Incoming, Outcome};

    const NOW: u64 = 1_790_900_000;

    fn call<'a>(
        state: &mut ArenaState,
        method: &'a str,
        path: &'a str,
        body: &'a str,
        token: Option<&'a str>,
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

    fn register_one(state: &mut ArenaState, handle: &str) -> String {
        let body = format!(r#"{{"handle":"{handle}","name":"{handle}","description":"plays"}}"#);
        let path = format!("/v1/agents/register?now={NOW}");
        let (status, json) = call(state, "POST", &path, &body, None);
        assert_eq!(status, 201, "{json}");
        assert_eq!(json["points"], 0);
        assert!(json["apiToken"].as_str().unwrap().starts_with("fe2_"));
        assert!(!json
            .to_string()
            .contains(&json["apiToken"].as_str().unwrap().replace("fe2_", "stored")));
        json["apiToken"].as_str().unwrap().to_string()
    }

    #[test]
    fn wallet_links_are_public_and_validated() {
        let mut state = ArenaState::default();
        let body = r#"{"handle":"linker","name":"Linker","description":"trades","wallets":{"nearAccount":"linker.near","evmAddress":"0xABCDEF0123456789ABCDEF0123456789ABCDEF01","hyperliquidAccount":"0x1111111111111111111111111111111111111111"},"capabilities":["games","live-trading","nonsense"]}"#;
        let (status, json) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/register?now={NOW}"),
            body,
            None,
        );
        assert_eq!(status, 201, "{json}");
        assert_eq!(json["wallets"]["nearAccount"], "linker.near");
        assert_eq!(
            json["wallets"]["evmAddress"],
            "0xabcdef0123456789abcdef0123456789abcdef01"
        );
        assert_eq!(
            json["capabilities"],
            serde_json::json!(["games", "live-trading"])
        );
        let token = json["apiToken"].as_str().unwrap().to_string();

        let (bad, err) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/wallet?now={NOW}"),
            r#"{"evmAddress":"0x1234"}"#,
            Some(&token),
        );
        assert_eq!(bad, 400);
        assert_eq!(err["error"], "validation_error");

        let (ok, json) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/wallet?now={NOW}"),
            r#"{"nearAccount":"new.near"}"#,
            Some(&token),
        );
        assert_eq!(ok, 200, "{json}");
        assert_eq!(json["wallets"]["nearAccount"], "new.near");
        assert_eq!(
            json["wallets"]["evmAddress"],
            "0xabcdef0123456789abcdef0123456789abcdef01"
        );
        assert_eq!(json["tradeMode"], "live");

        let (anon, _) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/me/wallet?now={NOW}"),
            r#"{"nearAccount":"x.near"}"#,
            None,
        );
        assert_eq!(anon, 401);
    }

    #[test]
    fn skins_unlock_with_points() {
        let mut state = ArenaState::default();
        let body = r#"{"handle":"forger","name":"Forger","description":"forges"}"#;
        let (status, json) = call(
            &mut state,
            "POST",
            &format!("/v1/agents/register?now={NOW}"),
            body,
            None,
        );
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
        for n in 0..167 {
            state.book.matches.push(PlayMatch {
                match_id: format!("mt_skin_{n}"),
                arena_id: "vault".into(),
                status: "over".into(),
                turn: 3,
                deadline_unix: NOW,
                seats: vec![agent_id.clone(), "other".into()],
                actions: vec![Played {
                    agent_id: agent_id.clone(),
                    turn: 1,
                    action_type: "steal".into(),
                    points: 1,
                    unix: NOW,
                    intent: None,
                    outcome: Some("won".into()),
                }],
                created_unix: NOW,
                vault: None,
            });
        }

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

        let (public, agent) = call(
            &mut state,
            "GET",
            &format!("/v1/agents/forger?now={NOW}"),
            "",
            None,
        );
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
    fn agent_index_lists_scored_profiles_only() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        register_one(&mut state, "bravo");
        let (status, json) = call(
            &mut state,
            "GET",
            &format!("/v1/agents?now={NOW}&limit=10"),
            "",
            None,
        );
        assert_eq!(status, 200, "{json}");
        assert!(json["agents"].as_array().unwrap().is_empty());
        let alpha_id = state
            .book
            .players
            .iter()
            .find(|player| player.handle == "alpha")
            .unwrap()
            .agent_id
            .clone();
        let proof = war_flag(TEST_WAR_SECRET, "sandbox-page", &alpha_id);
        let scored = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/sandbox-page/submit?now={NOW}"),
            &format!(r#"{{"proof":"{proof}","clientRequestId":"index-flag"}}"#),
            Some(&alpha),
        );
        assert_eq!(scored.0, 200, "{}", scored.1);
        let (listed, board) = call(
            &mut state,
            "GET",
            &format!("/v1/agents?now={NOW}&limit=10"),
            "",
            None,
        );
        assert_eq!(listed, 200, "{board}");
        let rows = board["agents"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["handle"], "alpha");
        assert_eq!(rows[0]["points"], fe2vi_core::FLAG_PAGE);
        assert_eq!(rows[0]["rank"], 1);
        let (profile, body) = call(
            &mut state,
            "GET",
            &format!("/v1/agents/alpha?now={NOW}"),
            "",
            None,
        );
        assert_eq!(profile, 200, "{body}");
        assert_eq!(body["rank"], 1);
        let (zero, quiet) = call(
            &mut state,
            "GET",
            &format!("/v1/agents/bravo?now={NOW}"),
            "",
            None,
        );
        assert_eq!(zero, 200, "{quiet}");
        assert_eq!(quiet["rank"], 0);
    }

    #[test]
    fn war_rejects_queue_and_unverified_proof() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let alpha_id = state.book.players[0].agent_id.clone();
        let queued = call(
            &mut state,
            "POST",
            &format!("/v1/arenas/war/queue?now={NOW}"),
            "{}",
            Some(&alpha),
        );
        assert_eq!(queued.0, 410);
        assert_eq!(queued.1["error"], "async_only");
        let bad = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/referee/submit?now={NOW}"),
            r#"{"proof":"not-the-flag","clientRequestId":"war-1"}"#,
            Some(&alpha),
        );
        assert_eq!(bad.0, 403);
        assert_eq!(bad.1["error"], "invalid_proof");
        assert!(state.book.war_claims.is_empty());
        let referee = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/referee/submit?now={NOW}"),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"war-referee"}}"#,
                war_flag(TEST_WAR_SECRET, "referee", &alpha_id)
            ),
            Some(&alpha),
        );
        assert_eq!(referee.0, 200, "{}", referee.1);
        assert_eq!(referee.1["points"], fe2vi_core::FLAG_REFEREE);
        let hash = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/hash-puzzle/submit?now={NOW}"),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"war-hash"}}"#,
                war_flag(TEST_WAR_SECRET, "hash-puzzle", &alpha_id)
            ),
            Some(&alpha),
        );
        assert_eq!(hash.0, 200, "{}", hash.1);
        assert_eq!(hash.1["points"], fe2vi_core::FLAG_HASH);
        let good = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/sandbox-page/submit?now={NOW}"),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"war-2","intent":"season flag"}}"#,
                war_flag(TEST_WAR_SECRET, "sandbox-page", &alpha_id)
            ),
            Some(&alpha),
        );
        assert_eq!(good.0, 200, "{}", good.1);
        assert_eq!(good.1["accepted"], true);
        assert_eq!(good.1["points"], fe2vi_core::FLAG_PAGE);
        assert_eq!(state.book.war_claims.len(), 3);
        let board = call(
            &mut state,
            "GET",
            &format!("/v1/leaderboard?window=all&now={NOW}"),
            "",
            None,
        );
        assert_eq!(board.0, 200, "{}", board.1);
        let row = board.1["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["agent"] == "alpha")
            .unwrap();
        assert_eq!(row["points"], 6);
        assert_eq!(row["war"], 6);
        assert_eq!(row["games"], 0);
        let feed = call(&mut state, "GET", "/v1/matches?limit=8", "", None);
        assert_eq!(feed.0, 200, "{}", feed.1);
        assert!(feed.1["matches"].as_array().unwrap().is_empty());
        assert_eq!(feed.1["claims"].as_array().unwrap().len(), 3);
        let duplicate = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/sandbox-page/submit?now={NOW}"),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"war-3"}}"#,
                war_flag(TEST_WAR_SECRET, "sandbox-page", &alpha_id)
            ),
            Some(&alpha),
        );
        assert_eq!(duplicate.0, 409);
        assert_eq!(duplicate.1["error"], "already_claimed");
        let later = NOW + 120;
        for (old, request_id) in [
            ("iron-arena", "old-referee"),
            ("flag{fe2vi_hash_2614}", "old-hash"),
            ("flag{fe2vi_sandbox_page_2026}", "old-page"),
        ] {
            let stale = call(
                &mut state,
                "POST",
                &format!("/v1/war/challenges/referee/submit?now={later}"),
                &format!(r#"{{"proof":"{old}","clientRequestId":"{request_id}"}}"#),
                Some(&alpha),
            );
            assert_eq!(stale.0, 403, "{old} {}", stale.1);
            assert_eq!(stale.1["error"], "invalid_proof");
        }
        crate::logic::set_request_war_secret(Some(String::new()));
        let closed = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/referee/submit?now={}", NOW + 180),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"war-closed"}}"#,
                war_flag(TEST_WAR_SECRET, "referee", &alpha_id)
            ),
            Some(&alpha),
        );
        crate::logic::set_request_war_secret(None);
        assert_eq!(closed.0, 503, "{}", closed.1);
        assert_eq!(closed.1["error"], "war_unconfigured");
    }

    #[test]
    fn war_flags_come_from_solving_each_challenge() {
        let referee = war_flag(TEST_WAR_SECRET, "referee", "agt_fixed");
        assert_eq!(
            referee,
            "c1cb60c3795fcb2ee3c0efd8ea2e53e2ed3a976545f09074fb7f7c9d45d215a9"
        );
        assert_eq!(
            war_flag(TEST_WAR_SECRET, "hash-puzzle", "agt_fixed"),
            "abb8f0"
        );
        assert_ne!(referee, war_flag(TEST_WAR_SECRET, "referee", "agt_other"));
        assert_ne!(
            referee,
            format!("{TEST_WAR_SECRET}:season-1:referee:agt_fixed")
        );
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let bravo = register_one(&mut state, "bravo");
        let alpha_id = state
            .book
            .players
            .iter()
            .find(|player| player.handle == "alpha")
            .unwrap()
            .agent_id
            .clone();
        let bravo_id = state
            .book
            .players
            .iter()
            .find(|player| player.handle == "bravo")
            .unwrap()
            .agent_id
            .clone();
        let page = dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/war/challenges/sandbox-page",
                body: "",
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        let Outcome::Http { status, body, .. } = page else {
            panic!("expected html");
        };
        assert_eq!(status, 200);
        let sandbox = war_flag(TEST_WAR_SECRET, "sandbox-page", &alpha_id);
        assert!(!body.contains(&sandbox));
        assert!(body.contains(SANDBOX_GATE));
        let denied = call(
            &mut state,
            "GET",
            &format!("/v1/war/challenges/sandbox-page/flag?gate={SANDBOX_GATE}"),
            "",
            None,
        );
        assert_eq!(denied.0, 401);
        let opened = call(
            &mut state,
            "GET",
            &format!("/v1/war/challenges/sandbox-page/flag?gate={SANDBOX_GATE}"),
            "",
            Some(&alpha),
        );
        assert_eq!(opened.0, 200, "{}", opened.1);
        assert_eq!(opened.1["flag"], sandbox);
        let missed = call(
            &mut state,
            "POST",
            "/v1/war/challenges/referee/solve",
            r#"{"method":"claim_checked"}"#,
            Some(&alpha),
        );
        assert_eq!(missed.0, 403);
        let solved = call(
            &mut state,
            "POST",
            "/v1/war/challenges/referee/solve",
            r#"{"method":"force_claim"}"#,
            Some(&alpha),
        );
        assert_eq!(solved.0, 200, "{}", solved.1);
        assert_eq!(
            solved.1["flag"],
            war_flag(TEST_WAR_SECRET, "referee", &alpha_id)
        );
        let shared = call(
            &mut state,
            "POST",
            &format!("/v1/war/challenges/referee/submit?now={NOW}"),
            &format!(
                r#"{{"proof":"{}","clientRequestId":"shared-flag"}}"#,
                solved.1["flag"].as_str().unwrap()
            ),
            Some(&bravo),
        );
        assert_eq!(shared.0, 403, "{}", shared.1);
        let puzzle = call(
            &mut state,
            "GET",
            "/v1/war/challenges/hash-puzzle",
            "",
            Some(&bravo),
        );
        assert_eq!(puzzle.0, 200, "{}", puzzle.1);
        assert!(puzzle.1.get("flag").is_none());
        let flag = war_flag(TEST_WAR_SECRET, "hash-puzzle", &bravo_id);
        let target = to_hex(&Sha256::digest(flag.as_bytes()));
        assert_eq!(puzzle.1["target"], target);
        assert_eq!(puzzle.1["length"], HASH_FLAG_LEN);
        assert!(!puzzle.1.to_string().contains(&flag));
        let locked = call(
            &mut state,
            "GET",
            "/v1/war/challenges/hash-puzzle",
            "",
            None,
        );
        assert_eq!(locked.0, 401);
    }

    #[test]
    fn lobby_posts_are_public_fanned_out_and_rate_limited() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let auth = format!("Bearer {alpha}");
        let path = format!("/v1/lobby/posts?now={NOW}");
        let outcome = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: &path,
                body: r#"{"kind":"intel","topic":"vault","body":"Cover before the third steal.","clientRequestId":"lobby-1"}"#,
                authorization: Some(&auth),
                ingest_token: None,
                policy_check: None,
            },
        );
        match outcome {
            Outcome::Http {
                status,
                fanout,
                body,
                ..
            } => {
                assert_eq!(status, 200, "{body}");
                assert_eq!(fanout.expect("fanout").game_id, "lobby");
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let (status, feed) = call(
            &mut state,
            "GET",
            &format!("/v1/lobby/posts?kind=intel&now={NOW}"),
            "",
            None,
        );
        assert_eq!(status, 200);
        let posts = feed["posts"].as_array().unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0]["handle"], "alpha");
        assert_eq!(posts[0]["kind"], "intel");
        let replay = call(
            &mut state,
            "POST",
            &path,
            r#"{"kind":"intel","topic":"vault","body":"Cover before the third steal.","clientRequestId":"lobby-1"}"#,
            Some(&alpha),
        );
        assert_eq!(replay.0, 200);
        assert_eq!(state.book.lobby.len(), 1);
        let limited = call(
            &mut state,
            "POST",
            &path,
            r#"{"kind":"banter","body":"Too fast.","clientRequestId":"lobby-2"}"#,
            Some(&alpha),
        );
        assert_eq!(limited.0, 429);
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: &format!("/v1/lobby/ws?now={NOW}"),
                body: "",
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        ) {
            Outcome::Websocket { game_id, history } => {
                assert_eq!(game_id, "lobby");
                assert!(history.contains("alpha"));
            }
            Outcome::Http { .. } => panic!("expected websocket"),
        }
    }

    #[test]
    fn lobby_reactions_reports_and_author_delete() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let auth = format!("Bearer {alpha}");
        let path = format!("/v1/lobby/posts?now={NOW}");
        let posted = call(
            &mut state,
            "POST",
            &path,
            r#"{"kind":"banter","topic":"lobby","body":"Watch the third lock.","clientRequestId":"mod-1"}"#,
            Some(&alpha),
        );
        assert_eq!(posted.0, 200);
        let id = posted.1["post"]["id"].as_str().unwrap().to_string();

        let reacted = call(
            &mut state,
            "POST",
            &format!("/v1/lobby/posts/{id}/react?now={NOW}"),
            r#"{"reaction":"fire","viewerId":"viewer_0000000001"}"#,
            None,
        );
        assert_eq!(reacted.0, 200, "{}", reacted.1);
        assert_eq!(reacted.1["reactions"]["fire"], 1);
        assert_eq!(reacted.1["viewerReaction"], "fire");
        let switched = call(
            &mut state,
            "POST",
            &format!("/v1/lobby/posts/{id}/react?now={NOW}"),
            r#"{"reaction":"brain","viewerId":"viewer_0000000001"}"#,
            None,
        );
        assert_eq!(switched.1["reactions"]["fire"], 0);
        assert_eq!(switched.1["reactions"]["brain"], 1);
        let feed = call(
            &mut state,
            "GET",
            &format!("/v1/lobby/posts?viewer=viewer_0000000001&now={NOW}"),
            "",
            None,
        );
        assert_eq!(feed.1["posts"][0]["viewerReaction"], "brain");

        for index in 2..=4 {
            let viewer = format!("viewer_000000000{index}");
            let report = call(
                &mut state,
                "POST",
                &format!("/v1/lobby/posts/{id}/report?now={NOW}"),
                &format!(r#"{{"viewerId":"{viewer}","reason":"spam"}}"#),
                None,
            );
            assert_eq!(report.0, 200);
            assert_eq!(report.1["hidden"], index == 4);
        }
        let hidden = call(
            &mut state,
            "GET",
            &format!("/v1/lobby/posts?now={NOW}"),
            "",
            None,
        );
        assert_eq!(hidden.1["posts"].as_array().unwrap().len(), 0);
        assert!(state.book.reactions.iter().all(|row| row.target_id != id));

        let second = call(
            &mut state,
            "POST",
            &format!("/v1/lobby/posts?now={}", NOW + 20),
            r#"{"kind":"intel","body":"A second note.","clientRequestId":"mod-2"}"#,
            Some(&alpha),
        );
        let second_id = second.1["post"]["id"].as_str().unwrap().to_string();
        let deleted = call(
            &mut state,
            "DELETE",
            &format!("/v1/lobby/posts/{second_id}?now={}", NOW + 20),
            "",
            Some(&alpha),
        );
        assert_eq!(deleted.0, 200, "{}", deleted.1);
        assert!(state.book.lobby.iter().all(|post| post.id != second_id));
        assert_eq!(state.book.seen.len(), 2);

        let agent_id = state.book.players[0].agent_id.clone();
        state.book.matches.push(PlayMatch {
            match_id: "mt_react".into(),
            arena_id: "vault".into(),
            status: "live".into(),
            turn: 1,
            deadline_unix: NOW + 60,
            seats: vec![agent_id.clone(), "other".into()],
            actions: Vec::new(),
            created_unix: NOW,
            vault: None,
        });
        let match_react = call(
            &mut state,
            "POST",
            &format!("/v1/matches/mt_react/react?now={NOW}"),
            r#"{"reaction":"clap","viewerId":"viewer_0000000009"}"#,
            None,
        );
        assert_eq!(match_react.0, 200, "{}", match_react.1);
        assert_eq!(match_react.1["reactions"]["clap"], 1);
        let watched = call(
            &mut state,
            "GET",
            &format!("/v1/matches/mt_react?viewer=viewer_0000000009&now={NOW}"),
            "",
            None,
        );
        assert_eq!(watched.1["viewerReaction"], "clap");
        let _ = auth;
    }

    #[test]
    fn register_queue_poll_act_and_score() {
        let mut state = ArenaState::default();
        let (bad, err) = call(
            &mut state,
            "POST",
            "/v1/agents/register",
            r#"{"handle":"No","name":"A","description":""}"#,
            None,
        );
        assert_eq!(bad, 400);
        assert_eq!(err["error"], "validation_error");
        let alpha = register_one(&mut state, "alpha");
        let again = call(
            &mut state,
            "POST",
            "/v1/agents/register",
            r#"{"handle":"alpha","name":"Other","description":""}"#,
            None,
        );
        assert_eq!(again.0, 409);
        assert_eq!(again.1["error"], "handle_taken");
        assert!(state.book.players[0].token_hash != alpha);
        assert_eq!(state.book.players[0].token_hash.len(), 64);

        let (denied, _) = call(
            &mut state,
            "GET",
            "/v1/agents/me",
            "",
            Some("fe2_0123456789abcdef0123456789abcdef"),
        );
        assert_eq!(denied, 401);

        let me = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me?now={NOW}"),
            "",
            Some(&alpha),
        );
        assert_eq!(me.0, 200);
        assert_eq!(me.1["handle"], "alpha");
        assert_eq!(me.1["points"], 0);

        let bravo = register_one(&mut state, "bravo");
        let queue = format!("/v1/arenas/vault/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&alpha)).0, 200);
        assert!(state.book.matches.is_empty());
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&bravo)).0, 200);
        assert_eq!(state.book.matches.len(), 1);

        let listed = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/matches?now={NOW}"),
            "",
            Some(&alpha),
        );
        assert_eq!(listed.0, 200);
        let play = &listed.1["matches"][0];
        assert_eq!(play["match"]["yourSeat"], 0);
        assert_eq!(play["match"]["turn"], 1);
        assert_eq!(play["arena"]["name"], "Vault");
        assert!(play["legalActions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "scout"));
        let match_id = play["match"]["matchId"].as_str().unwrap().to_string();

        let act_path = format!("/v1/matches/{match_id}/actions?now={NOW}");
        let acted = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"crack","intent":"open the first lock"},"clientRequestId":"req-1"}"#,
            Some(&alpha),
        );
        assert_eq!(acted.0, 200, "{}", acted.1);
        assert_eq!(acted.1["accepted"], true);
        assert_eq!(acted.1["resolved"], false);
        let replay = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"crack","intent":"open the first lock"},"clientRequestId":"req-1"}"#,
            Some(&alpha),
        );
        assert_eq!(replay.0, 200);
        assert_eq!(state.book.matches[0].actions.len(), 0);
        let bravo_act = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":1,"action":{"type":"sabotage","target":"alpha","intent":"stop the thief"},"clientRequestId":"req-2"}"#,
            Some(&bravo),
        );
        assert_eq!(bravo_act.0, 200, "{}", bravo_act.1);
        assert_eq!(bravo_act.1["resolved"], true);
        assert_eq!(state.book.matches[0].actions.len(), 2);
        let scored = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":2,"action":{"type":"crack","intent":"open the first lock"},"clientRequestId":"req-3"}"#,
            Some(&alpha),
        );
        assert_eq!(scored.0, 200);
        assert_eq!(scored.1["resolved"], false);
        let close = call(
            &mut state,
            "POST",
            &act_path,
            r#"{"turn":2,"action":{"type":"cover"},"clientRequestId":"req-4"}"#,
            Some(&bravo),
        );
        assert_eq!(close.0, 200, "{}", close.1);
        assert_eq!(close.1["resolved"], true);

        let board = call(
            &mut state,
            "GET",
            &format!("/v1/leaderboard?window=all&now={NOW}"),
            "",
            None,
        );
        assert_eq!(board.0, 200);
        let rows = board.1["rows"].as_array().unwrap();
        assert!(rows
            .iter()
            .all(|row| row["agent"] != "alpha" && row["agent"] != "bravo"));
        assert!(rows
            .iter()
            .all(|row| row["games"].as_u64().unwrap_or(0) > 0));
        assert!(rows
            .iter()
            .all(|row| row["points"].as_u64().unwrap_or(0) > 0));

        let (hall_status, hall) = call(
            &mut state,
            "GET",
            &format!("/v1/hall-of-fame?now={NOW}"),
            "",
            None,
        );
        assert_eq!(hall_status, 200);
        assert!(hall["champions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["agent"] != "alpha"));
        assert_eq!(hall["rooms"].as_array().unwrap().len(), 4);

        let profile = call(
            &mut state,
            "GET",
            &format!("/v1/agents/alpha?now={NOW}"),
            "",
            None,
        );
        assert_eq!(profile.0, 200);
        assert_eq!(profile.1["points"], 0);
        assert!(profile.1.get("apiToken").is_none());

        let spec = call(&mut state, "GET", "/openapi.json", "", None);
        assert_eq!(spec.0, 200);
        let text = spec.1.to_string();
        assert!(!text.contains("429"));
        for path in [
            "/v1/agents/register",
            "/v1/agents/me/matches",
            "/v1/matches/{matchId}/actions",
            "/v1/matches/{matchId}/react",
            "/v1/war/challenges/{challengeId}/submit",
            "/v1/lobby/posts",
            "/v1/lobby/posts/{postId}/react",
            "/v1/lobby/posts/{postId}/report",
            "/v1/lobby/ws",
            "/v1/hall-of-fame",
            "/llms.txt",
        ] {
            assert!(text.contains(path), "{path}");
        }
        let skill = dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/llms.txt",
                body: "",
                authorization: None,
                ingest_token: None,
                policy_check: None,
            },
        );
        match skill {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 200);
                assert!(body.contains("https://fe2vi.space/skill/SKILL.md"));
                assert!(body.contains("https://api.fe2vi.space"));
                assert!(!body.contains("fe2vi.xyz"));
                assert!(body.contains("## Errors and limits"));
                assert!(body.contains("rate_limited"));
                assert!(body.contains("Retry-After"));
                assert!(body.contains("5 per hour"));
                assert!(body.contains("120 per minute"));
                assert!(body.contains("300 per minute"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn deadlines_pass_and_tokens_rotate() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let bravo = register_one(&mut state, "bravo");
        let queue = format!("/v1/arenas/vault/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&alpha)).0, 200);
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&bravo)).0, 200);
        let later = NOW + TURN_SECS + 5;
        let listed = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/matches?now={later}"),
            "",
            Some(&bravo),
        );
        assert_eq!(listed.1["matches"][0]["match"]["turn"], 2);
        assert_eq!(state.book.matches[0].actions[0].action_type, "pass");
        let rotated = call(
            &mut state,
            "POST",
            "/v1/agents/me/rotate-token",
            "{}",
            Some(&alpha),
        );
        assert_eq!(rotated.0, 200);
        let next = rotated.1["apiToken"].as_str().unwrap().to_string();
        assert_ne!(next, alpha);
        assert_eq!(
            call(&mut state, "GET", "/v1/agents/me", "", Some(&alpha)).0,
            401
        );
        assert_eq!(
            call(&mut state, "GET", "/v1/agents/me", "", Some(&next)).0,
            200
        );
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn storage_plan_is_incremental_and_arena_json_omits_the_book() {
        let player = Player {
            agent_id: "ag".into(),
            handle: "alpha".into(),
            name: "Alpha".into(),
            description: "d".into(),
            owner_contact: None,
            token_hash: "deadbeefdeadbeef".into(),
            token_prefix: "fe2_dead".into(),
            status: "active".into(),
            created_unix: 10,
            wallets: Wallets::default(),
            capabilities: Vec::new(),
            skin_id: "ingot".into(),
            war_hits: Vec::new(),
        };
        let done = PlayMatch {
            match_id: "mt_done".into(),
            arena_id: "vault".into(),
            status: "over".into(),
            turn: 2,
            deadline_unix: 50,
            seats: vec!["ag".into(), "bg".into()],
            actions: vec![Played {
                agent_id: "ag".into(),
                turn: 1,
                action_type: "scout".into(),
                points: 1,
                unix: 40,
                intent: None,
                outcome: None,
            }],
            created_unix: 30,
            vault: None,
        };
        let live = PlayMatch {
            match_id: "mt_live".into(),
            arena_id: "vault".into(),
            status: "live".into(),
            turn: 1,
            deadline_unix: 90,
            seats: vec!["ag".into(), "bg".into()],
            actions: Vec::new(),
            created_unix: 80,
            vault: None,
        };
        let mut before = PlayerBook::default();
        before.players.push(player);
        before.matches.push(done);
        before.matches.push(live);
        before.seen.push(Seen {
            agent_id: "ag".into(),
            client_request_id: "finished".into(),
            status: 200,
            body: "{}".into(),
            match_id: "mt_done".into(),
            created_unix: 40,
        });
        before.seen.push(Seen {
            agent_id: "ag".into(),
            client_request_id: "old".into(),
            status: 200,
            body: "{}".into(),
            match_id: String::new(),
            created_unix: 1,
        });
        before.seen.push(Seen {
            agent_id: "ag".into(),
            client_request_id: "live-row".into(),
            status: 200,
            body: "{}".into(),
            match_id: "mt_live".into(),
            created_unix: 100,
        });

        let mut state = ArenaState {
            book: before.clone(),
            ..ArenaState::default()
        };
        let now = 1 + SEEN_KEEP_SECS;
        assert!(prune_seen(&mut state, now));
        assert_eq!(state.book.seen.len(), 1);
        state.book.matches[1].actions.push(Played {
            agent_id: "ag".into(),
            turn: 1,
            action_type: "crack".into(),
            points: 0,
            unix: now,
            intent: None,
            outcome: Some("opened".into()),
        });
        state.book.seen.push(Seen {
            agent_id: "ag".into(),
            client_request_id: "new".into(),
            status: 200,
            body: "{}".into(),
            match_id: "mt_live".into(),
            created_unix: now,
        });
        let plan = plan_writes(&before, &state.book);
        assert!(plan.upsert_players.is_empty());
        assert!(plan.delete_player_ids.is_empty());
        assert_eq!(plan.upsert_matches.len(), 1);
        assert_eq!(plan.upsert_matches[0].match_id, "mt_live");
        assert_eq!(plan.upsert_actions.len(), 1);
        assert_eq!(plan.upsert_actions[0].1.action_type, "crack");
        assert!(plan.delete_actions.is_empty());
        assert_eq!(plan.delete_seen.len(), 2);
        assert_eq!(plan.upsert_seen.len(), 1);
        assert!(!plan.is_empty());

        let mut baseline = state.book.clone();
        baseline.players[0].token_hash.clear();
        baseline.players[0].token_prefix.clear();
        baseline.matches[0].actions.clear();
        baseline.matches[1].actions.clear();
        let backfill = plan_writes(&baseline, &state.book);
        assert_eq!(backfill.upsert_players.len(), 1);
        assert_eq!(backfill.upsert_actions.len(), 2);
        assert!(backfill.delete_player_ids.is_empty());
        assert!(backfill.delete_match_ids.is_empty());

        let encoded = serde_json::to_string(&state).unwrap();
        assert!(!encoded.contains("deadbeefdeadbeef"));
        assert!(!encoded.contains("token_hash"));
        let mut legacy = serde_json::to_value(&state).unwrap();
        legacy["book"] = serde_json::json!({
            "players": [{
                "agent_id": "ag",
                "handle": "alpha",
                "name": "Alpha",
                "description": "d",
                "owner_contact": null,
                "token_hash": "deadbeefdeadbeef",
                "token_prefix": "fe2_dead",
                "status": "active",
                "created_unix": 1
            }]
        });
        let restored: ArenaState = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.book.players[0].token_hash, "deadbeefdeadbeef");
        let again = serde_json::to_string(&restored).unwrap();
        assert!(!again.contains("deadbeefdeadbeef"));
        assert!(plan_writes(&state.book, &state.book).is_empty());
    }

    fn openapi_templates() -> Vec<String> {
        let doc: serde_json::Value = serde_json::from_str(&openapi()).unwrap();
        doc["paths"].as_object().unwrap().keys().cloned().collect()
    }

    fn path_char(ch: char, path: &str) -> bool {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '/' | '_' | '-' | '{' | '}')
            || (ch == '.' && (path.starts_with("/openapi") || path.starts_with("/llms")))
    }

    fn mentioned_api_paths(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(offset) = ["/v1/", "/openapi.json", "/llms.txt"]
            .iter()
            .filter_map(|needle| rest.find(needle))
            .min()
        {
            rest = &rest[offset..];
            let mut path = String::new();
            let mut end = 0;
            for ch in rest.chars() {
                if !path_char(ch, &path) {
                    break;
                }
                path.push(ch);
                end += ch.len_utf8();
            }
            while path.ends_with('.') && !path.ends_with(".json") && !path.ends_with(".txt") {
                path.pop();
            }
            if path.starts_with("/v1/") || path == "/openapi.json" || path == "/llms.txt" {
                found.push(path);
            }
            rest = &rest[end.max(1)..];
        }
        found.sort();
        found.dedup();
        found
    }

    fn same_shape(template: &str, path: &str) -> bool {
        let left: Vec<&str> = template
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let right: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        if left.len() != right.len() {
            return false;
        }
        left.iter()
            .zip(right.iter())
            .all(|(template_part, path_part)| {
                template_part == path_part
                    || (template_part.starts_with('{') && template_part.ends_with('}'))
                    || (path_part.starts_with('{') && path_part.ends_with('}'))
            })
    }

    fn path_is_documented(path: &str, templates: &[String]) -> bool {
        templates.iter().any(|template| same_shape(template, path))
    }

    #[test]
    fn finished_matches_stay_on_my_matches() {
        let mut state = ArenaState::default();
        let alpha = register_one(&mut state, "alpha");
        let bravo = register_one(&mut state, "bravo");
        let queue = format!("/v1/arenas/vault/queue?now={NOW}");
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&alpha)).0, 200);
        assert_eq!(call(&mut state, "POST", &queue, "{}", Some(&bravo)).0, 200);
        state.book.matches[0].status = "over".to_string();
        let listed = call(
            &mut state,
            "GET",
            &format!("/v1/agents/me/matches?now={NOW}"),
            "",
            Some(&alpha),
        );
        assert_eq!(listed.0, 200, "{}", listed.1);
        let play = &listed.1["matches"][0];
        assert_eq!(play["match"]["status"], "over");
        assert_eq!(play["match"]["result"], "draw");
        let feed = call(&mut state, "GET", "/v1/matches?limit=5", "", None);
        assert_eq!(feed.0, 200, "{}", feed.1);
        let row = &feed.1["matches"][0];
        assert_eq!(row["arenaId"], "vault");
        assert_eq!(row["status"], "over");
        assert!(row["vault"]["rounds"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn readme_endpoints_are_in_openapi() {
        let readme = include_str!("../../../README.md");
        let templates = openapi_templates();
        let mentioned = mentioned_api_paths(readme);
        assert!(
            mentioned.iter().any(|path| path == "/v1/agents/register"),
            "README scan found no register path: {mentioned:?}"
        );
        let missing: Vec<String> = mentioned
            .into_iter()
            .filter(|path| !path_is_documented(path, &templates))
            .collect();
        assert!(
            missing.is_empty(),
            "README names API paths that are not in /openapi.json: {missing:?}"
        );
    }

    #[test]
    fn readme_path_check_rejects_an_unknown_route() {
        let templates = openapi_templates();
        assert!(path_is_documented("/v1/agents/register", &templates));
        assert!(path_is_documented("/v1/arenas/vault/queue", &templates));
        assert!(path_is_documented(
            "/v1/matches/{matchId}/actions",
            &templates
        ));
        assert!(path_is_documented("/openapi.json", &templates));
        assert!(!path_is_documented("/v1/not-real", &templates));
        assert!(!path_is_documented("/v1/agents/me/not-a-route", &templates));
        let sample = "See GET /v1/nope and GET /v1/leaderboard.";
        let mentioned = mentioned_api_paths(sample);
        assert!(mentioned.contains(&"/v1/nope".to_string()));
        assert!(mentioned.contains(&"/v1/leaderboard".to_string()));
    }
}

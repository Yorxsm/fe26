//! Host-testable arena relay. Chain state is the trust root. This process
//! recomputes hashes and refuses games that exceed platform caps. It cannot
//! raise those caps. When no ingest token is configured the relay is open and
//! every response says so.

use fe2vi_core::{
    default_platform_caps, pin_hash, render_game_md, validate_rules, FollowEdge, GamePointer,
    GameRules, PlatformCaps, SCOPED_METHODS, TEMPLATE_CUP, TEMPLATE_HEIST,
};
use fe2vi_intents::StoredGrant;
use serde::{Deserialize, Serialize};

const HISTORY_LIMIT: usize = 200;

pub fn history_limit() -> usize {
    HISTORY_LIMIT
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArenaState {
    pub games: Vec<StoredGame>,
    pub agents: Vec<StoredAgent>,
    pub events: Vec<FeedEvent>,
    pub next_event: u64,
    pub caps: PlatformCaps,
    #[serde(default)]
    pub indexed_height: u64,
    #[serde(default)]
    pub seen_chain: Vec<String>,
    #[serde(default)]
    pub follows: Vec<FollowEdge>,
    #[serde(default)]
    pub grants: Vec<StoredGrant>,
    #[serde(default)]
    pub book: crate::agents::PlayerBook,
}

impl Default for ArenaState {
    fn default() -> Self {
        Self {
            games: Vec::new(),
            agents: Vec::new(),
            events: Vec::new(),
            next_event: 1,
            caps: default_platform_caps(),
            indexed_height: 0,
            seen_chain: Vec::new(),
            follows: Vec::new(),
            grants: Vec::new(),
            book: crate::agents::PlayerBook::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredGame {
    pub id: String,
    pub creator: String,
    pub rules: GameRules,
    pub delisted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredAgent {
    pub account_id: String,
    pub revoked: bool,
    pub allowed_methods: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedEvent {
    pub id: u64,
    pub game_id: String,
    pub kind: String,
    pub summary: String,
    /// True only when this worker read the log from a testnet RPC response.
    #[serde(default)]
    pub verified: bool,
    #[serde(default = "relay_source")]
    pub source: String,
}

fn relay_source() -> String {
    "relay".to_string()
}

#[derive(Clone, Debug, Serialize)]
pub struct Trust {
    pub hash_pinned: bool,
    pub within_platform_caps: bool,
    pub known_template: bool,
    pub delisted: bool,
    pub creator_text: &'static str,
    pub network: &'static str,
    pub relay: &'static str,
}

pub struct Incoming<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a str,
    pub authorization: Option<&'a str>,
    pub ingest_token: Option<&'a str>,
}

pub struct Fanout {
    pub game_id: String,
    pub message: String,
}

pub enum Outcome {
    Http {
        status: u16,
        content_type: &'static str,
        body: String,
        mutated: bool,
        fanout: Option<Fanout>,
        location: Option<String>,
    },
    Websocket {
        game_id: String,
        history: String,
    },
}

pub fn dispatch(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    if incoming.method == "OPTIONS" {
        return http(204, "text/plain", String::new(), false, None);
    }
    if let Some(location) = legacy_game_redirect(incoming.path) {
        return http_at(
            308,
            "text/plain",
            String::new(),
            false,
            None,
            Some(location),
        );
    }
    if let Some(outcome) = crate::agents::dispatch_player(state, incoming) {
        return outcome;
    }
    let parts = split_path(incoming.path);
    match (incoming.method, parts.as_slice()) {
        ("GET", ["health"]) | ("GET", ["v1", "health"]) => http(
            200,
            "application/json",
            r#"{"ok":true,"network":"testnet"}"#.into(),
            false,
            None,
        ),
        ("POST", ["v1", "index"]) => http(
            403,
            "application/json",
            json_error("chain index does not accept a request body"),
            false,
            None,
        ),
        ("GET", ["v1", "caps"]) => json(200, &state.caps, false, None),
        ("GET", ["v1", "games"]) => json(200, &game_cards(state), false, None),
        ("GET", ["v1", "games", id]) => match state.games.iter().find(|game| game.id == *id) {
            Some(game) => json(200, &card(state, game), false, None),
            None => http(
                404,
                "application/json",
                r#"{"error":"game not found"}"#.into(),
                false,
                None,
            ),
        },
        ("GET", ["v1", "games", id, "GAME.md"]) => {
            match state.games.iter().find(|game| game.id == *id) {
                Some(game) => match render_game_md(&game.rules, &pointer(id)) {
                    Ok(markdown) => {
                        http(200, "text/markdown; charset=utf-8", markdown, false, None)
                    }
                    Err(err) => http(400, "application/json", json_error(err), false, None),
                },
                None => http(
                    404,
                    "application/json",
                    r#"{"error":"game not found"}"#.into(),
                    false,
                    None,
                ),
            }
        }
        ("GET", ["v1", "games", id, "events"]) => {
            let events: Vec<&FeedEvent> = state
                .events
                .iter()
                .filter(|event| event.game_id == *id)
                .collect();
            json(200, &events, false, None)
        }
        ("GET", ["v1", "games", id, "ws"]) => {
            let events: Vec<&FeedEvent> = state
                .events
                .iter()
                .filter(|event| event.game_id == *id)
                .collect();
            Outcome::Websocket {
                game_id: (*id).to_string(),
                history: serde_json::to_string(&events).unwrap_or_else(|_| "[]".into()),
            }
        }
        ("GET", ["v1", "agents", id]) => {
            match state.agents.iter().find(|agent| agent.account_id == *id) {
                Some(agent) => json(200, &agent_view(state, agent), false, None),
                None => http(
                    404,
                    "application/json",
                    r#"{"error":"agent not found"}"#.into(),
                    false,
                    None,
                ),
            }
        }
        ("POST", ["v1", "agents"]) => ingest_agent(state, incoming),
        ("POST", ["v1", "events"]) => ingest_event(state, incoming),
        ("GET", ["v1", "city"]) => crate::surface::city(),
        ("GET", ["v1", "village", "leaderboard"]) => crate::surface::leaderboard(
            query_param(incoming.path, "window"),
            query_now(incoming.path),
        ),
        ("GET", ["v1", "village", "tape"]) => crate::surface::tape(query_now(incoming.path)),
        ("GET", ["v1", "schedule"]) => crate::surface::schedule(query_now(incoming.path)),
        ("GET", ["v1", "leaderboard"]) => crate::surface::public_board(
            state,
            query_param(incoming.path, "window"),
            query_param(incoming.path, "room").or_else(|| query_param(incoming.path, "arena")),
            query_param(incoming.path, "agent"),
            query_now(incoming.path),
            query_param(incoming.path, "limit"),
            query_param(incoming.path, "offset"),
        ),
        ("GET", ["v1", "rules"]) => crate::surface::point_rules(),
        ("GET", ["v1", "heist", "leaderboard"]) => crate::surface::heist_board(
            query_param(incoming.path, "window"),
            query_now(incoming.path),
        ),
        ("GET", ["v1", "venues"]) => crate::surface::venues(query_now(incoming.path)),
        ("GET", ["v1", "challenges"]) => crate::surface::challenges(),
        ("GET", ["v1", "events"]) => crate::surface::events(query_now(incoming.path)),
        ("GET", ["v1", "stats"]) => {
            crate::surface::spectator_stats(state, query_now(incoming.path))
        }
        ("GET", ["v1", "war", "GAME.md"]) => crate::surface::war_md(),
        ("POST", ["v1", "village", "follow"]) => crate::surface::follow(state, incoming, true),
        ("POST", ["v1", "village", "unfollow"]) => crate::surface::follow(state, incoming, false),
        ("GET", ["v1", "intents", "policy"]) => crate::surface::intents_policy(&state.caps),
        ("POST", ["v1", "intents", "grants"]) => crate::surface::issue_grant(state, incoming),
        ("POST", ["v1", "intents", "revoke"]) => crate::surface::revoke_grant(state, incoming),
        ("POST", ["v1", "intents", "act"]) => crate::surface::intents_act(state, incoming),
        _ => http(
            404,
            "application/json",
            r#"{"error":"not found"}"#.into(),
            false,
            None,
        ),
    }
}

fn ingest_agent(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    if let Some(rejection) = reject_ingest(incoming) {
        return rejection;
    }
    let body: AgentIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                json_error(&err.to_string()),
                false,
                None,
            )
        }
    };
    if body.account_id.is_empty() {
        return http(
            400,
            "application/json",
            json_error("account id required"),
            false,
            None,
        );
    }
    let scoped: Vec<&str> = SCOPED_METHODS.to_vec();
    let presented: Vec<&str> = body.allowed_methods.iter().map(String::as_str).collect();
    if presented != scoped {
        return http(
            400,
            "application/json",
            json_error("function-call key cannot call methods outside its scope"),
            false,
            None,
        );
    }
    if let Some(existing) = state
        .agents
        .iter_mut()
        .find(|agent| agent.account_id == body.account_id)
    {
        existing.revoked = body.revoked;
        existing.allowed_methods = body.allowed_methods;
    } else {
        state.agents.push(StoredAgent {
            account_id: body.account_id,
            revoked: body.revoked,
            allowed_methods: body.allowed_methods,
        });
    }
    json(
        201,
        &serde_json::json!({"ok": true, "relay": "unverified"}),
        true,
        None,
    )
}

fn ingest_event(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    if let Some(rejection) = reject_ingest(incoming) {
        return rejection;
    }
    let body: EventIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                json_error(&err.to_string()),
                false,
                None,
            )
        }
    };
    if !valid_id(&body.game_id) || body.kind.is_empty() {
        return http(
            400,
            "application/json",
            json_error("invalid event"),
            false,
            None,
        );
    }
    let Some(summary) = clean_summary(&body.summary) else {
        return http(
            400,
            "application/json",
            json_error("summary rejected"),
            false,
            None,
        );
    };
    let event = FeedEvent {
        id: state.next_event,
        game_id: body.game_id.clone(),
        kind: body.kind,
        summary,
        verified: false,
        source: "relay".to_string(),
    };
    state.next_event += 1;
    state.events.push(event.clone());
    if state.events.len() > HISTORY_LIMIT {
        let extra = state.events.len() - HISTORY_LIMIT;
        state.events.drain(0..extra);
    }
    let message = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
    json(
        201,
        &event,
        true,
        Some(Fanout {
            game_id: body.game_id,
            message,
        }),
    )
}

fn reject_ingest(incoming: &Incoming<'_>) -> Option<Outcome> {
    if ingest_allowed(incoming.ingest_token, incoming.authorization) {
        None
    } else {
        Some(http(
            401,
            "application/json",
            json_error("ingest unauthorized"),
            false,
            None,
        ))
    }
}

pub fn ingest_allowed(configured: Option<&str>, presented: Option<&str>) -> bool {
    match configured.map(str::trim).filter(|token| !token.is_empty()) {
        None => true,
        Some(expected) => presented == Some(&format!("Bearer {expected}")),
    }
}

fn game_cards(state: &ArenaState) -> Vec<serde_json::Value> {
    state.games.iter().map(|game| card(state, game)).collect()
}

fn card(state: &ArenaState, game: &StoredGame) -> serde_json::Value {
    serde_json::json!({
        "id": game.id,
        "creator": game.creator,
        "rules": game.rules,
        "game_md_hash": pin_hash(&game.rules),
        "delisted": game.delisted,
        "verified_events": state.events.iter().filter(|event| event.game_id == game.id && event.verified).count(),
        "trust": trust(state, game),
    })
}

fn trust(state: &ArenaState, game: &StoredGame) -> Trust {
    Trust {
        hash_pinned: true,
        within_platform_caps: validate_rules(&game.rules, &state.caps).is_ok(),
        known_template: game.rules.template_id == TEMPLATE_HEIST
            || game.rules.template_id == TEMPLATE_CUP,
        delisted: game.delisted,
        creator_text: "untrusted",
        network: "testnet",
        relay: "unverified",
    }
}

fn agent_view(state: &ArenaState, agent: &StoredAgent) -> serde_json::Value {
    serde_json::json!({
        "account_id": agent.account_id,
        "revoked": agent.revoked,
        "allowed_methods": agent.allowed_methods,
        "platform_caps": state.caps,
        "network": "testnet",
        "relay": "unverified",
        "note": "Caps shown here are the platform caps. A game cannot raise them. Revocation on this relay is unverified until you read the registry view.",
    })
}

fn pointer(game_id: &str) -> GamePointer {
    GamePointer {
        network: "testnet".into(),
        factory: "factory.fe2vi.testnet".into(),
        game_id: game_id.to_string(),
    }
}

fn path_only(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

pub(crate) fn query_param<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let query = path.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then_some(value)
    })
}

/// `Request::path` drops the query string. `now` and `window` live there.
/// The wasm entry is the production caller. Host builds reach it from tests.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn path_with_query(path: &str, query: Option<&str>) -> String {
    match query {
        Some(query) if !query.is_empty() && !path.contains('?') => format!("{path}?{query}"),
        _ => path.to_string(),
    }
}

/// Explicit `?now=` wins. Host tests otherwise stay on [`fe2vi_core::DEMO_EPOCH`].
/// The deployed worker uses the wall clock.
pub(crate) fn query_now(path: &str) -> u64 {
    query_param(path, "now")
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(wall_unix)
}

fn wall_unix() -> u64 {
    #[cfg(test)]
    {
        fe2vi_core::DEMO_EPOCH
    }
    #[cfg(not(test))]
    {
        wall_unix_runtime()
    }
}

#[cfg(all(not(test), target_arch = "wasm32"))]
fn wall_unix_runtime() -> u64 {
    worker::Date::now().as_millis() / 1000
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn wall_unix_runtime() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(fe2vi_core::DEMO_EPOCH)
}

fn split_path(path: &str) -> Vec<&str> {
    path_only(path)
        .trim_end_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_' || *byte == b'.'
        })
}

fn clean_summary(summary: &str) -> Option<String> {
    let trimmed = summary.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 180 {
        return None;
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(trimmed.to_string())
}

fn json_error(message: &str) -> String {
    serde_json::json!({"error": message}).to_string()
}

pub(crate) fn json(
    status: u16,
    body: &impl Serialize,
    mutated: bool,
    fanout: Option<Fanout>,
) -> Outcome {
    http(
        status,
        "application/json",
        serde_json::to_string(body).unwrap_or_else(|_| r#"{"error":"encode"}"#.into()),
        mutated,
        fanout,
    )
}

pub(crate) fn http(
    status: u16,
    content_type: &'static str,
    body: String,
    mutated: bool,
    fanout: Option<Fanout>,
) -> Outcome {
    http_at(status, content_type, body, mutated, fanout, None)
}

fn http_at(
    status: u16,
    content_type: &'static str,
    body: String,
    mutated: bool,
    fanout: Option<Fanout>,
    location: Option<String>,
) -> Outcome {
    Outcome::Http {
        status,
        content_type,
        body,
        mutated,
        fanout,
        location,
    }
}

/// Old public ids keep working. The short id is the game route.
fn legacy_game_redirect(path: &str) -> Option<String> {
    let (path, query) = match path.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path, None),
    };
    let mut parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 3 || parts[0] != "v1" || parts[1] != "games" {
        return None;
    }
    let next = match parts[2] {
        "heist-night-template" => "heist-night",
        "trading-cup-template" => "trading-cup",
        _ => return None,
    };
    parts[2] = next;
    let mut out = format!("/{}", parts.join("/"));
    if let Some(query) = query {
        out.push('?');
        out.push_str(query);
    }
    Some(out)
}

#[derive(Deserialize)]
struct AgentIn {
    account_id: String,
    revoked: bool,
    allowed_methods: Vec<String>,
}

#[derive(Deserialize)]
struct EventIn {
    game_id: String,
    kind: String,
    summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2vi_core::{extract_rules_json, game_md_violations, heist_night_example, ONE_NEAR};

    fn post<'a>(path: &'a str, body: &'a str) -> Incoming<'a> {
        Incoming {
            method: "POST",
            path,
            body,
            authorization: None,
            ingest_token: None,
        }
    }

    #[test]
    fn posting_a_game_is_gone_and_caps_still_reject_a_raise() {
        let mut state = ArenaState::default();
        let mut rules = heist_night_example();
        rules.per_agent_cap_yocto = state.caps.max_per_agent_yocto + ONE_NEAR;
        let err = validate_rules(&rules, &state.caps).unwrap_err();
        assert!(err.to_string().contains("per-agent cap"));
        let body = serde_json::json!({
            "id": "g9",
            "creator": "creator.fe2vi.testnet",
            "rules": rules,
        })
        .to_string();
        match dispatch(&mut state, &post("/v1/games", &body)) {
            Outcome::Http { status, .. } => assert_eq!(status, 404),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        assert!(state.games.is_empty());
    }

    #[test]
    fn function_call_key_cannot_call_methods_outside_its_scope() {
        let mut state = ArenaState::default();
        let body = serde_json::json!({
            "account_id": "alpha.registry.fe2vi.testnet",
            "revoked": false,
            "allowed_methods": ["act", "bid", "transfer_item", "resolve"],
        })
        .to_string();
        match dispatch(&mut state, &post("/v1/agents", &body)) {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 400);
                assert!(body.contains("outside its scope"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        assert!(state.agents.is_empty());
    }

    #[test]
    fn old_game_routes_redirect_to_the_short_id() {
        let mut state = ArenaState::default();
        let incoming = Incoming {
            method: "GET",
            path: "/v1/games/heist-night-template/GAME.md",
            body: "",
            authorization: None,
            ingest_token: None,
        };
        match dispatch(&mut state, &incoming) {
            Outcome::Http {
                status, location, ..
            } => {
                assert_eq!(status, 308);
                assert_eq!(location.as_deref(), Some("/v1/games/heist-night/GAME.md"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                path: "/v1/games/trading-cup-template?watch=1",
                ..incoming
            },
        ) {
            Outcome::Http {
                status, location, ..
            } => {
                assert_eq!(status, 308);
                assert_eq!(location.as_deref(), Some("/v1/games/trading-cup?watch=1"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn serves_game_md_with_the_pinned_hash_and_no_command_channel() {
        let mut state = ArenaState::default();
        let rules = heist_night_example();
        state.games.push(StoredGame {
            id: "heist-night".into(),
            creator: "heist.fe2vi.testnet".into(),
            rules,
            delisted: false,
        });
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/games/heist-night/GAME.md",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http {
                status,
                body,
                content_type,
                ..
            } => {
                assert_eq!(status, 200);
                assert!(content_type.contains("markdown"));
                assert!(
                    game_md_violations(&body).is_empty(),
                    "{}",
                    game_md_violations(&body).join(",")
                );
                let json = extract_rules_json(&body).unwrap();
                let parsed: GameRules = serde_json::from_str(json).unwrap();
                assert!(body.contains(&fe2vi_core::pin_hash(&parsed)));
                assert!(body.contains("UNTRUSTED DATA"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn revoked_agent_is_visible_and_ingest_token_is_required_when_set() {
        let mut state = ArenaState::default();
        let body = serde_json::json!({
            "account_id": "alpha.registry.fe2vi.testnet",
            "revoked": true,
            "allowed_methods": ["act", "bid", "transfer_item"],
        })
        .to_string();
        let denied = dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/agents",
                body: &body,
                authorization: None,
                ingest_token: Some("local-only"),
            },
        );
        match denied {
            Outcome::Http { status, .. } => assert_eq!(status, 401),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "POST",
                path: "/v1/agents",
                body: &body,
                authorization: Some("Bearer local-only"),
                ingest_token: Some("local-only"),
            },
        ) {
            Outcome::Http { status, .. } => assert_eq!(status, 201),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/agents/alpha.registry.fe2vi.testnet",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                assert!(body.contains("\"revoked\":true"));
                assert!(body.contains("unverified"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn spectator_event_is_kept_and_labeled_for_fanout() {
        let mut state = ArenaState::default();
        let body = r#"{"game_id":"g1","kind":"action_taken","summary":"alice played scout"}"#;
        match dispatch(&mut state, &post("/v1/events", body)) {
            Outcome::Http { status, fanout, .. } => {
                assert_eq!(status, 201);
                let fanout = fanout.expect("fanout");
                assert_eq!(fanout.game_id, "g1");
                assert!(fanout.message.contains("scout"));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/games/g1/ws",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Websocket { game_id, history } => {
                assert_eq!(game_id, "g1");
                assert!(history.contains("scout"));
            }
            Outcome::Http { .. } => panic!("expected websocket"),
        }
    }

    #[test]
    fn public_post_cannot_mark_an_event_verified() {
        let mut state = ArenaState::default();
        let body = r#"{"game_id":"g1","kind":"action_taken","summary":"alice played scout","verified":true,"source":"chain"}"#;
        match dispatch(&mut state, &post("/v1/events", body)) {
            Outcome::Http { status, body, .. } => {
                assert_eq!(status, 201);
                assert!(body.contains("\"verified\":false"));
                assert!(body.contains("\"source\":\"relay\""));
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(&mut state, &post("/v1/index", r#"{"verified":true}"#)) {
            Outcome::Http {
                status, mutated, ..
            } => {
                assert_eq!(status, 403);
                assert!(!mutated);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        assert!(!state.events[0].verified);
    }

    #[test]
    fn query_string_reaches_the_router_and_tests_keep_the_demo_clock() {
        assert_eq!(
            path_with_query("/v1/heist/leaderboard", Some("window=week")),
            "/v1/heist/leaderboard?window=week"
        );
        assert_eq!(
            path_with_query("/v1/schedule?now=5", Some("now=9")),
            "/v1/schedule?now=5"
        );
        let mut state = ArenaState::default();
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/schedule",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["now"], fe2vi_core::DEMO_EPOCH);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let pinned = path_with_query("/v1/schedule", Some("now=1790812800"));
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: &pinned,
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["now"], 1_790_812_800u64);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/heist/leaderboard?window=week&now=1790812800",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["window"], "week");
                assert_eq!(json["now"], 1_790_812_800u64);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn stats_counts_the_roster_and_ignores_the_example_event() {
        let mut state = ArenaState::default();
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/stats",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["network"], "mock");
                assert_eq!(json["source"], "mock");
                assert_eq!(json["roster"].as_array().unwrap().len(), 10);
                assert_eq!(json["agents"], 10);
                assert_eq!(json["now"], fe2vi_core::DEMO_EPOCH);
                assert_eq!(json["live_games"], 0);
                assert_eq!(json["heist_live"], false);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let body = serde_json::json!({
            "account_id": "extra.registry.fe2vi.testnet",
            "revoked": false,
            "allowed_methods": ["act", "bid", "transfer_item"],
        })
        .to_string();
        match dispatch(&mut state, &post("/v1/agents", &body)) {
            Outcome::Http { status, .. } => assert_eq!(status, 201),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/stats",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["agents"], 11);
                assert_eq!(json["roster"].as_array().unwrap().len(), 10);
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        match dispatch(
            &mut state,
            &Incoming {
                method: "GET",
                path: "/v1/events?now=1793116800",
                body: "",
                authorization: None,
                ingest_token: None,
            },
        ) {
            Outcome::Http { body, .. } => {
                let json: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(json["live"].as_array().unwrap().len(), 0);
                assert_eq!(json["events"][0]["phase"], "example");
                assert_eq!(json["events"][0]["run"], "running");
            }
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }
}

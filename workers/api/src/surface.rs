//! City, Trading Village, and the mock Intents adapter. None of these paths
//! mark an event verified, and none of them call the hosted Intents API.

use fe2vi_core::{
    demo_tape_at, districts, follow as follow_edge, leaderboard as rank, unfollow as unfollow_edge,
    Venue, Window, WRAP_NEAR,
};
use fe2vi_intents::{
    issue_session, live_gate, mock_act, policy_from_caps, redact_secrets, revoke_session,
    MOCK_USD_PER_NEAR_CENTS,
};
use serde::Deserialize;

use crate::logic::{http, json, query_now, ArenaState, Incoming, Outcome};

pub fn city() -> Outcome {
    let districts: Vec<serde_json::Value> = districts()
        .iter()
        .map(|district| {
            serde_json::json!({
                "id": district.id,
                "name": district.name,
                "summary": district.summary,
                "status": district.status,
                "href": district.href,
                "model": district.model,
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "network": "testnet",
            "site": "fe2vi.space",
            "districts": districts,
        }),
        false,
        None,
    )
}

pub fn leaderboard(window_raw: Option<&str>, now: u64) -> Outcome {
    let window = Window::parse(window_raw.unwrap_or("24h"));
    let Some(window) = window else {
        return http(
            400,
            "application/json",
            r#"{"error":"window must be 24h, 7d, 30d, or all"}"#.into(),
            false,
            None,
        );
    };
    let rows = rank(&demo_tape_at(now), now, window);
    json(
        200,
        &serde_json::json!({
            "window": window_raw.unwrap_or("24h"),
            "now": now,
            "source": "simulated",
            "asset": WRAP_NEAR,
            "rows": rows,
        }),
        false,
        None,
    )
}

pub fn schedule(now: u64) -> Outcome {
    let slot = fe2vi_core::next_heist(now);
    json(
        200,
        &serde_json::json!({
            "season": fe2vi_core::SEASON_ID,
            "season_start": fe2vi_core::SEASON_START,
            "season_end": fe2vi_core::SEASON_END,
            "now": now,
            "live": fe2vi_core::heist_live(now),
            "countdown_secs": fe2vi_core::countdown_secs(now),
            "next": slot,
            "daily": fe2vi_core::daily_slot(now),
            "weekly": fe2vi_core::weekly_slot(now),
            "seasonal": fe2vi_core::upcoming_season_slot(now),
            "daily_open_secs": 18 * 3600,
            "daily_len_secs": 3600,
            "weekly_len_secs": 2 * 3600,
            "seasonal_len_secs": 3 * 3600,
        }),
        false,
        None,
    )
}

pub fn heist_board(window_raw: Option<&str>, now: u64) -> Outcome {
    let Some(window) = fe2vi_core::HeistWindow::parse(window_raw.unwrap_or("today")) else {
        return http(
            400,
            "application/json",
            r#"{"error":"window must be today, week, or season"}"#.into(),
            false,
            None,
        );
    };
    json(
        200,
        &serde_json::json!({
            "window": window_raw.unwrap_or("today"),
            "now": now,
            "rows": fe2vi_core::heist_leaderboard(now, window),
        }),
        false,
        None,
    )
}

pub fn venues(now: u64) -> Outcome {
    let quotes = fe2vi_core::default_venue().quotes(now);
    let notes: Vec<serde_json::Value> = fe2vi_core::venue_notes()
        .iter()
        .map(|note| {
            serde_json::json!({
                "id": note.id,
                "offers": note.offers,
                "testnet": note.testnet,
                "wired": note.wired,
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "network": "mock",
            "quotes": quotes,
            "notes": notes,
        }),
        false,
        None,
    )
}

pub fn challenges() -> Outcome {
    let rows: Vec<serde_json::Value> = fe2vi_core::challenges()
        .iter()
        .map(|challenge| {
            serde_json::json!({
                "id": challenge.id,
                "category": challenge.category,
                "title": challenge.title,
                "flag_hash": challenge.flag_hash,
                "points": challenge.points,
                "start_unix": challenge.start_unix,
                "end_unix": challenge.end_unix,
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "targets": "fe2VI sandboxes only",
            "challenges": rows,
        }),
        false,
        None,
    )
}

pub fn events(now: u64) -> Outcome {
    let rows: Vec<serde_json::Value> = fe2vi_core::hosted_events()
        .iter()
        .map(|event| {
            serde_json::json!({
                "id": event.id,
                "partner_label": event.partner_label,
                "example_only": event.example_only,
                "room_id": event.room_id,
                "start_unix": event.start_unix,
                "end_unix": event.end_unix,
                "prize_text": event.prize_text,
                "rules_text": event.rules_text,
                "phase": fe2vi_core::event_phase(event, now),
                "run": fe2vi_core::example_run(event, now),
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "now": now,
            "events": rows,
            "live": fe2vi_core::live_events(now).iter().map(|event| event.id).collect::<Vec<_>>(),
        }),
        false,
        None,
    )
}

pub fn spectator_stats(state: &ArenaState, now: u64) -> Outcome {
    let mut ids: std::collections::BTreeSet<String> =
        fe2vi_core::agent_roster().into_iter().collect();
    for agent in &state.agents {
        if !agent.account_id.is_empty() {
            ids.insert(agent.account_id.clone());
        }
    }
    let roster: Vec<serde_json::Value> = fe2vi_core::city_roster()
        .iter()
        .map(|agent| serde_json::json!({ "account_id": agent.account_id, "name": agent.name }))
        .collect();
    let live_games = fe2vi_core::open_slots(now).len() + fe2vi_core::live_events(now).len();
    json(
        200,
        &serde_json::json!({
            "network": "mock",
            "source": "mock",
            "now": now,
            "agents": ids.len(),
            "roster": roster,
            "live_games": live_games,
            "heist_live": fe2vi_core::heist_live(now),
            "countdown_secs": fe2vi_core::countdown_secs(now),
        }),
        false,
        None,
    )
}

pub fn war_md() -> Outcome {
    http(
        200,
        "text/markdown; charset=utf-8",
        fe2vi_core::war_room_md(),
        false,
        None,
    )
}

pub fn tape(now: u64) -> Outcome {
    json(
        200,
        &serde_json::json!({
            "now": now,
            "source": "simulated",
            "fills": demo_tape_at(now),
        }),
        false,
        None,
    )
}

#[derive(Deserialize)]
struct FollowIn {
    follower: String,
    target: String,
}

pub fn follow(state: &mut ArenaState, incoming: &Incoming<'_>, add: bool) -> Outcome {
    let body: FollowIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                error_body(&err.to_string()),
                false,
                None,
            )
        }
    };
    if add {
        if let Err(err) = follow_edge(&mut state.follows, &body.follower, &body.target) {
            return http(400, "application/json", error_body(err), false, None);
        }
    } else {
        unfollow_edge(&mut state.follows, &body.follower, &body.target);
    }
    json(
        200,
        &serde_json::json!({ "ok": true, "follows": state.follows.len() }),
        true,
        None,
    )
}

pub fn intents_policy(caps: &fe2vi_core::PlatformCaps) -> Outcome {
    let live = live_gate(
        &std::env::var("FE2VI_INTENTS_LIVE").unwrap_or_default(),
        &std::env::var("NEAR_INTENTS_AGENT_API_KEY").unwrap_or_default(),
    );
    json(
        200,
        &serde_json::json!({
            "mode": if live { "live-dry" } else { "mock" },
            "broadcast": false,
            "usd_per_near_cents": MOCK_USD_PER_NEAR_CENTS,
            "policy": policy_from_caps(caps, MOCK_USD_PER_NEAR_CENTS),
        }),
        false,
        None,
    )
}

#[derive(Deserialize)]
struct GrantIn {
    game_id: String,
    agent: String,
    #[serde(default = "default_ttl")]
    ttl_secs: u64,
}

fn default_ttl() -> u64 {
    60 * 60
}

pub fn issue_grant(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let body: GrantIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                error_body(&err.to_string()),
                false,
                None,
            )
        }
    };
    let now = query_now(incoming.path);
    let issued = issue_session(
        &mut state.grants,
        &body.game_id,
        &body.agent,
        now,
        body.ttl_secs,
    );
    json(
        201,
        &serde_json::json!({
            "mode": issued.mode,
            "grant_id": issued.grant_id,
            "label": issued.label,
            "commitment": issued.commitment,
            "expires_at_unix": issued.expires_at_unix,
            "token": "withheld",
        }),
        true,
        None,
    )
}

#[derive(Deserialize)]
struct RevokeIn {
    grant_id: String,
}

pub fn revoke_grant(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let body: RevokeIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                error_body(&err.to_string()),
                false,
                None,
            )
        }
    };
    let found = revoke_session(&mut state.grants, &body.grant_id);
    json(
        if found { 200 } else { 404 },
        &serde_json::json!({ "ok": found, "grant_id": body.grant_id }),
        found,
        None,
    )
}

#[derive(Deserialize)]
struct ActIn {
    grant_id: String,
    amount_yocto: String,
}

pub fn intents_act(state: &mut ArenaState, incoming: &Incoming<'_>) -> Outcome {
    let body: ActIn = match serde_json::from_str(incoming.body) {
        Ok(body) => body,
        Err(err) => {
            return http(
                400,
                "application/json",
                error_body(&err.to_string()),
                false,
                None,
            )
        }
    };
    let amount: u128 = match body.amount_yocto.parse() {
        Ok(amount) => amount,
        Err(_) => {
            return http(
                400,
                "application/json",
                error_body("amount_yocto must be an integer string"),
                false,
                None,
            )
        }
    };
    let now = query_now(incoming.path);
    match mock_act(&mut state.grants, &body.grant_id, amount, &state.caps, now) {
        Ok(value) => json(200, &value, true, None),
        Err(err) => http(403, "application/json", error_body(err.code()), false, None),
    }
}

fn error_body(message: &str) -> String {
    serde_json::json!({ "error": redact_secrets(message) }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{dispatch, ArenaState};

    fn call<'a>(state: &mut ArenaState, method: &'a str, path: &'a str, body: &'a str) -> Outcome {
        dispatch(
            state,
            &Incoming {
                method,
                path,
                body,
                authorization: None,
                ingest_token: None,
            },
        )
    }

    fn body_of(outcome: Outcome) -> serde_json::Value {
        match outcome {
            Outcome::Http { body, .. } => serde_json::from_str(&body).unwrap(),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn city_lists_eight_districts_without_a_secret() {
        let mut state = ArenaState::default();
        let json = body_of(call(&mut state, "GET", "/v1/city", ""));
        assert_eq!(json["districts"].as_array().unwrap().len(), 8);
        assert_eq!(json["site"], "fe2vi.space");
        assert!(!json.to_string().contains("naa_"));
    }

    #[test]
    fn village_day_board_is_mason_and_follow_rejects_self() {
        let mut state = ArenaState::default();
        let board = body_of(call(
            &mut state,
            "GET",
            "/v1/village/leaderboard?window=24h",
            "",
        ));
        assert_eq!(board["rows"][0]["name"], "Mason");
        assert_eq!(board["source"], "simulated");
        let follow = call(
            &mut state,
            "POST",
            "/v1/village/follow",
            r#"{"follower":"mason.village.fe2vi.testnet","target":"mason.village.fe2vi.testnet"}"#,
        );
        match follow {
            Outcome::Http { status, .. } => assert_eq!(status, 400),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
    }

    #[test]
    fn intents_act_stays_mock_and_does_not_verify_events() {
        let mut state = ArenaState::default();
        let issued = body_of(call(
            &mut state,
            "POST",
            "/v1/intents/grants?now=1000",
            r#"{"game_id":"g1","agent":"mason.village.fe2vi.testnet","ttl_secs":3600}"#,
        ));
        assert_eq!(issued["token"], "withheld");
        let grant_id = issued["grant_id"].as_str().unwrap().to_string();
        let acted = body_of(call(
            &mut state,
            "POST",
            "/v1/intents/act?now=1000",
            &format!(r#"{{"grant_id":"{grant_id}","amount_yocto":"1"}}"#),
        ));
        assert_eq!(acted["mode"], "mock");
        assert_eq!(acted["broadcast"], false);
        assert!(state.events.iter().all(|event| !event.verified));
    }
}

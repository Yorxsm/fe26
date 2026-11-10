//! City, Trading Village, and the mock Intents adapter. None of these paths
//! mark an event verified, and none of them call the hosted Intents API.

use fe2vi_core::{
    districts, follow as follow_edge, unfollow as unfollow_edge, PointRoom, PublicRow,
};
use fe2vi_intents::{
    issue_session, live_gate, policy_from_caps, redact_secrets, revoke_session,
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
            "network": "off-chain",
            "site": "fe2vi.space",
            "districts": districts,
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
            "rooms": fe2vi_core::room_windows(now),
        }),
        false,
        None,
    )
}

pub fn public_board(
    state: &crate::logic::ArenaState,
    window_raw: Option<&str>,
    room_raw: Option<&str>,
    agent: Option<&str>,
    now: u64,
    limit_raw: Option<&str>,
    offset_raw: Option<&str>,
) -> Outcome {
    let Some(window) = fe2vi_core::BoardWindow::parse(window_raw.unwrap_or("season")) else {
        return http(
            400,
            "application/json",
            r#"{"error":"window must be today, week, season, or all"}"#.into(),
            false,
            None,
        );
    };
    let room = match room_raw.unwrap_or("all") {
        "all" => None,
        other => match fe2vi_core::PointRoom::parse(other) {
            Some(room) => Some(room),
            None => {
                return http(
                    400,
                    "application/json",
                    r#"{"error":"room must be all, vault, cup, village, or war"}"#.into(),
                    false,
                    None,
                );
            }
        },
    };
    let limit = match limit_raw {
        None => None,
        Some(raw) => match raw.parse::<usize>() {
            Ok(value) if (1..=100).contains(&value) => Some(value),
            _ => {
                return http(
                    400,
                    "application/json",
                    r#"{"error":"validation_error","message":"limit is 1 to 100"}"#.into(),
                    false,
                    None,
                );
            }
        },
    };
    let offset = match offset_raw.unwrap_or("0").parse::<usize>() {
        Ok(value) => value,
        Err(_) => {
            return http(
                400,
                "application/json",
                r#"{"error":"validation_error","message":"offset is a number"}"#.into(),
                false,
                None,
            );
        }
    };
    let mut rows = crate::agents::registered_rows(state, now, window, room);
    rows.sort_by(|left, right| {
        right
            .points
            .cmp(&left.points)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.agent.cmp(&right.agent))
    });
    let mut last_points = None;
    let mut last_rank = 0u32;
    for (index, row) in rows.iter_mut().enumerate() {
        if last_points == Some(row.points) {
            row.rank = last_rank;
        } else {
            last_rank = index as u32 + 1;
            last_points = Some(row.points);
            row.rank = last_rank;
        }
    }
    let page: Vec<_> = rows
        .into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect();
    let recent = agent.map(|id| {
        crate::agents::registered_recent(state, id, now, window).unwrap_or_else(|| {
            serde_json::to_value(fe2vi_core::recent_results(now, id, window))
                .ok()
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default()
        })
    });
    json(
        200,
        &serde_json::json!({
            "window": window.as_str(),
            "room": room.map(|room| room.as_str()).unwrap_or("all"),
            "arena": room.map(|room| room.as_str()).unwrap_or("all"),
            "now": now,
            "source": "arena",
            "rows": page,
            "recent": recent,
        }),
        false,
        None,
    )
}

pub fn hall_of_fame(state: &crate::logic::ArenaState, now: u64) -> Outcome {
    fn ranked(mut rows: Vec<PublicRow>) -> Vec<PublicRow> {
        rows.retain(|row| row.points > 0);
        rows.sort_by(|left, right| {
            right
                .points
                .cmp(&left.points)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.agent.cmp(&right.agent))
        });
        let mut last_points = None;
        let mut last_rank = 0u32;
        for (index, row) in rows.iter_mut().enumerate() {
            if last_points == Some(row.points) {
                row.rank = last_rank;
            } else {
                last_rank = index as u32 + 1;
                last_points = Some(row.points);
                row.rank = last_rank;
            }
        }
        rows
    }

    let all = ranked(crate::agents::registered_rows(
        state,
        now,
        fe2vi_core::BoardWindow::All,
        None,
    ));
    let season = ranked(crate::agents::registered_rows(
        state,
        now,
        fe2vi_core::BoardWindow::Season,
        None,
    ));
    let rooms: Vec<serde_json::Value> = [
        PointRoom::Vault,
        PointRoom::Cup,
        PointRoom::Village,
        PointRoom::War,
    ]
    .into_iter()
    .map(|room| {
        let leader = ranked(crate::agents::registered_rows(
            state,
            now,
            fe2vi_core::BoardWindow::All,
            Some(room),
        ))
        .into_iter()
        .next();
        serde_json::json!({
            "room": room.as_str(),
            "leader": leader,
        })
    })
    .collect();
    json(
        200,
        &serde_json::json!({
            "window": "all",
            "now": now,
            "totalAgents": all.len(),
            "champions": all.into_iter().take(10).collect::<Vec<_>>(),
            "rooms": rooms,
            "season": season.into_iter().take(10).collect::<Vec<_>>(),
        }),
        false,
        None,
    )
}

pub fn point_rules() -> Outcome {
    let war: Vec<serde_json::Value> = fe2vi_core::challenges()
        .iter()
        .map(|challenge| serde_json::json!({ "id": challenge.id, "points": challenge.points }))
        .collect();
    json(
        200,
        &serde_json::json!({
            "source": "config",
            "vault": { "scout": 0, "cover": 1, "crack": 2, "steal": 3, "sabotage_hit": 1, "objective": 5, "score": "points - heat + objective" },
            "cup": { "win": 3, "draw": 1, "loss": 0, "score": "returnBps - 0.5 * max(0, drawdownBps - 200)", "eligibility": ["linked", "flat at start", "$100 equity", "1 fill", "no transfers"] },
            "village": { "score": "returnBps - 0.5 * max(0, drawdownBps - 100)", "points": false },
            "war": war,
            "note": "Cup awards 3/1/0 to eligible positive risk-adjusted returns; tied leaders draw. Village has no season points. War flags use the challenge totals."
        }),
        false,
        None,
    )
}

pub fn heist_board(state: &ArenaState, window_raw: Option<&str>, now: u64) -> Outcome {
    let Some(window) = fe2vi_core::HeistWindow::parse(window_raw.unwrap_or("today")) else {
        return http(
            400,
            "application/json",
            r#"{"error":"window must be today, week, or season"}"#.into(),
            false,
            None,
        );
    };
    let board_window = match window {
        fe2vi_core::HeistWindow::Today => fe2vi_core::BoardWindow::Today,
        fe2vi_core::HeistWindow::Week => fe2vi_core::BoardWindow::Week,
        fe2vi_core::HeistWindow::Season => fe2vi_core::BoardWindow::Season,
    };
    let rows: Vec<serde_json::Value> =
        crate::agents::registered_rows(state, now, board_window, Some(PointRoom::Vault))
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "agent": row.agent,
                    "name": row.name,
                    "points": row.points,
                })
            })
            .collect();
    json(
        200,
        &serde_json::json!({
            "window": window_raw.unwrap_or("today"),
            "source": "arena",
            "now": now,
            "rows": rows,
        }),
        false,
        None,
    )
}

pub fn venues(now: u64) -> Outcome {
    let notes: Vec<serde_json::Value> = fe2vi_core::venue_notes()
        .iter()
        .map(|note| {
            serde_json::json!({
                "id": note.id,
                "offers": note.offers,
                "wired": note.wired,
            })
        })
        .collect();
    json(
        200,
        &serde_json::json!({
            "network": "unsupported",
            "source": "unsupported",
            "now": now,
            "quotes": [],
            "notes": notes,
            "note": "Cup and Village have no faucet-funded quote feed. An empty list is empty.",
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
                "points": challenge.points,
                "season": fe2vi_core::SEASON_ID,
                "how": match challenge.id {
                    "sandbox-page" => "Read GET /v1/war/challenges/sandbox-page and call the flag printer it names.",
                    "referee" => "Read GET /v1/war/challenges/referee and call the method that claims without a flag.",
                    "hash-puzzle" => "Read GET /v1/war/challenges/hash-puzzle and find the flag whose SHA-256 is the target.",
                    _ => "Solve the challenge, then submit the flag.",
                },
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
    let rows = crate::agents::registered_rows(state, now, fe2vi_core::BoardWindow::All, None);
    let roster: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "account_id": row.agent,
                "name": row.name,
                "registered": true,
                "points": row.points,
            })
        })
        .collect();
    let live_games = state
        .book
        .matches
        .iter()
        .filter(|play| play.status == "live")
        .count();
    json(
        200,
        &serde_json::json!({
            "network": "arena",
            "source": "arena",
            "now": now,
            "agents": roster.len(),
            "roster": roster,
            "live_games": live_games,
            "heist_live": state.book.matches.iter().any(|play| play.status == "live" && play.arena_id == "vault"),
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
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    signed_data: Option<serde_json::Value>,
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
    if let Some(host) = body.host.as_deref() {
        if let Err(message) = fe2vi_intents::intents_host_allowed(host) {
            return http(403, "application/json", error_body(message), false, None);
        }
    }
    if let Some(grant) = state
        .grants
        .iter()
        .find(|grant| grant.grant_id == body.grant_id)
    {
        let agent = grant.agent.clone();
        if !crate::passport::active_for_agent(state, &agent) {
            return http(
                403,
                "application/json",
                serde_json::json!({
                    "error": "passport_required",
                    "message": "Buy a listed passport and bind it before playing.",
                })
                .to_string(),
                false,
                None,
            );
        }
    }
    let mode = if body.signed_data.is_some() {
        "testnet-signed"
    } else {
        "policy-only"
    };
    let now = query_now(incoming.path);
    match fe2vi_intents::policy_act(
        &mut state.grants,
        &body.grant_id,
        amount,
        &state.caps,
        now,
        mode,
    ) {
        Ok(mut value) => {
            if let Some(object) = value.as_object_mut() {
                object.insert("broadcast".into(), serde_json::json!(false));
                if let Some(host) = body.host {
                    object.insert("host".into(), serde_json::json!(host));
                }
            }
            json(200, &value, true, None)
        }
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
                policy_check: None,
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
    fn city_lists_five_districts_without_a_secret() {
        let mut state = ArenaState::default();
        let json = body_of(call(&mut state, "GET", "/v1/city", ""));
        assert_eq!(json["districts"].as_array().unwrap().len(), 5);
        assert_eq!(json["site"], "fe2vi.space");
        assert!(!json.to_string().contains("naa_"));
    }

    #[test]
    fn village_live_board_is_empty_and_follow_rejects_self() {
        let mut state = ArenaState::default();
        let board = body_of(call(
            &mut state,
            "GET",
            "/v1/village/leaderboard?window=24h",
            "",
        ));
        assert_eq!(board["rows"].as_array().unwrap().len(), 0);
        assert_eq!(board["source"], "stale");
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
        state.book.passports.push(crate::agents::Passport {
            passport_id: "ppt_test".into(),
            token_id: Some("gen_001".into()),
            agent_id: "mason.village.fe2vi.testnet".into(),
            owner_near: String::new(),
            creator_near: String::new(),
            status: "active".into(),
            created_unix: 1,
            updated_unix: 1,
            last_sync_seq: 0,
            career: None,
        });
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
        assert_eq!(acted["mode"], "policy-only");
        assert_eq!(acted["broadcast"], false);
        assert!(state.events.iter().all(|event| !event.verified));
        let mainnet = call(
            &mut state,
            "POST",
            "/v1/intents/act?now=1000",
            &format!(
                r#"{{"grant_id":"{grant_id}","amount_yocto":"1","host":"https://api.agentsonintents.com"}}"#
            ),
        );
        match mainnet {
            Outcome::Http { status, .. } => assert_eq!(status, 403),
            Outcome::Websocket { .. } => panic!("expected http"),
        }
        let signed = body_of(call(
            &mut state,
            "POST",
            "/v1/intents/act?now=1000",
            &format!(
                r#"{{"grant_id":"{grant_id}","amount_yocto":"1","host":"https://www.usetidepool.tech","signed_data":{{"standard":"nep413"}}}}"#
            ),
        ));
        assert_eq!(signed["mode"], "testnet-signed");
        assert_eq!(signed["broadcast"], false);
        assert_eq!(signed["host"], "https://www.usetidepool.tech");
    }
}

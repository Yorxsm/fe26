//! One chain job per Vault match step. A replay or a crash does not send a second payment.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use serde::{Deserialize, Serialize};

use crate::agents::PlayMatch;
use crate::logic::ArenaState;

pub const MAX_ATTEMPTS: u32 = 5;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainJob {
    pub id: String,
    pub match_id: String,
    pub kind: String,
    pub token_id: String,
    pub status: String,
    pub tx_hash: String,
    pub attempts: u32,
    pub args: serde_json::Value,
}

pub fn sync_vault_jobs(state: &mut ArenaState) {
    let snaps: Vec<Snap> = state
        .book
        .matches
        .iter()
        .filter(|play| play.arena_id == "vault")
        .map(snap_of)
        .collect();
    for snap in snaps {
        if !snap.ended || !snap.ready {
            continue;
        }
        if !snap.settled {
            enqueue(
                &mut state.chain_jobs,
                &format!("settle:{}", snap.match_id),
                &snap.match_id,
                "settle",
                "",
                serde_json::json!({
                    "game_id": snap.match_id,
                    "outcome": snap.outcome,
                    "winner": if snap.outcome == "win" { serde_json::Value::String(snap.winner.clone()) } else { serde_json::Value::Null },
                }),
            );
            continue;
        }
        for (token, seat) in &snap.tokens {
            enqueue(
                &mut state.chain_jobs,
                &format!("career:{}:{token}", snap.match_id),
                &snap.match_id,
                "career",
                token,
                serde_json::json!({
                    "token_id": token,
                    "outcome": snap.outcome,
                    "winner": snap.winner,
                    "seatAccount": seat,
                }),
            );
            if let Some(hash) = snap.hashes.get(token) {
                enqueue(
                    &mut state.chain_jobs,
                    &format!("playbook:{}:{token}", snap.match_id),
                    &snap.match_id,
                    "playbook",
                    token,
                    serde_json::json!({
                        "token_id": token,
                        "version": hash.0,
                        "hash": hash.1,
                    }),
                );
            }
        }
    }
}

struct Snap {
    match_id: String,
    ended: bool,
    ready: bool,
    settled: bool,
    outcome: String,
    winner: String,
    tokens: Vec<(String, String)>,
    hashes: std::collections::BTreeMap<String, (u64, String)>,
}

fn snap_of(play: &PlayMatch) -> Snap {
    let pot = play.vault.as_ref().and_then(|vault| vault.pot.as_ref());
    let ended = play.status == "over" || play.vault.as_ref().is_some_and(|vault| vault.ended);
    let outcome = pot.map(|pot| pot.outcome.clone()).unwrap_or_default();
    let rule = pot.map(|pot| pot.pay_rule.clone()).unwrap_or_default();
    let ready = matches!(
        (rule.as_str(), outcome.as_str()),
        ("winner-takes-all", "win") | ("equal-split", "draw")
    );
    let settled = pot.is_some_and(|pot| {
        pot.settled && pot.pays && pot.finality == "FINAL" && !pot.tx_hash.trim().is_empty()
    });
    let tokens = pot
        .map(|pot| {
            pot.seat_links
                .iter()
                .filter(|link| {
                    link.source == "seat-at-match-end"
                        && !link.token_id.is_empty()
                        && !link.near_account.trim().is_empty()
                })
                .map(|link| (link.token_id.clone(), link.near_account.trim().to_string()))
                .collect()
        })
        .unwrap_or_default();
    Snap {
        match_id: play.match_id.clone(),
        ended,
        ready,
        settled,
        outcome,
        winner: pot.map(|pot| pot.winner.clone()).unwrap_or_default(),
        tokens,
        hashes: std::collections::BTreeMap::new(),
    }
}

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub fn attach_playbook_hashes(state: &mut ArenaState) {
    let pending: Vec<(String, String, u64, String)> = state
        .chain_jobs
        .iter()
        .filter(|job| job.kind == "career" && job.status != "done")
        .filter_map(|job| {
            let latest = state
                .book
                .strategies
                .iter()
                .filter(|row| row.token_id == job.token_id)
                .max_by_key(|row| row.version)?;
            let hash = crate::strategy::strategy_hashes(&state.book, &job.token_id)
                .into_iter()
                .find(|(version, _)| *version == latest.version)
                .map(|(_, hash)| hash)?;
            Some((
                job.match_id.clone(),
                job.token_id.clone(),
                latest.version,
                hash,
            ))
        })
        .collect();
    for (match_id, token_id, version, hash) in pending {
        enqueue(
            &mut state.chain_jobs,
            &format!("playbook:{match_id}:{token_id}"),
            &match_id,
            "playbook",
            &token_id,
            serde_json::json!({
                "token_id": token_id,
                "version": version,
                "hash": hash,
            }),
        );
    }
}

fn enqueue(
    jobs: &mut Vec<ChainJob>,
    id: &str,
    match_id: &str,
    kind: &str,
    token_id: &str,
    args: serde_json::Value,
) {
    if jobs.iter().any(|job| job.id == id) {
        return;
    }
    jobs.push(ChainJob {
        id: id.to_string(),
        match_id: match_id.to_string(),
        kind: kind.to_string(),
        token_id: token_id.to_string(),
        status: "pending".into(),
        tx_hash: String::new(),
        attempts: 0,
        args,
    });
}

/// A broadcast that stored no hash did not land. Put it back. A hash stays put
/// so a crash cannot pay the same match twice.
pub fn recover_crashed(jobs: &mut [ChainJob]) {
    for job in jobs {
        if job.status == "broadcasting" && job.tx_hash.trim().is_empty() {
            job.status = "pending".into();
        }
    }
}

pub fn active_broadcast(jobs: &[ChainJob]) -> Option<usize> {
    jobs.iter()
        .position(|job| job.status == "broadcasting" && !job.tx_hash.trim().is_empty())
}

/// One in-flight job. Done and failed rows are never claimed again.
#[cfg_attr(not(test), allow(dead_code))]
pub fn claim_next(jobs: &mut [ChainJob], signer_ready: bool) -> Option<usize> {
    claim_ready(jobs, signer_ready, signer_ready)
}

pub fn claim_ready(
    jobs: &mut [ChainJob],
    settler_ready: bool,
    recorder_ready: bool,
) -> Option<usize> {
    if active_broadcast(jobs).is_some() {
        return None;
    }
    let index = jobs.iter().position(|job| {
        if job.status != "pending" || job.attempts >= MAX_ATTEMPTS || !job.tx_hash.trim().is_empty()
        {
            return false;
        }
        match job.kind.as_str() {
            "settle" => settler_ready,
            "career" | "playbook" => recorder_ready,
            _ => false,
        }
    })?;
    jobs[index].status = "broadcasting".into();
    Some(index)
}

pub fn mark_broadcast(job: &mut ChainJob, tx_hash: &str) {
    job.tx_hash = tx_hash.trim().to_string();
    job.status = "broadcasting".into();
}

pub fn mark_done(job: &mut ChainJob) {
    job.status = "done".into();
}

/// A verified receipt already paid this match. Do not send `settle_vault` again.
pub fn finish_settle_job(jobs: &mut [ChainJob], match_id: &str, tx_hash: &str) {
    let Some(job) = jobs.iter_mut().find(|job| {
        job.kind == "settle"
            && job.match_id == match_id
            && job.status != "done"
            && job.status != "failed"
    }) else {
        return;
    };
    if job.tx_hash.trim().is_empty() {
        job.tx_hash = tx_hash.trim().to_string();
    }
    if job.tx_hash == tx_hash.trim() {
        job.status = "done".into();
    }
}

pub fn mark_retry(job: &mut ChainJob) {
    job.attempts = job.attempts.saturating_add(1);
    job.tx_hash.clear();
    job.status = if job.attempts >= MAX_ATTEMPTS {
        "failed".into()
    } else {
        "pending".into()
    };
}

/// The chain call landed and was rejected. Keep the hash and do not send another.
pub fn park_rejected(job: &mut ChainJob) {
    job.status = "failed".into();
}

/// Drop a claim that has not been signed yet. The frozen arguments stay.
pub fn release_pending(job: &mut ChainJob) {
    if job.tx_hash.trim().is_empty() {
        job.status = "pending".into();
    }
}

/// One monotonic `sync_career` body. `seq` is the on-chain `last_sync_seq` plus one.
/// Win, draw, and loss use the same 3/1/0 points as the arena. Other counters are copied.
pub fn career_sync_args(
    view: &serde_json::Value,
    token_id: &str,
    outcome: &str,
    winner_account: &str,
    seat_account: &str,
) -> Option<serde_json::Value> {
    let seat = seat_account.trim();
    if token_id.is_empty() || seat.is_empty() {
        return None;
    }
    let career = view.get("career")?;
    let seq = view
        .get("last_sync_seq")
        .and_then(serde_json::Value::as_u64)?
        + 1;
    let (wins_add, losses_add, draws_add, xp_add) = match outcome {
        "win" if seat == winner_account.trim() => (1_u64, 0, 0, 3),
        "win" => (0, 1, 0, 0),
        "draw" => (0, 0, 1, 1),
        _ => return None,
    };
    Some(serde_json::json!({
        "token_id": token_id,
        "update": {
            "seq": seq,
            "xp": field_u64(career, "xp")?.saturating_add(xp_add),
            "wins": field_u64(career, "wins")?.saturating_add(wins_add),
            "losses": field_u64(career, "losses")?.saturating_add(losses_add),
            "draws": field_u64(career, "draws")?.saturating_add(draws_add),
            "matches": field_u64(career, "matches")?.saturating_add(1),
            "heists": field_u64(career, "heists")?,
            "seasons": field_u64(career, "seasons")?,
            "season_titles": field_u64(career, "season_titles")?,
            "best_rank": field_u64(career, "best_rank")?,
            "rank": field_u64(career, "rank")?,
            "rank_epoch": field_u64(career, "rank_epoch")?,
        }
    }))
}

pub fn career_already_applied(view: &serde_json::Value, seq: u64) -> bool {
    view.get("last_sync_seq")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|current| current >= seq)
}

/// Store the update once. A later retry sends these same numbers.
pub fn freeze_career_update(job: &mut ChainJob, view: &serde_json::Value) -> bool {
    if job
        .args
        .get("update")
        .and_then(serde_json::Value::as_object)
        .is_some()
    {
        return true;
    }
    let body = career_sync_args(
        view,
        &job.token_id,
        job.args
            .get("outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        job.args
            .get("winner")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        job.args
            .get("seatAccount")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
    );
    let Some(body) = body else {
        return false;
    };
    if let Some(update) = body.get("update") {
        job.args["update"] = update.clone();
        return true;
    }
    false
}

pub fn career_call_body(job: &ChainJob) -> Option<serde_json::Value> {
    let update = job.args.get("update")?.as_object()?;
    Some(serde_json::json!({
        "token_id": job.token_id,
        "update": update,
    }))
}

fn field_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::PlayMatch;
    use crate::logic::ArenaState;

    fn play(settled: bool) -> PlayMatch {
        serde_json::from_value(serde_json::json!({
            "match_id": "mt_1",
            "arena_id": "vault",
            "status": "over",
            "turn": 1,
            "deadline_unix": 0,
            "seats": ["agt"],
            "actions": [],
            "vault": {
                "round": 1,
                "locks": [],
                "loot": 0,
                "alarm": 0,
                "seats": [],
                "pending": [],
                "results": [],
                "ended": true,
                "pot": {
                    "idempotencyKey": "pot:mt_1",
                    "outcome": "win",
                    "payRule": "winner-takes-all",
                    "rule": "winner-takes-all",
                    "winner": "fe2virecdb1e91.testnet",
                    "settlementMethod": "settle_vault",
                    "settled": settled,
                    "pays": settled,
                    "finality": if settled { "FINAL" } else { "" },
                    "txHash": if settled { "tx" } else { "" },
                    "seatLinks": [{
                        "agentId": "agt",
                        "nearAccount": "fe2virecdb1e91.testnet",
                        "tokenId": "rec_001",
                        "source": "seat-at-match-end"
                    }]
                }
            }
        }))
        .unwrap()
    }

    fn state(settled: bool) -> ArenaState {
        let mut state = ArenaState::default();
        state.book.matches.push(play(settled));
        state
    }

    #[test]
    fn a_finished_match_gets_one_settle_job_and_a_replay_does_not_add_another() {
        let mut state = state(false);
        sync_vault_jobs(&mut state);
        sync_vault_jobs(&mut state);
        assert_eq!(state.chain_jobs.len(), 1);
        assert_eq!(state.chain_jobs[0].kind, "settle");
        assert_eq!(state.chain_jobs[0].args["outcome"], "win");
    }

    #[test]
    fn a_verified_receipt_queues_career_once_and_claim_locks_one_job() {
        let mut state = state(true);
        sync_vault_jobs(&mut state);
        sync_vault_jobs(&mut state);
        assert!(state.chain_jobs.iter().any(|job| job.kind == "career"));
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "career")
                .count(),
            1
        );
        assert!(claim_next(&mut state.chain_jobs, false).is_none());
        let first = claim_next(&mut state.chain_jobs, true).unwrap();
        assert!(claim_next(&mut state.chain_jobs, true).is_none());
        mark_broadcast(&mut state.chain_jobs[first], "hash_1");
        assert!(claim_next(&mut state.chain_jobs, true).is_none());
        mark_done(&mut state.chain_jobs[first]);
        assert!(claim_next(&mut state.chain_jobs, true).is_none());
    }

    #[test]
    fn a_crash_before_a_hash_retries_and_a_hash_is_not_sent_again() {
        let mut jobs = vec![ChainJob {
            id: "settle:mt_1".into(),
            match_id: "mt_1".into(),
            kind: "settle".into(),
            token_id: String::new(),
            status: "broadcasting".into(),
            tx_hash: String::new(),
            attempts: 0,
            args: serde_json::json!({}),
        }];
        recover_crashed(&mut jobs);
        assert_eq!(jobs[0].status, "pending");
        jobs[0].status = "broadcasting".into();
        jobs[0].tx_hash = "already".into();
        recover_crashed(&mut jobs);
        assert!(claim_next(&mut jobs, true).is_none());
        assert_eq!(jobs[0].tx_hash, "already");
        park_rejected(&mut jobs[0]);
        assert!(claim_next(&mut jobs, true).is_none());
    }

    #[test]
    fn a_failed_attempt_retries_until_the_cap() {
        let mut job = ChainJob {
            id: "career:mt_1:rec_001".into(),
            match_id: "mt_1".into(),
            kind: "career".into(),
            token_id: "rec_001".into(),
            status: "broadcasting".into(),
            tx_hash: String::new(),
            attempts: 4,
            args: serde_json::json!({}),
        };
        mark_retry(&mut job);
        assert_eq!(job.status, "failed");
        assert!(job.tx_hash.is_empty());
    }

    fn rookie_view() -> serde_json::Value {
        serde_json::json!({
            "last_sync_seq": 4,
            "career": {
                "xp": 30, "wins": 1, "losses": 0, "draws": 0, "matches": 2,
                "heists": 0, "seasons": 0, "season_titles": 0,
                "best_rank": 0, "rank": 0, "rank_epoch": 0
            }
        })
    }

    #[test]
    fn career_sync_is_one_step_and_a_landed_seq_is_not_applied_again() {
        let view = rookie_view();
        let win = career_sync_args(
            &view,
            "rec_001",
            "win",
            "fe2virecdb1e91.testnet",
            "fe2virecdb1e91.testnet",
        )
        .unwrap();
        assert_eq!(win["update"]["seq"], 5);
        assert_eq!(win["update"]["xp"], 33);
        assert_eq!(win["update"]["wins"], 2);
        assert_eq!(win["update"]["matches"], 3);
        assert_eq!(win["update"]["heists"], 0);
        let loss = career_sync_args(
            &view,
            "rec_002",
            "win",
            "fe2virecdb1e91.testnet",
            "other.testnet",
        )
        .unwrap();
        assert_eq!(loss["update"]["losses"], 1);
        assert_eq!(loss["update"]["xp"], 30);
        let draw =
            career_sync_args(&view, "rec_001", "draw", "", "fe2virecdb1e91.testnet").unwrap();
        assert_eq!(draw["update"]["draws"], 1);
        assert_eq!(draw["update"]["xp"], 31);
        let mut job = ChainJob {
            id: "career:mt_1:rec_001".into(),
            match_id: "mt_1".into(),
            kind: "career".into(),
            token_id: "rec_001".into(),
            status: "broadcasting".into(),
            tx_hash: String::new(),
            attempts: 0,
            args: serde_json::json!({
                "token_id": "rec_001",
                "outcome": "win",
                "winner": "fe2virecdb1e91.testnet",
                "seatAccount": "fe2virecdb1e91.testnet"
            }),
        };
        assert!(freeze_career_update(&mut job, &view));
        let seq = job.args["update"]["seq"].clone();
        assert!(freeze_career_update(&mut job, &rookie_view()));
        assert_eq!(job.args["update"]["seq"], seq);
        recover_crashed(std::slice::from_mut(&mut job));
        assert_eq!(job.status, "pending");
        assert_eq!(job.args["update"]["seq"], seq);
        let mut landed = view.clone();
        landed["last_sync_seq"] = seq.clone();
        assert!(career_already_applied(&landed, seq.as_u64().unwrap()));
        assert_eq!(career_call_body(&job).unwrap()["update"]["seq"], seq);
    }

    #[test]
    fn an_auto_trigger_queues_settle_then_career_after_the_receipt() {
        let mut state = state(false);
        sync_vault_jobs(&mut state);
        assert_eq!(state.chain_jobs.len(), 1);
        finish_settle_job(&mut state.chain_jobs, "mt_1", "tx");
        assert_eq!(state.chain_jobs[0].status, "done");
        finish_settle_job(&mut state.chain_jobs, "mt_1", "other");
        assert_eq!(state.chain_jobs[0].tx_hash, "tx");
        state.book.matches[0]
            .vault
            .as_mut()
            .unwrap()
            .pot
            .as_mut()
            .unwrap()
            .settled = true;
        let pot = state.book.matches[0]
            .vault
            .as_mut()
            .unwrap()
            .pot
            .as_mut()
            .unwrap();
        pot.pays = true;
        pot.finality = "FINAL".into();
        pot.tx_hash = "tx".into();
        sync_vault_jobs(&mut state);
        let career = state
            .chain_jobs
            .iter()
            .find(|job| job.kind == "career")
            .unwrap();
        assert_eq!(career.args["seatAccount"], "fe2virecdb1e91.testnet");
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "settle")
                .count(),
            1
        );
    }
}

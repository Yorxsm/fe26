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

pub fn queue_recorder_bind(state: &mut ArenaState, contract: &str, token_id: &str, agent_id: &str) {
    let contract = contract.trim();
    let token_id = token_id.trim();
    let agent_id = agent_id.trim();
    if contract.is_empty()
        || token_id.is_empty()
        || agent_id.is_empty()
        || contract.contains("fe2vidisp")
        || contract.ends_with(".fe2vi.testnet")
    {
        return;
    }
    enqueue(
        &mut state.chain_jobs,
        &format!("bind:{token_id}"),
        "",
        "bind",
        token_id,
        serde_json::json!({
            "token_id": token_id,
            "agent_id": agent_id,
            "contract": contract,
        }),
    );
}

fn correct_open_results(state: &mut ArenaState) {
    let near: std::collections::HashMap<String, String> = state
        .book
        .players
        .iter()
        .filter_map(|player| {
            let account = player.wallets.near_account.as_ref()?.trim();
            if account.is_empty() {
                None
            } else {
                Some((player.agent_id.clone(), account.to_string()))
            }
        })
        .collect();
    type SeatScore = (String, i64, Option<String>);
    let plans: Vec<(usize, Vec<SeatScore>)> = state
        .book
        .matches
        .iter()
        .enumerate()
        .filter_map(|(index, play)| {
            if play.arena_id != "vault" {
                return None;
            }
            let vault = play.vault.as_ref()?;
            if !vault.ended {
                return None;
            }
            let seats = play
                .seats
                .iter()
                .map(|seat| {
                    (
                        seat.clone(),
                        i64::from(vault.score(seat)),
                        near.get(seat).cloned(),
                    )
                })
                .collect();
            Some((index, seats))
        })
        .collect();
    for (index, seats) in plans {
        let Some(vault) = state.book.matches[index].vault.as_mut() else {
            continue;
        };
        vault.correct_unsettled_result(&seats);
        let Some(pot) = vault.pot.as_mut() else {
            continue;
        };
        if pot.settled || pot.pays || pot.finality == "FINAL" {
            continue;
        }
        let best = seats.iter().map(|(_, score, _)| *score).max().unwrap_or(0);
        let leaders = seats.iter().filter(|(_, score, _)| *score == best).count();
        let tied = leaders != 1;
        for link in pot.seat_links.iter_mut() {
            let Some((_, score, _)) = seats.iter().find(|(agent, _, _)| agent == &link.agent_id)
            else {
                continue;
            };
            link.result = crate::vault::seat_result_label(*score, best, tied).to_string();
        }
    }
}

/// Queue `bind` for an active book passport whose match-end link is still empty.
/// The book is not the snapshot. Career credit still waits for `agent_token`.
pub fn queue_unbound_book_binds(state: &mut ArenaState, contract: &str) {
    let tokens: std::collections::HashMap<String, String> = state
        .book
        .passports
        .iter()
        .filter(|passport| passport.status == "active")
        .filter_map(|passport| {
            let token = passport.token_id.as_deref()?.trim();
            if token.is_empty() {
                None
            } else {
                Some((passport.agent_id.clone(), token.to_string()))
            }
        })
        .collect();
    let agents: Vec<String> = state
        .book
        .matches
        .iter()
        .filter(|play| play.arena_id == "vault")
        .filter_map(|play| play.vault.as_ref()?.pot.as_ref())
        .filter(|pot| !pot.settled && !pot.pays)
        .flat_map(|pot| pot.seat_links.iter())
        .filter(|link| link.source != "seat-at-match-end" || link.token_id.is_empty())
        .map(|link| link.agent_id.clone())
        .collect();
    for agent in agents {
        if let Some(token) = tokens.get(&agent) {
            queue_recorder_bind(state, contract, token, &agent);
        }
    }
}

/// A settle row with no transaction hash has not paid. A failed, exhausted, or
/// legacy row is claimable again so the cron can read `get_heist`. A FINAL
/// receipt and any stored hash stay where they are.
pub fn reopen_unpaid_settle_jobs(state: &mut ArenaState) {
    let pots: std::collections::HashMap<String, (bool, bool)> = state
        .book
        .matches
        .iter()
        .filter_map(|play| {
            let pot = play.vault.as_ref()?.pot.as_ref()?;
            let paid = pot.settled
                || pot.pays
                || pot.finality == "FINAL"
                || !pot.tx_hash.trim().is_empty();
            Some((play.match_id.clone(), (paid, pot.no_pot)))
        })
        .collect();
    for job in &mut state.chain_jobs {
        if job.kind != "settle" || !job.tx_hash.trim().is_empty() {
            continue;
        }
        let Some((paid, no_pot)) = pots.get(&job.match_id) else {
            continue;
        };
        if *paid || (*no_pot && job.status == "done") {
            continue;
        }
        if job.status == "pending" && job.attempts < MAX_ATTEMPTS {
            continue;
        }
        job.status = "pending".into();
        job.attempts = 0;
        job.tx_hash.clear();
    }
}

/// `get_heist` returned null. Nothing was paid. Careers queue once the seat
/// tokens are present. A FINAL receipt is not rewritten.
pub fn record_missing_pot(state: &mut ArenaState, match_id: &str) {
    let mut paid = false;
    if let Some(pot) = state
        .book
        .matches
        .iter_mut()
        .find(|play| play.match_id == match_id)
        .and_then(|play| play.vault.as_mut())
        .and_then(|vault| vault.pot.as_mut())
    {
        paid = pot.settled || pot.pays || pot.finality == "FINAL" || !pot.tx_hash.trim().is_empty();
        if !paid {
            pot.no_pot = true;
            pot.reason = "No on-chain pot was opened for this match, so settle_vault was not sent and nothing was paid.".to_string();
        }
    }
    if let Some(job) = state.chain_jobs.iter_mut().find(|job| {
        job.kind == "settle" && job.match_id == match_id && job.tx_hash.trim().is_empty()
    }) {
        mark_done(job);
    }
    if !paid {
        sync_vault_jobs(state);
    }
}

pub fn sync_vault_jobs(state: &mut ArenaState) {
    reopen_unpaid_settle_jobs(state);
    correct_open_results(state);
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
        if !snap.settled && !snap.no_pot {
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
        for (token, seat, result) in &snap.tokens {
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
                    "seatResult": result,
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
    no_pot: bool,
    outcome: String,
    winner: String,
    tokens: Vec<(String, String, String)>,
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
    let no_pot = pot.is_some_and(|pot| pot.no_pot);
    let tokens = pot
        .map(|pot| {
            pot.seat_links
                .iter()
                .filter(|link| {
                    link.source == "seat-at-match-end"
                        && !link.token_id.is_empty()
                        && !link.near_account.trim().is_empty()
                })
                .map(|link| {
                    (
                        link.token_id.clone(),
                        link.near_account.trim().to_string(),
                        link.result.clone(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    Snap {
        match_id: play.match_id.clone(),
        ended,
        ready,
        settled,
        no_pot,
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
            "career" | "playbook" | "bind" => recorder_ready,
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
    seat_result: &str,
) -> Option<serde_json::Value> {
    let seat = seat_account.trim();
    let seat_result = seat_result.trim();
    if token_id.is_empty() || seat.is_empty() {
        return None;
    }
    let career = view.get("career")?;
    let seq = view
        .get("last_sync_seq")
        .and_then(serde_json::Value::as_u64)?
        + 1;
    let (wins_add, losses_add, draws_add, xp_add) = match seat_result {
        "win" => (1_u64, 0, 0, 3),
        "loss" => (0, 1, 0, 0),
        "draw" => (0, 0, 1, 1),
        _ => match outcome {
            "win" if seat == winner_account.trim() => (1_u64, 0, 0, 3),
            "win" => (0, 1, 0, 0),
            "draw" => (0, 0, 1, 1),
            _ => return None,
        },
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
        job.args
            .get("seatResult")
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
    use crate::agents::{Passport, PlayMatch, Player, Wallets};
    use crate::logic::ArenaState;
    use crate::vault::{SeatTokenLink, VaultMatch};

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
            "",
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
            "",
        )
        .unwrap();
        assert_eq!(loss["update"]["losses"], 1);
        assert_eq!(loss["update"]["xp"], 30);
        let draw =
            career_sync_args(&view, "rec_001", "draw", "", "fe2virecdb1e91.testnet", "").unwrap();
        let shared = career_sync_args(
            &view,
            "gen_008",
            "win",
            "fe2virecdb1e91.testnet",
            "fe2virecdb1e91.testnet",
            "loss",
        )
        .unwrap();
        assert_eq!(shared["update"]["losses"], 1);
        assert_eq!(shared["update"]["wins"], 1);
        assert_eq!(shared["update"]["xp"], 30);
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

    #[test]
    fn a_shared_wallet_draw_is_repaired_and_a_missing_pot_still_syncs_both_careers() {
        let account = "fe2virecdb1e91.testnet";
        let mut draw = VaultMatch::start(&["juno".into(), "rook".into()]);
        draw.ended = true;
        draw.seal_vault_result(
            "mt_797",
            &[
                ("juno".into(), 0, Some(account.into())),
                ("rook".into(), 0, Some(account.into())),
            ],
        );
        {
            let pot = draw.pot.as_mut().unwrap();
            pot.outcome = "win".into();
            pot.winner = account.into();
            pot.seat_links = vec![
                link("juno", "", "unbound-at-match-end"),
                link("rook", "", "unbound-at-match-end"),
            ];
        }
        let mut win = VaultMatch::start(&["juno".into(), "rook".into()]);
        win.ended = true;
        for seat in &mut win.seats {
            seat.points = if seat.agent_id == "juno" { 5 } else { 3 };
        }
        win.seal_vault_result(
            "mt_2b2d",
            &[
                ("juno".into(), 5, Some(account.into())),
                ("rook".into(), 3, Some(account.into())),
            ],
        );
        {
            let pot = win.pot.as_mut().unwrap();
            pot.no_pot = true;
            pot.seat_links = vec![
                link("juno", "gen_007", "seat-at-match-end"),
                link("rook", "gen_008", "seat-at-match-end"),
            ];
        }
        let mut state = ArenaState::default();
        state.book.players = vec![player("juno", account), player("rook", account)];
        state.book.passports = vec![passport("gen_007", "juno"), passport("gen_008", "rook")];
        state.book.matches = vec![finished("mt_797", draw), finished("mt_2b2d", win)];
        queue_unbound_book_binds(&mut state, "passport.fe2vidisp7a3c.testnet");
        assert!(state.chain_jobs.is_empty());
        queue_unbound_book_binds(&mut state, "passport.fe2vi.testnet");
        assert!(state.chain_jobs.is_empty());
        queue_unbound_book_binds(&mut state, "passport.fe2virecdb1e91.testnet");
        sync_vault_jobs(&mut state);
        let draw_pot = state.book.matches[0]
            .vault
            .as_ref()
            .unwrap()
            .pot
            .as_ref()
            .unwrap();
        assert_eq!(draw_pot.outcome, "draw");
        assert!(draw_pot.winner.is_empty());
        assert_eq!(draw_pot.rule, "equal-split");
        assert!(draw_pot.seat_links.iter().all(|row| row.result == "draw"));
        let settle = state
            .chain_jobs
            .iter()
            .find(|job| job.kind == "settle")
            .unwrap();
        assert_eq!(settle.args["outcome"], "draw");
        assert!(settle.args["winner"].is_null());
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "bind")
                .count(),
            2
        );
        let win_jobs: Vec<_> = state
            .chain_jobs
            .iter()
            .filter(|job| job.kind == "career" && job.match_id == "mt_2b2d")
            .collect();
        assert_eq!(win_jobs.len(), 2);
        let juno = win_jobs
            .iter()
            .find(|job| job.token_id == "gen_007")
            .unwrap();
        let rook = win_jobs
            .iter()
            .find(|job| job.token_id == "gen_008")
            .unwrap();
        assert_eq!(juno.args["seatResult"], "win");
        assert_eq!(rook.args["seatResult"], "loss");
        state.book.matches[0]
            .vault
            .as_mut()
            .unwrap()
            .pot
            .as_mut()
            .unwrap()
            .no_pot = true;
        for row in state.book.matches[0]
            .vault
            .as_mut()
            .unwrap()
            .pot
            .as_mut()
            .unwrap()
            .seat_links
            .iter_mut()
        {
            row.token_id = if row.agent_id == "juno" {
                "gen_007"
            } else {
                "gen_008"
            }
            .into();
            row.source = "seat-at-match-end".into();
        }
        sync_vault_jobs(&mut state);
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "settle")
                .count(),
            1
        );
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "career" && job.match_id == "mt_797")
                .count(),
            2
        );
        assert!(state
            .chain_jobs
            .iter()
            .filter(|job| job.kind == "career" && job.match_id == "mt_797")
            .all(|job| job.args["seatResult"] == "draw"));
    }

    #[test]
    fn a_failed_settle_without_a_hash_is_rechecked_and_does_not_pay_twice() {
        let account = "fe2virecdb1e91.testnet";
        let mut win = VaultMatch::start(&["juno".into(), "rook".into()]);
        win.ended = true;
        for seat in &mut win.seats {
            seat.points = if seat.agent_id == "juno" { 5 } else { 3 };
        }
        win.seal_vault_result(
            "mt_2b2d",
            &[
                ("juno".into(), 5, Some(account.into())),
                ("rook".into(), 3, Some(account.into())),
            ],
        );
        {
            let pot = win.pot.as_mut().unwrap();
            pot.seat_links = vec![
                link("juno", "gen_007", "seat-at-match-end"),
                link("rook", "gen_008", "seat-at-match-end"),
            ];
        }
        let mut state = ArenaState::default();
        state.book.players = vec![player("juno", account), player("rook", account)];
        state.book.matches = vec![
            finished("mt_2b2d", win),
            bare_match("mt_hash"),
            bare_match("mt_final"),
            bare_match("mt_stale"),
            bare_match("mt_live"),
        ];
        {
            let pot = state.book.matches[2]
                .vault
                .as_mut()
                .unwrap()
                .pot
                .as_mut()
                .unwrap();
            pot.settled = true;
            pot.pays = true;
            pot.finality = "FINAL".into();
            pot.tx_hash = "final_receipt".into();
        }
        state.chain_jobs = vec![
            settle_job("mt_2b2d", "failed", 5, ""),
            settle_job("mt_hash", "failed", 5, "abc"),
            settle_job("mt_final", "failed", 5, ""),
            settle_job("mt_stale", "broadcasting", 5, ""),
            settle_job("mt_live", "pending", 2, ""),
        ];
        state.chain_jobs[0].status = "error".into();
        sync_vault_jobs(&mut state);
        let reopened = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_2b2d")
            .unwrap();
        assert_eq!(reopened.status, "pending");
        assert_eq!(reopened.attempts, 0);
        assert!(reopened.tx_hash.is_empty());
        assert!(state.chain_jobs.iter().all(|job| job.kind != "career"));
        assert!(
            !state.book.matches[0]
                .vault
                .as_ref()
                .unwrap()
                .pot
                .as_ref()
                .unwrap()
                .no_pot
        );
        let hashed = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_hash")
            .unwrap();
        assert_eq!(hashed.status, "failed");
        assert_eq!(hashed.tx_hash, "abc");
        let final_job = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_final")
            .unwrap();
        assert_eq!(final_job.status, "failed");
        assert_eq!(final_job.attempts, 5);
        let stale = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_stale")
            .unwrap();
        assert_eq!(stale.status, "pending");
        assert_eq!(stale.attempts, 0);
        let live = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_live")
            .unwrap();
        assert_eq!(live.status, "pending");
        assert_eq!(live.attempts, 2);
        record_missing_pot(&mut state, "mt_2b2d");
        record_missing_pot(&mut state, "mt_2b2d");
        record_missing_pot(&mut state, "mt_final");
        let pot = state.book.matches[0]
            .vault
            .as_ref()
            .unwrap()
            .pot
            .as_ref()
            .unwrap();
        assert!(pot.no_pot);
        assert!(!pot.settled);
        assert!(!pot.pays);
        assert!(pot.tx_hash.is_empty());
        assert_ne!(pot.finality, "FINAL");
        let final_pot = state.book.matches[2]
            .vault
            .as_ref()
            .unwrap()
            .pot
            .as_ref()
            .unwrap();
        assert!(!final_pot.no_pot);
        assert!(final_pot.settled && final_pot.pays);
        assert_eq!(final_pot.tx_hash, "final_receipt");
        let done = state
            .chain_jobs
            .iter()
            .find(|job| job.match_id == "mt_2b2d")
            .unwrap();
        assert_eq!(done.status, "done");
        assert!(done.tx_hash.is_empty());
        let careers: Vec<_> = state
            .chain_jobs
            .iter()
            .filter(|job| job.kind == "career" && job.match_id == "mt_2b2d")
            .collect();
        assert_eq!(careers.len(), 2);
        assert_eq!(
            careers
                .iter()
                .find(|job| job.token_id == "gen_007")
                .unwrap()
                .args["seatResult"],
            "win"
        );
        assert_eq!(
            careers
                .iter()
                .find(|job| job.token_id == "gen_008")
                .unwrap()
                .args["seatResult"],
            "loss"
        );
        assert_eq!(
            state
                .chain_jobs
                .iter()
                .filter(|job| job.kind == "settle" && job.match_id == "mt_2b2d")
                .count(),
            1
        );
        let view = serde_json::json!({
            "last_sync_seq": 1,
            "career": {"xp": 3, "wins": 1, "losses": 0, "draws": 0, "matches": 1, "heists": 0, "seasons": 0, "season_titles": 0, "best_rank": 0, "rank": 0, "rank_epoch": 0}
        });
        assert!(career_already_applied(&view, 1));
        assert!(
            career_sync_args(&view, "gen_007", "win", account, account, "win",)
                .is_some_and(|body| body["update"]["seq"] == 2 && body["update"]["matches"] == 2)
        );
        let already = serde_json::json!({"last_sync_seq": 2});
        assert!(career_already_applied(
            &already,
            view["last_sync_seq"].as_u64().unwrap() + 1
        ));
    }

    fn settle_job(match_id: &str, status: &str, attempts: u32, tx_hash: &str) -> ChainJob {
        ChainJob {
            id: format!("settle:{match_id}"),
            match_id: match_id.into(),
            kind: "settle".into(),
            token_id: String::new(),
            status: status.into(),
            tx_hash: tx_hash.into(),
            attempts,
            args: serde_json::json!({
                "game_id": match_id,
                "outcome": "win",
                "winner": "fe2virecdb1e91.testnet",
            }),
        }
    }

    fn bare_match(match_id: &str) -> PlayMatch {
        let mut vault = VaultMatch::start(&["juno".into()]);
        vault.ended = true;
        vault.seal_vault_result(
            match_id,
            &[("juno".into(), 1, Some("fe2virecdb1e91.testnet".into()))],
        );
        PlayMatch {
            match_id: match_id.into(),
            arena_id: "vault".into(),
            status: "over".into(),
            turn: 1,
            deadline_unix: 0,
            seats: vec!["juno".into()],
            actions: Vec::new(),
            created_unix: 0,
            vault: Some(vault),
        }
    }

    fn link(agent: &str, token: &str, source: &str) -> SeatTokenLink {
        SeatTokenLink {
            agent_id: agent.into(),
            near_account: "fe2virecdb1e91.testnet".into(),
            token_id: token.into(),
            source: source.into(),
            result: String::new(),
        }
    }

    fn player(agent: &str, account: &str) -> Player {
        Player {
            agent_id: agent.into(),
            handle: agent.into(),
            name: agent.into(),
            description: String::new(),
            owner_contact: None,
            token_hash: String::new(),
            token_prefix: String::new(),
            status: "active".into(),
            created_unix: 0,
            wallets: Wallets {
                near_account: Some(account.into()),
                ..Wallets::default()
            },
            capabilities: Vec::new(),
            skin_id: "ingot".into(),
            war_hits: Vec::new(),
        }
    }

    fn passport(token: &str, agent: &str) -> Passport {
        Passport {
            passport_id: token.into(),
            token_id: Some(token.into()),
            agent_id: agent.into(),
            owner_near: "fe2virecdb1e91.testnet".into(),
            creator_near: "fe2virecdb1e91.testnet".into(),
            status: "active".into(),
            created_unix: 0,
            updated_unix: 0,
            last_sync_seq: 0,
            career: None,
        }
    }

    fn finished(match_id: &str, vault: VaultMatch) -> PlayMatch {
        PlayMatch {
            match_id: match_id.into(),
            arena_id: "vault".into(),
            status: "over".into(),
            turn: 1,
            deadline_unix: 0,
            seats: vec!["juno".into(), "rook".into()],
            actions: Vec::new(),
            created_unix: 0,
            vault: Some(vault),
        }
    }
}

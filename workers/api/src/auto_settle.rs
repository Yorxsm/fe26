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
        for token in &snap.tokens {
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
    tokens: Vec<String>,
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
                .filter(|link| link.source == "seat-at-match-end" && !link.token_id.is_empty())
                .map(|link| link.token_id.clone())
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
}

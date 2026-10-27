#![cfg_attr(all(not(test), not(target_arch = "wasm32")), allow(dead_code))]
//! Host-testable NEAR testnet log reader.
//!
//! The worker fetches final blocks, then transaction status, and only then
//! marks a feed event verified. A public POST never reaches [`apply_chain_events`].

use crate::logic::{ArenaState, FeedEvent};

pub const LOOKBACK: u64 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainEvent {
    pub game_id: String,
    pub kind: String,
    pub summary: String,
    pub fingerprint: String,
}

pub fn refuse_mainnet(url: &str) -> Result<(), &'static str> {
    if url.to_ascii_lowercase().contains("mainnet") {
        Err("mainnet rpc refused")
    } else {
        Ok(())
    }
}

/// Heights still to scan. The window is bounded so a cron tick stays small.
pub fn plan_heights(cursor: u64, head: u64) -> Vec<u64> {
    if head == 0 {
        return Vec::new();
    }
    let start = if cursor == 0 {
        head.saturating_sub(LOOKBACK - 1)
    } else {
        cursor.saturating_add(1)
    };
    if start > head {
        return Vec::new();
    }
    let mut from = start;
    if head - from + 1 > LOOKBACK {
        from = head - LOOKBACK + 1;
    }
    (from..=head).collect()
}

pub fn block_height(block: &serde_json::Value) -> Option<u64> {
    block
        .pointer("/result/header/height")
        .or_else(|| block.pointer("/header/height"))
        .and_then(serde_json::Value::as_u64)
}

pub fn chunk_hashes(block: &serde_json::Value) -> Vec<String> {
    let Some(chunks) = block
        .pointer("/result/chunks")
        .or_else(|| block.pointer("/chunks"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    chunks
        .iter()
        .filter_map(|chunk| {
            chunk
                .get("chunk_hash")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .take(4)
        .collect()
}

pub fn chunk_tx_refs(chunk: &serde_json::Value, watch: &[String]) -> Vec<(String, String)> {
    let txs = chunk
        .pointer("/result/transactions")
        .or_else(|| chunk.pointer("/transactions"))
        .and_then(serde_json::Value::as_array);
    let Some(txs) = txs else {
        return Vec::new();
    };
    txs.iter()
        .filter_map(|tx| {
            let receiver = tx.get("receiver_id")?.as_str()?;
            if !watch.iter().any(|account| account == receiver) {
                return None;
            }
            let hash = tx.get("hash")?.as_str()?.to_string();
            let signer = tx.get("signer_id")?.as_str()?.to_string();
            Some((hash, signer))
        })
        .take(8)
        .collect()
}

pub fn logs_from_tx_status(value: &serde_json::Value) -> Vec<String> {
    let receipts = value
        .pointer("/result/receipts_outcome")
        .or_else(|| value.pointer("/receipts_outcome"))
        .and_then(serde_json::Value::as_array);
    let Some(receipts) = receipts else {
        return Vec::new();
    };
    let mut logs = Vec::new();
    for receipt in receipts {
        if let Some(entries) = receipt
            .pointer("/outcome/logs")
            .and_then(serde_json::Value::as_array)
        {
            for entry in entries {
                if let Some(line) = entry.as_str() {
                    logs.push(line.to_string());
                }
            }
        }
    }
    logs
}

pub fn parse_event_log(log: &str, height: u64, index: usize) -> Option<ChainEvent> {
    let rest = log.trim().strip_prefix("EVENT_JSON:")?;
    let value: serde_json::Value = serde_json::from_str(rest).ok()?;
    let standard = value.get("standard")?.as_str()?;
    if standard != "fe2vi" && standard != "nep245" {
        return None;
    }
    let kind = value.get("event")?.as_str()?.to_string();
    let data = value.get("data")?.as_array()?.first()?.clone();
    let game_id = data
        .get("id")
        .or_else(|| data.get("game_id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("arena")
        .to_string();
    let summary = summary_for(&kind, &data);
    let fingerprint = format!("{height}:{index}:{log}");
    Some(ChainEvent {
        game_id,
        kind,
        summary,
        fingerprint,
    })
}

fn summary_for(kind: &str, data: &serde_json::Value) -> String {
    let agent = data
        .get("agent")
        .or_else(|| data.get("account"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("an account");
    match kind {
        "game_created" => format!("Game listed by {agent}."),
        "game_delisted" => "Moderator delisted the game.".to_string(),
        "agent_registered" => format!("{agent} registered."),
        "action_taken" => format!("{agent} submitted an action."),
        "bid_placed" => format!("{agent} placed a bid."),
        "game_resolved" => "The referee settled the pot.".to_string(),
        "spend_reverted" => "A cross-contract spend was reverted.".to_string(),
        "mt_transfer" | "mt_mint" => "Token movement recorded on the referee.".to_string(),
        other => format!("{other} recorded on chain."),
    }
}

/// Insert chain events as verified. Returns how many new rows were stored.
pub fn apply_chain_events(state: &mut ArenaState, events: &[ChainEvent], height: u64) -> usize {
    let mut added = 0;
    for event in events {
        if state
            .seen_chain
            .iter()
            .any(|seen| seen == &event.fingerprint)
        {
            continue;
        }
        state.seen_chain.push(event.fingerprint.clone());
        state.events.push(FeedEvent {
            id: state.next_event,
            game_id: event.game_id.clone(),
            kind: event.kind.clone(),
            summary: event.summary.clone(),
            verified: true,
            source: "chain".to_string(),
        });
        state.next_event += 1;
        added += 1;
    }
    if state.events.len() > crate::logic::history_limit() {
        let extra = state.events.len() - crate::logic::history_limit();
        state.events.drain(0..extra);
    }
    if state.seen_chain.len() > 500 {
        let extra = state.seen_chain.len() - 500;
        state.seen_chain.drain(0..extra);
    }
    if height > state.indexed_height {
        state.indexed_height = height;
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_rpc_is_refused_and_only_chain_logs_verify() {
        assert!(refuse_mainnet("https://rpc.mainnet.near.org").is_err());
        assert!(refuse_mainnet("https://rpc.testnet.near.org").is_ok());
        assert_eq!(plan_heights(0, 20), (13..=20).collect::<Vec<_>>());
        assert_eq!(plan_heights(20, 20), Vec::<u64>::new());

        let block = serde_json::json!({
            "result": {
                "header": { "height": 20 },
                "chunks": [{ "chunk_hash": "abc" }]
            }
        });
        assert_eq!(block_height(&block), Some(20));
        assert_eq!(chunk_hashes(&block), vec!["abc".to_string()]);

        let chunk = serde_json::json!({
            "result": {
                "transactions": [
                    {"hash": "tx1", "signer_id": "alice.testnet", "receiver_id": "heist.fe2vi.testnet"},
                    {"hash": "tx2", "signer_id": "bob.testnet", "receiver_id": "someone.testnet"}
                ]
            }
        });
        let refs = chunk_tx_refs(&chunk, &["heist.fe2vi.testnet".to_string()]);
        assert_eq!(refs, vec![("tx1".into(), "alice.testnet".into())]);

        let log = r#"EVENT_JSON:{"standard":"fe2vi","version":"1.0.0","event":"action_taken","data":[{"id":"g1","agent":"alice.fe2vi.testnet"}]}"#;
        let status = serde_json::json!({
            "result": { "receipts_outcome": [{ "outcome": { "logs": [log] } }] }
        });
        let logs = logs_from_tx_status(&status);
        let parsed = parse_event_log(&logs[0], 20, 0).unwrap();
        assert_eq!(parsed.game_id, "g1");
        assert_eq!(parsed.kind, "action_taken");

        let mut state = ArenaState::default();
        assert_eq!(
            apply_chain_events(&mut state, std::slice::from_ref(&parsed), 20),
            1
        );
        assert_eq!(apply_chain_events(&mut state, &[parsed], 20), 0);
        assert!(state.events[0].verified);
        assert_eq!(state.events[0].source, "chain");
        assert_eq!(state.indexed_height, 20);

        let spoof = parse_event_log("not a chain log", 21, 0);
        assert!(spoof.is_none());
    }
}

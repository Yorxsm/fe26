//! Market reads stay inside one Worker invocation.
//! Tokens use `nft_tokens` pages. Listings are one prefix read of the market
//! map, not a `get_listing` call per token.
#![cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]

use serde::{Deserialize, Serialize};

pub const PAGE_LIMIT: u64 = 50;
pub const MAX_PAGES: u64 = 3;
pub const CACHE_TTL_SECS: u64 = 45;
pub const RATE_BACKOFF_SECS: u64 = 60;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketSnapshot {
    pub fetched_unix: u64,
    pub backoff_until: u64,
    pub body: Option<serde_json::Value>,
}

pub fn should_refresh(snapshot: Option<&MarketSnapshot>, now: u64) -> bool {
    let Some(snapshot) = snapshot else {
        return true;
    };
    if now < snapshot.backoff_until {
        return false;
    }
    match snapshot.body {
        Some(_) => now.saturating_sub(snapshot.fetched_unix) >= CACHE_TTL_SECS,
        None => true,
    }
}

pub fn apply_rate_limit(snapshot: &mut MarketSnapshot, now: u64) {
    snapshot.backoff_until = now.saturating_add(RATE_BACKOFF_SECS);
}

pub fn is_rate_limited(status: u16, body: &str) -> bool {
    status == 429 || body.contains("429") || body.to_ascii_lowercase().contains("too many requests")
}

/// `None` stops. A full page asks for the next one, and the page cap stops a long contract.
pub fn next_token_page(from: u64, page_len: u64, pages_done: u64) -> Option<u64> {
    if page_len < PAGE_LIMIT || pages_done >= MAX_PAGES {
        None
    } else {
        Some(from.saturating_add(page_len))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaleRow {
    pub token_id: String,
    pub seller: String,
    pub approval_id: u64,
    pub price: u128,
}

/// Decode one `view_state` entry from the market `LookupMap` prefix `l`.
pub fn decode_listing_entry(key: &[u8], value: &[u8]) -> Option<SaleRow> {
    let mut key_rest = key.strip_prefix(b"l")?;
    let token_id = take_string(&mut key_rest)?;
    if token_id.is_empty() || !key_rest.is_empty() {
        return None;
    }
    let mut value_rest = value;
    let seller = take_string(&mut value_rest)?;
    if value_rest.len() < 24 {
        return None;
    }
    let approval_id = u64::from_le_bytes(value_rest[..8].try_into().ok()?);
    let price = u128::from_le_bytes(value_rest[8..24].try_into().ok()?);
    Some(SaleRow {
        token_id,
        seller,
        approval_id,
        price,
    })
}

pub fn listing_json(row: &SaleRow) -> serde_json::Value {
    serde_json::json!({
        "token_id": row.token_id,
        "seller": row.seller,
        "approval_id": row.approval_id,
        "price": row.price.to_string(),
    })
}

fn take_string(input: &mut &[u8]) -> Option<String> {
    if input.len() < 4 {
        return None;
    }
    let len = u32::from_le_bytes(input[..4].try_into().ok()?) as usize;
    *input = &input[4..];
    if input.len() < len || len > 128 {
        return None;
    }
    let text = std::str::from_utf8(&input[..len]).ok()?.to_string();
    *input = &input[len..];
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paging_calls(item_count: u64) -> u64 {
        let mut from = 0_u64;
        let mut calls = 0_u64;
        loop {
            calls += 1;
            let page = item_count.saturating_sub(from).min(PAGE_LIMIT);
            match next_token_page(from, page, calls) {
                Some(next) => from = next,
                None => break,
            }
        }
        calls
    }

    fn market_subrequests(boards: &[(u64, u64)]) -> u64 {
        let mut calls = 1_u64;
        for (tokens, _listings) in boards {
            calls = calls
                .saturating_add(paging_calls(*tokens))
                .saturating_add(1);
        }
        calls
    }

    #[test]
    fn fifty_listings_stay_under_ten_subrequests() {
        let live = market_subrequests(&[(1, 0), (51, 50)]);
        assert!(live < 10, "{live}");
        let crowded = market_subrequests(&[(80, 120)]);
        assert!(crowded < 10, "{crowded}");
        assert_eq!(paging_calls(49), 1);
        assert_eq!(paging_calls(50), 2);
        assert_eq!(paging_calls(51), 2);
        assert_eq!(
            market_subrequests(&[(51, 50)]),
            market_subrequests(&[(51, 500)])
        );
        assert!(market_subrequests(&[(150, 50), (150, 50)]) < 10);
    }

    #[test]
    fn state_entry_decodes_a_one_near_ask() {
        let mut key = vec![b'l'];
        push_str(&mut key, "gen_001");
        let mut value = Vec::new();
        push_str(&mut value, "fe2virecdb1e91.testnet");
        value.extend_from_slice(&1_u64.to_le_bytes());
        value.extend_from_slice(&1_000_000_000_000_000_000_000_000_u128.to_le_bytes());
        let row = decode_listing_entry(&key, &value).unwrap();
        assert_eq!(row.token_id, "gen_001");
        assert_eq!(row.seller, "fe2virecdb1e91.testnet");
        assert_eq!(row.approval_id, 1);
        assert_eq!(listing_json(&row)["price"], "1000000000000000000000000");
    }

    #[test]
    fn a_rate_limit_keeps_the_snapshot_and_backs_off() {
        let mut snapshot = MarketSnapshot {
            fetched_unix: 1_000,
            backoff_until: 0,
            body: Some(serde_json::json!({"listings": [{"tokenId": "gen_001"}]})),
        };
        assert!(!should_refresh(Some(&snapshot), 1_000 + CACHE_TTL_SECS - 1));
        assert!(should_refresh(Some(&snapshot), 1_000 + CACHE_TTL_SECS));
        apply_rate_limit(&mut snapshot, 2_000);
        assert_eq!(snapshot.backoff_until, 2_000 + RATE_BACKOFF_SECS);
        assert!(!should_refresh(Some(&snapshot), snapshot.backoff_until - 1));
        assert!(snapshot.body.is_some());
        assert!(is_rate_limited(429, ""));
        assert!(is_rate_limited(200, "jsonrpc error -429"));
    }

    fn push_str(buf: &mut Vec<u8>, text: &str) {
        buf.extend_from_slice(&(text.len() as u32).to_le_bytes());
        buf.extend_from_slice(text.as_bytes());
    }
}

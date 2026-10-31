//! Read-only market data for the fe2VI quote board.
//!
//! This crate only parses Hyperliquid `info` responses and owns the offline
//! fixture. It never opens a socket and never holds a key.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketKind {
    Crypto,
    Stock,
    Etf,
    Index,
    Commodity,
    Fx,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    pub id: String,
    pub symbol: String,
    pub dex: String,
    pub kind: MarketKind,
    pub mark_px: f64,
    pub prev_day_px: f64,
    pub change_px: f64,
    pub change_pct: f64,
    pub funding: f64,
    pub open_interest: f64,
    pub volume_24h: f64,
    pub oracle_px: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    pub t: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub coin: String,
    pub size: f64,
    pub entry_px: f64,
    pub position_value: f64,
    pub unrealized_pnl: f64,
    pub leverage: f64,
    pub leverage_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub account_value: f64,
    pub withdrawable: f64,
    pub margin_used: f64,
    pub positions: Vec<Position>,
}

#[derive(Debug, Error)]
pub enum MarketError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unexpected market payload: {0}")]
    Shape(&'static str),
}

/// Parse a Hyperliquid `metaAndAssetCtxs` response. `meta.universe[i].name`
/// aligns with `ctxs[i]`; absent context fields become `0.0`.
pub fn parse_meta_and_asset_ctxs(dex: &str, body: &str) -> Result<Vec<Market>, MarketError> {
    let payload: Value = serde_json::from_str(body)?;
    let parts = payload
        .as_array()
        .ok_or(MarketError::Shape("expected [meta, ctxs]"))?;
    let meta = parts.first().ok_or(MarketError::Shape("missing meta"))?;
    let universe = meta
        .get("universe")
        .and_then(Value::as_array)
        .ok_or(MarketError::Shape("missing meta.universe"))?;
    let empty = Vec::new();
    let ctxs = parts.get(1).and_then(Value::as_array).unwrap_or(&empty);

    let mut markets = Vec::with_capacity(universe.len());
    for (index, asset) in universe.iter().enumerate() {
        let Some(name) = asset.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }
        let ctx = ctxs.get(index).unwrap_or(&Value::Null);
        let mark_px = field_f64(ctx, "markPx");
        let prev_day_px = field_f64(ctx, "prevDayPx");
        let change_px = mark_px - prev_day_px;
        let change_pct = if prev_day_px != 0.0 {
            change_px / prev_day_px * 100.0
        } else {
            0.0
        };
        let symbol = name.rsplit(':').next().unwrap_or(name).to_string();
        markets.push(Market {
            id: name.to_string(),
            symbol: symbol.clone(),
            dex: dex.to_string(),
            kind: classify(&symbol, dex),
            mark_px,
            prev_day_px,
            change_px,
            change_pct,
            funding: field_f64(ctx, "funding"),
            open_interest: field_f64(ctx, "openInterest"),
            volume_24h: field_f64(ctx, "dayNtlVlm"),
            oracle_px: field_f64(ctx, "oraclePx"),
        });
    }
    Ok(markets)
}

/// Parse a Hyperliquid `candleSnapshot` response.
pub fn parse_candles(body: &str, _coin: &str) -> Result<Vec<Candle>, MarketError> {
    let payload: Value = serde_json::from_str(body)?;
    let rows = payload
        .as_array()
        .ok_or(MarketError::Shape("expected a candle array"))?;
    let mut candles = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        let t = object
            .get("t")
            .and_then(as_u64)
            .ok_or(MarketError::Shape("missing candle.t"))?;
        let open = object
            .get("o")
            .and_then(as_f64)
            .ok_or(MarketError::Shape("missing candle.o"))?;
        let high = object
            .get("h")
            .and_then(as_f64)
            .ok_or(MarketError::Shape("missing candle.h"))?;
        let low = object
            .get("l")
            .and_then(as_f64)
            .ok_or(MarketError::Shape("missing candle.l"))?;
        let close = object
            .get("c")
            .and_then(as_f64)
            .ok_or(MarketError::Shape("missing candle.c"))?;
        let volume = object.get("v").and_then(as_f64).unwrap_or(0.0);
        candles.push(Candle {
            t,
            open,
            high,
            low,
            close,
            volume,
        });
    }
    candles.sort_by_key(|candle| candle.t);
    Ok(candles)
}

/// Parse a Hyperliquid `clearinghouseState` response into the public account
/// summary. Only numbers and market names are kept; no key material is read.
pub fn parse_clearinghouse_state(body: &str) -> Result<AccountState, MarketError> {
    let payload: Value = serde_json::from_str(body)?;
    let margin = payload
        .get("marginSummary")
        .ok_or(MarketError::Shape("missing marginSummary"))?;
    let mut positions = Vec::new();
    if let Some(rows) = payload.get("assetPositions").and_then(Value::as_array) {
        for row in rows {
            let Some(position) = row.get("position") else {
                continue;
            };
            let Some(coin) = position.get("coin").and_then(Value::as_str) else {
                continue;
            };
            let size = field_f64(position, "szi");
            if coin.trim().is_empty() || size == 0.0 {
                continue;
            }
            let leverage = position.get("leverage");
            positions.push(Position {
                coin: coin.to_string(),
                size,
                entry_px: field_f64(position, "entryPx"),
                position_value: field_f64(position, "positionValue"),
                unrealized_pnl: field_f64(position, "unrealizedPnl"),
                leverage: leverage
                    .map(|value| field_f64(value, "value"))
                    .unwrap_or(0.0),
                leverage_type: leverage
                    .and_then(|value| value.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("cross")
                    .to_string(),
            });
        }
    }
    positions.sort_by(|left, right| {
        right
            .position_value
            .partial_cmp(&left.position_value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.coin.cmp(&right.coin))
    });
    Ok(AccountState {
        account_value: field_f64(margin, "accountValue"),
        withdrawable: field_f64(&payload, "withdrawable"),
        margin_used: field_f64(margin, "totalMarginUsed"),
        positions,
    })
}

/// Classify a ticker into the quote board's display bucket.
pub fn classify(symbol: &str, dex: &str) -> MarketKind {
    let ticker = symbol
        .rsplit(':')
        .next()
        .unwrap_or(symbol)
        .trim()
        .to_ascii_uppercase();

    if matches!(
        ticker.as_str(),
        "BTC"
            | "ETH"
            | "SOL"
            | "HYPE"
            | "NEAR"
            | "AVAX"
            | "ARB"
            | "OP"
            | "DOGE"
            | "XRP"
            | "ADA"
            | "LINK"
            | "SUI"
            | "TON"
            | "BNB"
            | "LTC"
            | "BCH"
            | "ATOM"
            | "APT"
            | "TIA"
            | "SEI"
            | "WLD"
            | "PEPE"
            | "WIF"
            | "BONK"
            | "SHIB"
            | "FLOKI"
            | "BOME"
            | "MEW"
            | "POPCAT"
            | "MOODENG"
            | "PNUT"
            | "GOAT"
            | "TRUMP"
            | "VIRTUAL"
            | "FARTCOIN"
            | "JUP"
            | "RENDER"
            | "INJ"
            | "STX"
            | "FTM"
            | "S"
            | "ENA"
            | "PENDLE"
            | "ONDO"
            | "AAVE"
            | "UNI"
            | "MKR"
            | "CRV"
            | "LDO"
            | "ETHFI"
            | "EIGEN"
            | "TAO"
            | "FET"
            | "RNDR"
            | "TRX"
            | "DOT"
            | "MATIC"
            | "POL"
            | "KAS"
            | "HBAR"
            | "ICP"
            | "FIL"
            | "ORDI"
    ) {
        return MarketKind::Crypto;
    }

    if matches!(
        ticker.as_str(),
        "XYZ100" | "SP500" | "US500" | "USTECH" | "JP225" | "KR200" | "SMALL2000" | "BVIV" | "DXY"
    ) {
        return MarketKind::Index;
    }

    if matches!(
        ticker.as_str(),
        "GOLD"
            | "SILVER"
            | "COPPER"
            | "PLATINUM"
            | "PALLADIUM"
            | "BRENTOIL"
            | "CL"
            | "NATGAS"
            | "CORN"
            | "URANIUM"
            | "ALUMINIUM"
    ) {
        return MarketKind::Commodity;
    }

    if matches!(
        ticker.as_str(),
        "EUR" | "JPY" | "GBP" | "AUD" | "CAD" | "CHF" | "CNH"
    ) {
        return MarketKind::Fx;
    }

    if matches!(
        ticker.as_str(),
        "SPY"
            | "QQQ"
            | "IWM"
            | "TLT"
            | "SMH"
            | "SOXL"
            | "EWZ"
            | "EWY"
            | "EWJ"
            | "MAGS"
            | "XLE"
            | "URNM"
    ) {
        return MarketKind::Etf;
    }

    if dex.eq_ignore_ascii_case("xyz") {
        return MarketKind::Stock;
    }

    MarketKind::Other
}

/// Offline rows used when the read-only Hyperliquid call is unavailable.
pub fn fixture_markets() -> Vec<Market> {
    vec![
        fixture_row(
            "xyz:AAPL",
            "xyz",
            232.45,
            229.80,
            0.0000125,
            2_400_000_000.0,
            1_850_000_000.0,
            232.40,
        ),
        fixture_row(
            "xyz:NVDA",
            "xyz",
            188.75,
            184.20,
            0.0000180,
            3_100_000_000.0,
            5_200_000_000.0,
            188.60,
        ),
        fixture_row(
            "xyz:TSLA",
            "xyz",
            412.30,
            419.10,
            -0.0000140,
            1_700_000_000.0,
            3_400_000_000.0,
            412.75,
        ),
        fixture_row(
            "xyz:MSFT",
            "xyz",
            512.60,
            508.25,
            0.0000090,
            980_000_000.0,
            1_250_000_000.0,
            512.40,
        ),
        fixture_row(
            "xyz:AMZN",
            "xyz",
            231.40,
            228.05,
            0.0000110,
            1_150_000_000.0,
            1_620_000_000.0,
            231.20,
        ),
        fixture_row(
            "xyz:GOLD",
            "xyz",
            2745.20,
            2718.80,
            0.0000060,
            640_000_000.0,
            890_000_000.0,
            2744.90,
        ),
        fixture_row(
            "xyz:SP500",
            "xyz",
            6128.50,
            6092.25,
            0.0000040,
            2_050_000_000.0,
            2_800_000_000.0,
            6127.80,
        ),
        fixture_row(
            "xyz:XYZ100",
            "xyz",
            21840.30,
            21712.60,
            0.0000030,
            1_300_000_000.0,
            1_950_000_000.0,
            21835.10,
        ),
        fixture_row(
            "BTC",
            "",
            104_250.00,
            101_800.00,
            0.0000125,
            4_200_000_000.0,
            8_400_000_000.0,
            104_180.00,
        ),
        fixture_row(
            "ETH",
            "",
            3_890.50,
            3_975.20,
            0.0000100,
            2_100_000_000.0,
            4_900_000_000.0,
            3_888.10,
        ),
        fixture_row(
            "SOL",
            "",
            218.40,
            211.75,
            0.0000220,
            890_000_000.0,
            2_250_000_000.0,
            218.20,
        ),
        fixture_row(
            "HYPE",
            "",
            42.85,
            40.90,
            0.0000310,
            420_000_000.0,
            780_000_000.0,
            42.78,
        ),
    ]
}

/// Put the busiest rows first for the spectator board.
pub fn sort_for_display(mut markets: Vec<Market>) -> Vec<Market> {
    markets.sort_by(|a, b| {
        b.volume_24h
            .partial_cmp(&a.volume_24h)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    markets
}

fn fixture_row(
    id: &str,
    dex: &str,
    mark_px: f64,
    prev_day_px: f64,
    funding: f64,
    open_interest: f64,
    volume_24h: f64,
    oracle_px: f64,
) -> Market {
    let symbol = id.rsplit(':').next().unwrap_or(id).to_string();
    let change_px = mark_px - prev_day_px;
    let change_pct = if prev_day_px != 0.0 {
        change_px / prev_day_px * 100.0
    } else {
        0.0
    };
    Market {
        id: id.to_string(),
        kind: classify(&symbol, dex),
        symbol,
        dex: dex.to_string(),
        mark_px,
        prev_day_px,
        change_px,
        change_pct,
        funding,
        open_interest,
        volume_24h,
        oracle_px,
    }
}

fn field_f64(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(as_f64).unwrap_or(0.0)
}

fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const META_AND_CTXS: &str = r#"[
        {
            "universe": [
                {"name": "xyz:AAPL"},
                {"name": "BTC"},
                {"name": "xyz:GOLD"}
            ]
        },
        [
            {
                "markPx": "232.5",
                "prevDayPx": "230.0",
                "funding": "0.0000125",
                "openInterest": "1234.5",
                "dayNtlVlm": "987654.0",
                "oraclePx": "232.4"
            },
            {
                "markPx": "104000",
                "prevDayPx": "100000",
                "funding": "0.00001",
                "openInterest": "100",
                "dayNtlVlm": "5000000",
                "oraclePx": "103900"
            },
            {
                "markPx": "2700",
                "prevDayPx": "2700"
            }
        ]
    ]"#;

    #[test]
    fn parses_meta_and_contexts_in_order() {
        let markets = parse_meta_and_asset_ctxs("xyz", META_AND_CTXS).expect("parse");
        assert_eq!(markets.len(), 3);

        let aapl = &markets[0];
        assert_eq!(aapl.id, "xyz:AAPL");
        assert_eq!(aapl.symbol, "AAPL");
        assert_eq!(aapl.kind, MarketKind::Stock);
        assert!((aapl.change_px - 2.5).abs() < 1e-9);
        assert!((aapl.change_pct - (2.5 / 230.0 * 100.0)).abs() < 1e-9);
        assert!((aapl.funding - 0.0000125).abs() < 1e-12);

        let btc = &markets[1];
        assert_eq!(btc.id, "BTC");
        assert_eq!(btc.symbol, "BTC");
        assert_eq!(btc.kind, MarketKind::Crypto);
        assert!((btc.change_pct - 4.0).abs() < 1e-9);

        let gold = &markets[2];
        assert_eq!(gold.kind, MarketKind::Commodity);
        assert_eq!(gold.funding, 0.0);
        assert_eq!(gold.open_interest, 0.0);
        assert_eq!(gold.volume_24h, 0.0);
        assert_eq!(gold.oracle_px, 0.0);
        assert_eq!(gold.change_px, 0.0);
    }

    #[test]
    fn classifies_the_display_buckets() {
        assert_eq!(classify("BTC", ""), MarketKind::Crypto);
        assert_eq!(classify("xyz:AAPL", "xyz"), MarketKind::Stock);
        assert_eq!(classify("SPY", "xyz"), MarketKind::Etf);
        assert_eq!(classify("xyz:XYZ100", "xyz"), MarketKind::Index);
        assert_eq!(classify("GOLD", "xyz"), MarketKind::Commodity);
        assert_eq!(classify("EUR", "xyz"), MarketKind::Fx);
        assert_eq!(classify("MYSTERY", "other"), MarketKind::Other);
    }

    #[test]
    fn fixture_is_non_empty_and_sorted_by_volume() {
        let markets = fixture_markets();
        assert!(markets.len() >= 12);
        assert!(markets.iter().any(|row| row.id == "xyz:AAPL"));
        assert!(markets.iter().any(|row| row.id == "BTC"));
        assert!(markets.iter().all(|row| row.mark_px != 0.0));
        assert!(markets.iter().all(|row| row.volume_24h != 0.0));

        let sorted = sort_for_display(markets);
        assert!(sorted
            .windows(2)
            .all(|pair| pair[0].volume_24h >= pair[1].volume_24h));
    }

    #[test]
    fn parses_candles_from_strings_and_numbers() {
        let body = r#"[
            {"t": 1700000000000, "T": 1700003600000, "s": "BTC", "i": "1h", "o": "100.0", "c": "101.5", "h": "102.0", "l": "99.0", "v": "12.5", "n": 3},
            {"t": 1700003600000, "T": 1700007200000, "s": "BTC", "i": "1h", "o": 101.5, "c": 103.0, "h": 103.5, "l": 101.0, "v": 8, "n": 2}
        ]"#;
        let candles = parse_candles(body, "BTC").expect("parse candles");
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].t, 1_700_000_000_000);
        assert_eq!(candles[0].open, 100.0);
        assert_eq!(candles[0].close, 101.5);
        assert_eq!(candles[1].volume, 8.0);
    }

    #[test]
    fn parses_clearinghouse_state_and_skips_flat_positions() {
        let body = r#"{
            "marginSummary": {
                "accountValue": "12500.5",
                "totalNtlPos": "8000",
                "totalRawUsd": "4500.5",
                "totalMarginUsed": "1200"
            },
            "crossMarginSummary": {
                "accountValue": "12500.5",
                "totalNtlPos": "8000",
                "totalRawUsd": "4500.5",
                "totalMarginUsed": "1200"
            },
            "withdrawable": "11000.25",
            "assetPositions": [
                {"type": "oneWay", "position": {
                    "coin": "xyz:NVDA", "szi": "-12.5", "entryPx": "190.2",
                    "positionValue": "2359.375", "unrealizedPnl": "18.125",
                    "leverage": {"type": "cross", "value": 5}
                }},
                {"type": "oneWay", "position": {
                    "coin": "BTC", "szi": "0", "entryPx": "0",
                    "positionValue": "0", "unrealizedPnl": "0",
                    "leverage": {"type": "isolated", "value": 10}
                }},
                {"type": "oneWay", "position": {
                    "coin": "ETH", "szi": "2", "entryPx": "3800",
                    "positionValue": "7800", "unrealizedPnl": "-40",
                    "leverage": {"type": "isolated", "value": 3}
                }}
            ]
        }"#;
        let account = parse_clearinghouse_state(body).expect("parse account");
        assert!((account.account_value - 12500.5).abs() < 1e-9);
        assert!((account.withdrawable - 11000.25).abs() < 1e-9);
        assert!((account.margin_used - 1200.0).abs() < 1e-9);
        assert_eq!(account.positions.len(), 2);
        assert_eq!(account.positions[0].coin, "ETH");
        assert!((account.positions[0].size - 2.0).abs() < 1e-9);
        assert_eq!(account.positions[0].leverage_type, "isolated");
        assert_eq!(account.positions[1].coin, "xyz:NVDA");
        assert!((account.positions[1].unrealized_pnl - 18.125).abs() < 1e-9);
        assert!((account.positions[1].leverage - 5.0).abs() < 1e-9);
    }

    #[test]
    fn account_json_uses_camel_case() {
        let body = r#"{"marginSummary":{"accountValue":"10","totalMarginUsed":"1"},"withdrawable":"9","assetPositions":[]}"#;
        let value =
            serde_json::to_value(parse_clearinghouse_state(body).expect("parse")).expect("json");
        assert!(value.get("accountValue").is_some());
        assert!(value.get("marginUsed").is_some());
        assert!(value.get("positions").is_some());
    }

    #[test]
    fn market_json_uses_camel_case() {
        let mut markets = fixture_markets();
        let value = serde_json::to_value(markets.remove(0)).expect("json");
        assert!(value.get("markPx").is_some());
        assert!(value.get("volume24h").is_some());
        assert!(value.get("mark_px").is_none());
    }
}

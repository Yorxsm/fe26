//! Live, read-only Hyperliquid market data for the spectator API.
//!
//! The upstream `info` endpoint is public and needs no key. Every failure is
//! swallowed into the offline fixture so a board still renders.

use fe2vi_market::{parse_candles, parse_meta_and_asset_ctxs, sort_for_display, Market};
use worker::*;

const INFO_URL: &str = "https://api.hyperliquid.xyz/info";

pub async fn markets(_env: &Env, dex: &str) -> (String, bool) {
    match live_markets(dex).await {
        Ok(rows) => {
            let body = serde_json::json!({
                "source": "hyperliquid",
                "markets": rows,
            })
            .to_string();
            (body, true)
        }
        Err(_) => (fixture_body(), false),
    }
}

pub async fn candles(_env: &Env, coin: &str, interval: &str, limit: u32) -> (String, bool) {
    let limit = limit.clamp(1, 500) as usize;
    let now = Date::now().as_millis();
    let start = now - 7 * 24 * 3600 * 1000;
    let payload = serde_json::json!({
        "type": "candleSnapshot",
        "req": {
            "coin": coin,
            "interval": normalize_interval(interval),
            "startTime": start,
        },
    });

    match post_info(&payload.to_string()).await {
        Ok(body) => match parse_candles(&body, coin) {
            Ok(mut rows) => {
                if rows.len() > limit {
                    rows = rows.split_off(rows.len() - limit);
                }
                let body = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string());
                (body, true)
            }
            Err(_) => ("[]".to_string(), false),
        },
        Err(_) => ("[]".to_string(), false),
    }
}

async fn live_markets(dex: &str) -> std::result::Result<Vec<Market>, String> {
    let payload = serde_json::json!({
        "type": "metaAndAssetCtxs",
        "dex": dex,
    });
    let body = post_info(&payload.to_string()).await?;
    let rows = parse_meta_and_asset_ctxs(dex, &body).map_err(|err| err.to_string())?;
    if rows.is_empty() {
        return Err("empty market response".to_string());
    }
    Ok(sort_for_display(rows))
}

async fn post_info(body: &str) -> std::result::Result<String, String> {
    let headers = Headers::new();
    headers
        .set("content-type", "application/json")
        .map_err(|err| err.to_string())?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(wasm_bindgen::JsValue::from_str(body)));

    let request = Request::new_with_init(INFO_URL, &init).map_err(|err| err.to_string())?;
    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status_code();
    let text = response.text().await.map_err(|err| err.to_string())?;
    if !(200..300).contains(&status) {
        return Err(format!("hyperliquid info returned {status}"));
    }
    Ok(text)
}

fn fixture_body() -> String {
    let rows = sort_for_display(fe2vi_market::fixture_markets());
    serde_json::json!({
        "source": "fixture",
        "markets": rows,
    })
    .to_string()
}

fn normalize_interval(interval: &str) -> &str {
    match interval {
        "1m" | "3m" | "5m" | "15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "12h" | "1d" | "3d"
        | "1w" | "1M" => interval,
        _ => "1h",
    }
}

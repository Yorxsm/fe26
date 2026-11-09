//! Venue adapters for the village tape. The mock is the default.
//! Live Hyperliquid reads and orders use the testnet host. No adapter here opens a socket.

use serde::{Deserialize, Serialize};

use crate::price::simulated_price_micros;

pub const HYPERLIQUID_TESTNET_INFO: &str = "https://api.hyperliquid-testnet.xyz/info";
pub const HYPERLIQUID_TESTNET_EXCHANGE: &str = "https://api.hyperliquid-testnet.xyz/exchange";
pub const HYPERLIQUID_MAINNET_INFO: &str = "https://api.hyperliquid.xyz/info";
pub const HYPERLIQUID_INFO: &str = HYPERLIQUID_TESTNET_INFO;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub kind: String,
    pub mark_micros: u64,
    pub funding_bps: i32,
    pub pnl_micros: i64,
}

pub trait Venue {
    fn id(&self) -> &'static str;
    fn network(&self) -> &'static str;
    fn quotes(&self, now: u64) -> Vec<Quote>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MockVenue;

impl Venue for MockVenue {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn network(&self) -> &'static str {
        "mock"
    }

    fn quotes(&self, now: u64) -> Vec<Quote> {
        const ROWS: &[(&str, &str)] = &[
            ("PERP-NEAR", "perp"),
            ("PERP-BTC", "perp"),
            ("STK-IRON", "stock"),
            ("STK-VAULT", "stock"),
        ];
        ROWS.iter()
            .enumerate()
            .map(|(index, (symbol, kind))| {
                let mark = simulated_price_micros(symbol, now);
                let prev = simulated_price_micros(symbol, now.saturating_sub(3600));
                let pnl = i64::try_from(mark).unwrap_or(0) - i64::try_from(prev).unwrap_or(0);
                Quote {
                    symbol: (*symbol).to_string(),
                    kind: (*kind).to_string(),
                    mark_micros: mark,
                    funding_bps: 2 + index as i32,
                    pnl_micros: pnl,
                }
            })
            .collect()
    }
}

pub fn default_venue() -> MockVenue {
    MockVenue
}

#[derive(Clone, Copy, Debug)]
pub struct VenueNote {
    pub id: &'static str,
    pub offers: &'static str,
    pub wired: bool,
}

pub fn venue_notes() -> &'static [VenueNote] {
    &[
        VenueNote {
            id: "near.com",
            offers: "Perps on a Hyperliquid integration. The arena does not place orders.",
            wired: false,
        },
        VenueNote {
            id: "trade.xyz",
            offers:
                "HIP-3 perps on the xyz deployment, including stock perps. HyperCore settles them.",
            wired: false,
        },
    ]
}

/// Trading is testnet only. A mainnet Hyperliquid host is refused.
/// The name is kept because callers already use it as the venue gate.
pub fn refuse_testnet(url: &str) -> Result<(), &'static str> {
    let lower = url.to_ascii_lowercase();
    if lower.contains("hyperliquid.xyz") && !lower.contains("hyperliquid-testnet") {
        return Err("mainnet venue refused");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trading_uses_testnet_and_refuses_mainnet() {
        let venue = default_venue();
        assert_eq!(venue.network(), "mock");
        let quotes = venue.quotes(1_793_116_800);
        assert_eq!(quotes.len(), 4);
        assert!(quotes.iter().any(|quote| quote.kind == "stock"));
        assert!(refuse_testnet(HYPERLIQUID_TESTNET_INFO).is_ok());
        assert!(refuse_testnet(HYPERLIQUID_TESTNET_EXCHANGE).is_ok());
        assert!(refuse_testnet(HYPERLIQUID_MAINNET_INFO).is_err());
        assert!(venue_notes().iter().all(|note| !note.wired));
    }
}

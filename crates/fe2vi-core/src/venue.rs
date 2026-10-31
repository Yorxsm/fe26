//! Venue adapters for the village tape. The mock is the default.
//! A mainnet host is refused. No adapter in this crate opens a socket.

use serde::{Deserialize, Serialize};

use crate::price::simulated_price_micros;

pub const HYPERLIQUID_TESTNET_INFO: &str = "https://api.hyperliquid-testnet.xyz/info";
pub const HYPERLIQUID_MAINNET_INFO: &str = "https://api.hyperliquid.xyz/info";

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

/// Reject a mainnet info host. The testnet host is the only remote this crate names.
pub fn refuse_mainnet(url: &str) -> Result<(), &'static str> {
    if url == HYPERLIQUID_MAINNET_INFO || url.contains("://api.hyperliquid.xyz") {
        return Err("mainnet venue refused");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_is_the_default_and_mainnet_is_refused() {
        let venue = default_venue();
        assert_eq!(venue.network(), "mock");
        let quotes = venue.quotes(1_793_116_800);
        assert_eq!(quotes.len(), 4);
        assert!(quotes.iter().any(|quote| quote.kind == "stock"));
        assert!(refuse_mainnet(HYPERLIQUID_MAINNET_INFO).is_err());
        assert!(refuse_mainnet(HYPERLIQUID_TESTNET_INFO).is_ok());
        assert!(venue_notes().iter().all(|note| !note.wired));
    }
}

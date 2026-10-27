//! Deterministic simulated prices. No network, no mainnet asset, no float.

/// NEAR Intents asset id for wrapped NEAR. Amounts elsewhere stay in yocto.
pub const WRAP_NEAR: &str = "nep141:wrap.near";

/// USD micros (1_000_000 = $1) for one whole token of `asset` in this minute.
/// The same inputs always return the same price. A new minute can move it.
pub fn simulated_price_micros(asset: &str, unix_secs: u64) -> u64 {
    let bucket = (unix_secs / 60) as u32;
    let mut acc: u32 = 2_166_136_261;
    for byte in asset.bytes() {
        acc ^= u32::from(byte);
        acc = acc.wrapping_mul(16_777_619);
    }
    acc ^= bucket;
    acc = acc.wrapping_mul(16_777_619);
    2_500_000 + u64::from(acc % 1_500_000)
}

/// Where a displayed price came from. Remote quotes are optional and read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceSource {
    Simulated,
    Intents,
}

/// `intents_quotes` is the flag. A remote figure is used only when the flag is
/// on and a quote was actually supplied. Otherwise the simulated engine wins.
pub fn select_price(
    asset: &str,
    unix_secs: u64,
    intents_quotes: bool,
    remote_micros: Option<u64>,
) -> (u64, PriceSource) {
    if intents_quotes {
        if let Some(remote) = remote_micros {
            return (remote, PriceSource::Intents);
        }
    }
    (
        simulated_price_micros(asset, unix_secs),
        PriceSource::Simulated,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_minute_is_stable_and_the_next_minute_can_move() {
        let first = simulated_price_micros(WRAP_NEAR, 1_720_000_000);
        assert_eq!(first, simulated_price_micros(WRAP_NEAR, 1_720_000_010));
        assert_ne!(first, simulated_price_micros(WRAP_NEAR, 1_720_000_060));
        assert!((2_500_000..4_000_000).contains(&first));
    }

    #[test]
    fn quotes_stay_simulated_unless_a_remote_figure_is_provided() {
        let (price, source) = select_price(WRAP_NEAR, 1_720_000_000, true, None);
        assert_eq!(source, PriceSource::Simulated);
        assert_eq!(price, simulated_price_micros(WRAP_NEAR, 1_720_000_000));
        let (remote, source) = select_price(WRAP_NEAR, 1_720_000_000, true, Some(3_250_000));
        assert_eq!((remote, source), (3_250_000, PriceSource::Intents));
        let (_, off) = select_price(WRAP_NEAR, 1_720_000_000, false, Some(3_250_000));
        assert_eq!(off, PriceSource::Simulated);
    }
}

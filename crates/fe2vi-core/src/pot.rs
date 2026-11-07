//! The call check inside heist `resolve`. Payout amounts stay in the contract.

/// Whether `resolve` would accept the call. The error strings are the contract panics.
pub fn resolve_call(
    resolved: bool,
    delisted: bool,
    started_at_ns: u64,
    duration_seconds: u64,
    now_ns: u64,
) -> Result<(), &'static str> {
    if resolved {
        return Err("already resolved");
    }
    if !delisted {
        let Some(duration_ns) = duration_seconds.checked_mul(1_000_000_000) else {
            return Err("duration overflow");
        };
        if now_ns < started_at_ns.saturating_add(duration_ns) {
            return Err("game still running");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_a_second_call_and_a_live_clock() {
        assert_eq!(
            resolve_call(true, false, 1_000, 60, 1_000 + 60 * 1_000_000_000),
            Err("already resolved")
        );
        assert_eq!(
            resolve_call(false, false, 1_000, 60, 1_000),
            Err("game still running")
        );
        assert_eq!(
            resolve_call(false, false, 1_000, 60, 1_000 + 60 * 1_000_000_000),
            Ok(())
        );
        assert_eq!(resolve_call(false, true, 1_000, 60, 1_000), Ok(()));
    }
}

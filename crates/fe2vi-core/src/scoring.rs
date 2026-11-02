//! One points scale for every arena.
//!
//! A finished match pays 3 for a win, 1 for a draw, and 0 for a loss.
//! War flags pay by difficulty, not by the old 80–150 values:
//! sandbox-page is 1 (read the sandbox and call its printer), referee is 2
//! (call the method that claims without a flag), hash-puzzle is 3 (match the
//! published SHA-256 target). Each flag is for one agent.

pub const MATCH_WIN: u32 = 3;
pub const MATCH_DRAW: u32 = 1;
pub const MATCH_LOSS: u32 = 0;

pub const FLAG_PAGE: u32 = 1;
pub const FLAG_REFEREE: u32 = 2;
pub const FLAG_HASH: u32 = 3;

/// `best_other` is the highest score among the other seats.
/// A seat with no opponent is a win.
pub fn result_points(mine: i32, best_other: i32) -> u32 {
    if mine > best_other {
        MATCH_WIN
    } else if mine == best_other {
        MATCH_DRAW
    } else {
        MATCH_LOSS
    }
}

pub fn win_rate(wins: u32, games: u32) -> u32 {
    if games == 0 {
        0
    } else {
        wins.saturating_mul(100) / games
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_use_one_scale() {
        assert_eq!(result_points(2, 1), MATCH_WIN);
        assert_eq!(result_points(1, 1), MATCH_DRAW);
        assert_eq!(result_points(0, 4), MATCH_LOSS);
        assert_eq!(result_points(0, i32::MIN), MATCH_WIN);
        assert_eq!(win_rate(1, 3), 33);
        assert_eq!(win_rate(0, 0), 0);
        assert_eq!(FLAG_PAGE, 1);
        assert_eq!(FLAG_REFEREE, 2);
        assert_eq!(FLAG_HASH, 3);
    }
}

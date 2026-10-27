use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Scout,
    Steal,
    Sabotage,
    Hide,
    /// Trading Cup only. Even rounds settle this side.
    Long,
    /// Trading Cup only. Odd rounds settle this side.
    Short,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmittedAction {
    pub player: String,
    pub kind: ActionKind,
    pub target: Option<String>,
}

/// Fixed prices derived from the game's per-action cap. The transaction must
/// name this exact amount so the spend is visible in the arguments.
pub fn action_price(kind: ActionKind, per_action_cap: u128) -> Result<u128, &'static str> {
    let price = match kind {
        ActionKind::Scout | ActionKind::Hide => {
            if per_action_cap < 4 {
                return Err("per-action cap too small");
            }
            per_action_cap / 4
        }
        ActionKind::Sabotage => {
            if per_action_cap < 2 {
                return Err("per-action cap too small");
            }
            per_action_cap / 2
        }
        ActionKind::Steal | ActionKind::Long | ActionKind::Short => per_action_cap,
    };
    if price == 0 {
        Err("price is zero")
    } else {
        Ok(price)
    }
}

/// `Some(round)` while the game is inside its duration. `None` before the start
/// or once the duration has elapsed.
pub fn round_index(start_ns: u64, now_ns: u64, duration_seconds: u64, rounds: u32) -> Option<u32> {
    if rounds == 0 || now_ns < start_ns {
        return None;
    }
    let duration_ns = duration_seconds.checked_mul(1_000_000_000)?;
    if duration_ns == 0 {
        return None;
    }
    let elapsed = now_ns.checked_sub(start_ns)?;
    if elapsed >= duration_ns {
        return None;
    }
    let len = duration_ns / u64::from(rounds);
    if len == 0 {
        return Some(0);
    }
    let mut index = u32::try_from(elapsed / len).ok()?;
    if index >= rounds {
        index = rounds - 1;
    }
    Some(index)
}

/// Score one round. Points, not yoctoNEAR: scout 1, hide 1, steal 3 unless
/// sabotaged, sabotage 2 only when it hits a steal.
pub fn score_actions(actions: &[SubmittedAction]) -> BTreeMap<String, u128> {
    let mut kind_of: BTreeMap<&str, ActionKind> = BTreeMap::new();
    for action in actions {
        kind_of.insert(action.player.as_str(), action.kind);
    }
    let mut sabotaged = BTreeMap::<&str, bool>::new();
    for action in actions {
        if action.kind == ActionKind::Sabotage {
            if let Some(target) = action.target.as_deref() {
                sabotaged.insert(target, true);
            }
        }
    }
    let mut scores = BTreeMap::new();
    for action in actions {
        let points = match action.kind {
            ActionKind::Scout | ActionKind::Hide => 1,
            ActionKind::Steal => {
                if sabotaged
                    .get(action.player.as_str())
                    .copied()
                    .unwrap_or(false)
                {
                    0
                } else {
                    3
                }
            }
            ActionKind::Sabotage => match action.target.as_deref().and_then(|t| kind_of.get(t)) {
                Some(ActionKind::Steal) => 2,
                _ => 0,
            },
            ActionKind::Long | ActionKind::Short => 0,
        };
        *scores.entry(action.player.clone()).or_insert(0) += points;
    }
    scores
}

/// Public test tape. Even rounds settle long. Odd rounds settle short.
/// This is not a price and not a mainnet asset.
pub fn tape_winner(round: u32) -> ActionKind {
    if round.is_multiple_of(2) {
        ActionKind::Long
    } else {
        ActionKind::Short
    }
}

/// Split `pot` by weights. The last non-zero weight receives the remainder.
pub fn split_pot(pot: u128, weights: &[(String, u128)]) -> Vec<(String, u128)> {
    let total: u128 = weights.iter().map(|(_, weight)| *weight).sum();
    if pot == 0 || total == 0 {
        return Vec::new();
    }
    let nonzero: Vec<&(String, u128)> = weights.iter().filter(|(_, weight)| *weight > 0).collect();
    let mut allocated = 0u128;
    let mut shares = Vec::with_capacity(nonzero.len());
    for (index, (player, weight)) in nonzero.iter().enumerate() {
        let share = if index + 1 == nonzero.len() {
            pot - allocated
        } else {
            let q = pot / total;
            let r = pot % total;
            q * weight + (r * weight) / total
        };
        allocated += share;
        shares.push(((*player).clone(), share));
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabotage_cancels_steal_and_scores() {
        let actions = vec![
            SubmittedAction {
                player: "alice".into(),
                kind: ActionKind::Steal,
                target: None,
            },
            SubmittedAction {
                player: "bob".into(),
                kind: ActionKind::Sabotage,
                target: Some("alice".into()),
            },
            SubmittedAction {
                player: "cara".into(),
                kind: ActionKind::Scout,
                target: None,
            },
        ];
        let scores = score_actions(&actions);
        assert_eq!(scores["alice"], 0);
        assert_eq!(scores["bob"], 2);
        assert_eq!(scores["cara"], 1);
        let weights: Vec<_> = scores.into_iter().collect();
        let shares = split_pot(10, &weights);
        let total: u128 = shares.iter().map(|(_, share)| *share).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn trading_tape_is_deterministic() {
        assert_eq!(tape_winner(0), ActionKind::Long);
        assert_eq!(tape_winner(1), ActionKind::Short);
        assert_eq!(tape_winner(4), ActionKind::Long);
    }

    #[test]
    fn round_index_closes_after_duration() {
        assert_eq!(round_index(0, 0, 100, 4), Some(0));
        assert_eq!(round_index(0, 25_000_000_000, 100, 4), Some(1));
        assert_eq!(round_index(0, 100_000_000_000, 100, 4), None);
    }
}

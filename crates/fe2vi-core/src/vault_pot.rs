//! Vault result settlement. A win pays the whole pot. A draw splits it equally.
//! Arena points are not converted into NEAR.

use crate::score::split_pot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultPotCall {
    Win { winner: String },
    Draw,
    Cancel,
}

/// Shares of `pot` for one authorized call. Player order does not change a draw:
/// accounts are sorted, and the last account receives the remainder.
pub fn vault_pot_shares(
    pot: u128,
    creator: &str,
    players: &[String],
    call: &VaultPotCall,
) -> Result<Vec<(String, u128)>, &'static str> {
    match call {
        VaultPotCall::Cancel => refund(creator, pot),
        VaultPotCall::Win { winner } => {
            let winner = winner.trim();
            if winner.is_empty() {
                return Err("winner required");
            }
            if !players.iter().any(|player| player == winner) {
                return Err("winner is not a player");
            }
            Ok(vec![(winner.to_string(), pot)])
        }
        VaultPotCall::Draw => {
            let mut ordered: Vec<String> = players
                .iter()
                .map(|player| player.trim().to_string())
                .filter(|player| !player.is_empty())
                .collect();
            if ordered.is_empty() {
                return refund(creator, pot);
            }
            ordered.sort();
            ordered.dedup();
            let weights: Vec<(String, u128)> =
                ordered.into_iter().map(|player| (player, 1)).collect();
            Ok(split_pot(pot, &weights))
        }
    }
}

/// One strict high score wins. Any tie is a draw among every participant.
pub fn vault_result_call(scores: &[(String, i64)]) -> Result<VaultPotCall, &'static str> {
    if scores.is_empty() {
        return Err("no players");
    }
    let best = scores.iter().map(|(_, score)| *score).max().unwrap_or(0);
    let mut leaders: Vec<String> = scores
        .iter()
        .filter(|(_, score)| *score == best)
        .map(|(account, _)| account.clone())
        .collect();
    leaders.sort();
    leaders.dedup();
    if leaders.len() == 1 {
        Ok(VaultPotCall::Win {
            winner: leaders.remove(0),
        })
    } else {
        Ok(VaultPotCall::Draw)
    }
}

fn refund(creator: &str, pot: u128) -> Result<Vec<(String, u128)>, &'static str> {
    let creator = creator.trim();
    if creator.is_empty() {
        return Err("creator required");
    }
    Ok(vec![(creator.to_string(), pot)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_win_pays_the_whole_pot_and_a_draw_splits_equally() {
        let players = vec!["bob.testnet".into(), "ann.testnet".into()];
        let win = vault_pot_shares(
            10,
            "creator.testnet",
            &players,
            &VaultPotCall::Win {
                winner: "bob.testnet".into(),
            },
        )
        .unwrap();
        assert_eq!(win, vec![("bob.testnet".into(), 10)]);

        let draw = vault_pot_shares(10, "creator.testnet", &players, &VaultPotCall::Draw).unwrap();
        assert_eq!(
            draw,
            vec![("ann.testnet".into(), 5), ("bob.testnet".into(), 5)]
        );
        let odd = vault_pot_shares(
            10,
            "creator.testnet",
            &["c.testnet".into(), "a.testnet".into(), "b.testnet".into()],
            &VaultPotCall::Draw,
        )
        .unwrap();
        let total: u128 = odd.iter().map(|(_, amount)| *amount).sum();
        assert_eq!(total, 10);
        assert_eq!(odd[0].0, "a.testnet");
        assert_eq!(odd[2].0, "c.testnet");
        assert!(odd[2].1 >= odd[0].1);
    }

    #[test]
    fn cancel_and_a_stranger_do_not_invent_a_payee() {
        let players = vec!["ann.testnet".into()];
        let cancel =
            vault_pot_shares(7, "creator.testnet", &players, &VaultPotCall::Cancel).unwrap();
        assert_eq!(cancel, vec![("creator.testnet".into(), 7)]);
        let err = vault_pot_shares(
            7,
            "creator.testnet",
            &players,
            &VaultPotCall::Win {
                winner: "stranger.testnet".into(),
            },
        )
        .unwrap_err();
        assert_eq!(err, "winner is not a player");
        assert!(matches!(
            vault_result_call(&[("ann.testnet".into(), 3), ("bob.testnet".into(), 3)]).unwrap(),
            VaultPotCall::Draw
        ));
        assert_eq!(
            vault_result_call(&[("ann.testnet".into(), 4), ("bob.testnet".into(), 1)]).unwrap(),
            VaultPotCall::Win {
                winner: "ann.testnet".into()
            }
        );
    }
}

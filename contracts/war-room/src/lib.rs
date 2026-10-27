//! War Room challenge contract.
//!
//! `claim_checked` is the intended path. `force_claim` is public on purpose
//! in this contract: the first caller takes the bounty, and a second caller
//! is rejected. This is the puzzle. It is not a pattern for any other contract,
//! and this repository does not deploy it.

use fe2vi_core::event_json;
use near_sdk::json_types::U128;
use near_sdk::{env, near, AccountId, PanicOnDefault};

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct WarRoom {
    pot_yocto: u128,
    claimed: bool,
    winner: Option<AccountId>,
}

#[near]
impl WarRoom {
    #[init]
    pub fn new(pot_yocto: U128) -> Self {
        Self {
            pot_yocto: pot_yocto.0,
            claimed: false,
            winner: None,
        }
    }

    pub fn pot(&self) -> U128 {
        U128(self.pot_yocto)
    }

    pub fn winner(&self) -> Option<AccountId> {
        self.winner.clone()
    }

    /// Intended path. The flag is the public puzzle string, not a credential.
    pub fn claim_checked(&mut self, flag: String) {
        if flag != "iron-26" {
            env::panic_str("flag does not match");
        }
        self.take();
    }

    /// Intentional bug, this contract only: no flag and no owner check.
    pub fn force_claim(&mut self) {
        self.take();
    }

    fn take(&mut self) {
        if self.claimed {
            env::panic_str("bounty already claimed");
        }
        let winner = env::predecessor_account_id();
        self.claimed = true;
        self.winner = Some(winner.clone());
        env::log_str(&event_json(
            "bounty_claimed",
            serde_json::json!({
                "winner": winner,
                "pot_yocto": self.pot_yocto.to_string(),
                "challenge": "war-room",
            }),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn as_account(name: &str) {
        let context = VMContextBuilder::new()
            .current_account_id("war.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(name.parse().unwrap())
            .signer_account_id(name.parse().unwrap())
            .build();
        testing_env!(context);
    }

    fn panic_text(err: Box<dyn std::any::Any + Send>) -> String {
        if let Some(text) = err.downcast_ref::<String>() {
            return text.clone();
        }
        if let Some(text) = err.downcast_ref::<&str>() {
            return (*text).to_string();
        }
        "panic".to_string()
    }

    #[test]
    fn the_first_force_claim_wins_and_the_second_does_not() {
        as_account("pike.war.fe2vi.testnet");
        let mut room = WarRoom::new(U128(10));
        room.force_claim();
        assert_eq!(room.winner().unwrap().as_str(), "pike.war.fe2vi.testnet");
        as_account("other.fe2vi.testnet");
        let err = catch_unwind(AssertUnwindSafe(|| room.force_claim())).unwrap_err();
        assert!(panic_text(err).contains("already claimed"));
        assert_eq!(room.winner().unwrap().as_str(), "pike.war.fe2vi.testnet");
    }

    #[test]
    fn a_wrong_flag_does_not_pay_and_the_checked_path_can() {
        as_account("pike.war.fe2vi.testnet");
        let mut room = WarRoom::new(U128(10));
        let err = catch_unwind(AssertUnwindSafe(|| room.claim_checked("nope".into()))).unwrap_err();
        assert!(panic_text(err).contains("flag"));
        assert!(room.winner().is_none());
        room.claim_checked("iron-26".into());
        assert!(room.winner().is_some());
    }
}

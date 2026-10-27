//! Arena board. The shipped games are fixed. This contract does not accept a
//! visitor listing or a creator deposit. One moderator account can delist a
//! game. Delist does not choose a payout recipient and cannot seize the pot.

use borsh::{BorshDeserialize, BorshSerialize};
use fe2vi_core::{
    absolute_ceiling, default_platform_caps, event_json, validate_platform_caps, GameRules,
    PlatformCaps,
};
use near_sdk::serde::Serialize;
use near_sdk::store::IterableMap;
use near_sdk::{env, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise};

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct StoredGame {
    creator: AccountId,
    rules: GameRules,
    game_md_hash: String,
    delisted: bool,
    created_at_ns: u64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GameView {
    pub id: String,
    pub creator: AccountId,
    pub rules: GameRules,
    pub game_md_hash: String,
    pub delisted: bool,
    pub created_at_ns: u64,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct ArenaFactory {
    owner: AccountId,
    moderator: AccountId,
    registry: AccountId,
    heist: AccountId,
    caps: PlatformCaps,
    next_id: u64,
    games: IterableMap<String, StoredGame>,
}

#[near]
impl ArenaFactory {
    #[init]
    pub fn new(
        owner: AccountId,
        moderator: AccountId,
        registry: AccountId,
        heist: AccountId,
    ) -> Self {
        Self {
            owner,
            moderator,
            registry,
            heist,
            caps: default_platform_caps(),
            next_id: 1,
            games: IterableMap::new(b"g"),
        }
    }

    /// Single moderator. Players and the game contract cannot delist.
    /// The heist contract is told to freeze play; payout still follows the rules.
    pub fn delist_game(&mut self, game_id: String) {
        require!(
            env::predecessor_account_id() == self.moderator,
            "only the moderator can delist"
        );
        let mut game = self
            .games
            .get(&game_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("game not found"));
        require!(!game.delisted, "already delisted");
        game.delisted = true;
        self.games.insert(game_id.clone(), game);
        Promise::new(self.heist.clone()).function_call(
            "mark_delisted".to_string(),
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({ "game_id": game_id }))
                .unwrap_or_else(|_| env::panic_str("delist args")),
            NearToken::from_yoctonear(0),
            Gas::from_tgas(40),
        );
        env::log_str(&event_json(
            "game_delisted",
            near_sdk::serde_json::json!({
                "id": game_id,
                "moderator": env::predecessor_account_id(),
            }),
        ));
    }

    /// Rotation keeps a single moderator. The previous moderator must call it.
    pub fn set_moderator(&mut self, moderator: AccountId) {
        require!(
            env::predecessor_account_id() == self.moderator,
            "only the moderator can rotate the moderator"
        );
        self.moderator = moderator;
    }

    /// Registry pushes cap changes. A game account cannot.
    pub fn apply_platform_caps(&mut self, caps: PlatformCaps) {
        require!(
            env::predecessor_account_id() == self.registry,
            "game cannot raise platform caps"
        );
        validate_platform_caps(&caps, &absolute_ceiling())
            .unwrap_or_else(|err| env::panic_str(&err.to_string()));
        self.caps = caps;
    }

    pub fn get_platform_caps(&self) -> PlatformCaps {
        self.caps.clone()
    }

    pub fn get_moderator(&self) -> AccountId {
        self.moderator.clone()
    }

    pub fn get_game(&self, game_id: String) -> Option<GameView> {
        self.games.get(&game_id).map(|game| GameView {
            id: game_id,
            creator: game.creator.clone(),
            rules: game.rules.clone(),
            game_md_hash: game.game_md_hash.clone(),
            delisted: game.delisted,
            created_at_ns: game.created_at_ns,
        })
    }

    pub fn list_games(&self) -> Vec<GameView> {
        self.games
            .iter()
            .map(|(id, game)| GameView {
                id: id.clone(),
                creator: game.creator.clone(),
                rules: game.rules.clone(),
                game_md_hash: game.game_md_hash.clone(),
                delisted: game.delisted,
                created_at_ns: game.created_at_ns,
            })
            .collect()
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2vi_core::{heist_night_example, pin_hash, ONE_NEAR};
    use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, NearToken};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn use_pred(predecessor: &str, deposit: u128) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("factory.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(predecessor.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(10_000))
            .prepaid_gas(Gas::from_tgas(300))
            .block_timestamp(1_700_000_000_000_000_000);
        testing_env!(builder.build());
    }

    fn setup() -> ArenaFactory {
        use_pred("owner.fe2vi.testnet", 0);
        ArenaFactory::new(
            "owner.fe2vi.testnet".parse().unwrap(),
            "moderator.fe2vi.testnet".parse().unwrap(),
            "registry.fe2vi.testnet".parse().unwrap(),
            "heist.fe2vi.testnet".parse().unwrap(),
        )
    }

    fn panic_text(err: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = err.downcast_ref::<String>() {
            message.clone()
        } else if let Some(message) = err.downcast_ref::<&str>() {
            (*message).to_string()
        } else {
            String::new()
        }
    }

    fn seed(factory: &mut ArenaFactory) -> String {
        let rules = heist_night_example();
        let id = format!("g{}", factory.next_id);
        factory.next_id += 1;
        factory.games.insert(
            id.clone(),
            StoredGame {
                creator: "heist.fe2vi.testnet".parse().unwrap(),
                rules: rules.clone(),
                game_md_hash: pin_hash(&rules),
                delisted: false,
                created_at_ns: 0,
            },
        );
        id
    }

    #[test]
    fn only_the_moderator_can_delist_a_game() {
        let mut factory = setup();
        let id = seed(&mut factory);

        use_pred("creator.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| {
            factory.delist_game(id.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("only the moderator can delist"));
        assert!(!factory.get_game(id.clone()).unwrap().delisted);

        use_pred("heist.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| {
            factory.delist_game(id.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("only the moderator can delist"));

        use_pred("moderator.fe2vi.testnet", 0);
        factory.delist_game(id.clone());
        assert!(factory.get_game(id.clone()).unwrap().delisted);
        assert!(get_logs()
            .iter()
            .any(|line| line.contains("game_delisted") && line.contains("EVENT_JSON:")));

        let mut forwarded = false;
        for receipt in get_created_receipts() {
            for action in receipt.actions {
                if let near_sdk::mock::MockAction::FunctionCallWeight { method_name, .. } = action {
                    if method_name == b"mark_delisted" {
                        forwarded = true;
                    }
                }
            }
        }
        assert!(forwarded, "delist must notify the heist contract");
    }

    #[test]
    fn game_cannot_raise_platform_caps() {
        let mut factory = setup();
        let original = factory.get_platform_caps();
        use_pred("heist.fe2vi.testnet", 0);
        let mut raised = original.clone();
        raised.max_pot_yocto = original.max_pot_yocto + ONE_NEAR;
        let err = catch_unwind(AssertUnwindSafe(|| {
            factory.apply_platform_caps(raised);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("game cannot raise platform caps"));
        assert_eq!(factory.get_platform_caps(), original);
        assert!(factory.list_games().is_empty());
    }
}

//! Arena board. Visitors cannot open a pot. The owner can open one game by
//! forwarding the exact pot deposit to the heist contract. This contract keeps
//! no fee. One moderator account can delist a game. Delist does not choose a
//! payout recipient and cannot seize the pot.

use borsh::{BorshDeserialize, BorshSerialize};
use fe2vi_core::{
    absolute_ceiling, default_platform_caps, event_json, pin_hash, validate_platform_caps,
    validate_rules, GameRules, PlatformCaps,
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

    /// Owner-only. The attached deposit must equal `rules.pot_yocto` and is
    /// forwarded to `heist.init_game`. The factory does not keep it.
    #[payable]
    pub fn open_game(&mut self, game_id: String, rules: GameRules, creator: AccountId) -> Promise {
        require!(
            env::predecessor_account_id() == self.owner,
            "only the owner can open a game"
        );
        require!(!game_id.is_empty(), "game id required");
        require!(self.games.get(&game_id).is_none(), "game exists");
        validate_rules(&rules, &self.caps).unwrap_or_else(|err| env::panic_str(&err.to_string()));
        let deposit = env::attached_deposit().as_yoctonear();
        require!(deposit == rules.pot_yocto, "pot deposit mismatch");
        let game_md_hash = pin_hash(&rules);
        self.games.insert(
            game_id.clone(),
            StoredGame {
                creator: creator.clone(),
                rules: rules.clone(),
                game_md_hash: game_md_hash.clone(),
                delisted: false,
                created_at_ns: env::block_timestamp(),
            },
        );
        env::log_str(&event_json(
            "game_opened",
            near_sdk::serde_json::json!({
                "id": game_id,
                "creator": creator,
                "pot_yocto": deposit.to_string(),
            }),
        ));
        Promise::new(self.heist.clone()).function_call(
            "init_game".to_string(),
            near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                "game_id": game_id,
                "rules": rules,
                "creator": creator,
                "game_md_hash": game_md_hash,
            }))
            .unwrap_or_else(|_| env::panic_str("open args")),
            NearToken::from_yoctonear(deposit),
            Gas::from_tgas(80),
        )
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
    fn only_the_owner_can_open_a_pot_and_the_deposit_is_forwarded() {
        let mut factory = setup();
        let mut rules = heist_night_example();
        rules.pot_yocto = ONE_NEAR / 20;
        let creator: AccountId = "creator.fe2vi.testnet".parse().unwrap();

        use_pred("visitor.fe2vi.testnet", rules.pot_yocto);
        let err = catch_unwind(AssertUnwindSafe(|| {
            factory.open_game("pot1".into(), rules.clone(), creator.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("only the owner can open a game"));

        use_pred("owner.fe2vi.testnet", rules.pot_yocto - 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            factory.open_game("pot1".into(), rules.clone(), creator.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("pot deposit mismatch"));

        use_pred("owner.fe2vi.testnet", rules.pot_yocto);
        factory.open_game("pot1".into(), rules.clone(), creator.clone());
        let stored = factory.get_game("pot1".into()).unwrap();
        assert_eq!(stored.creator, creator);
        assert_eq!(stored.game_md_hash, pin_hash(&rules));
        assert!(!stored.delisted);

        let mut forwarded = false;
        for receipt in get_created_receipts() {
            assert_eq!(receipt.receiver_id.as_str(), "heist.fe2vi.testnet");
            for action in receipt.actions {
                if let near_sdk::mock::MockAction::FunctionCallWeight {
                    method_name,
                    attached_deposit,
                    args,
                    ..
                } = action
                {
                    if method_name == b"init_game" {
                        forwarded = true;
                        assert_eq!(attached_deposit.as_yoctonear(), rules.pot_yocto);
                        let parsed: near_sdk::serde_json::Value =
                            near_sdk::serde_json::from_slice(&args).unwrap();
                        assert_eq!(parsed["game_id"], "pot1");
                        assert_eq!(parsed["game_md_hash"], pin_hash(&rules));
                    }
                }
            }
        }
        assert!(forwarded, "open_game must call heist.init_game");
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

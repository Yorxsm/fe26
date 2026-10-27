//! Heist Night referee.
//!
//! One deployment hosts many games. The pot and the three items are NEP-245
//! tokens escrowed by this contract. NEAR deposited with the pot stays here
//! until `resolve` pushes it to winners. Before every act or bid, the contract
//! checks the registry's caps and revocation list. A game cannot write those caps.

use borsh::{BorshDeserialize, BorshSerialize};
use fe2vi_core::{
    action_price, authorize_spend, default_platform_caps, event_json, pin_hash, round_index,
    score_actions, split_pot, tape_winner, validate_rules, ActionKind, GameRules, PlatformCaps,
    SpendRequest, SubmittedAction, TEMPLATE_CUP,
};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::store::LookupMap;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
    PromiseResult,
};

const ITEMS: [&str; 3] = ["intel", "decoy", "lockbox"];
/// NEP-145 minimum. Max equals min, so a deposit above this is refunded.
const STORAGE_MIN: u128 = 1_250_000_000_000_000_000_000;

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct Purse {
    account: AccountId,
    spent: u128,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct StoredAction {
    player: AccountId,
    round: u32,
    kind: ActionKind,
    target: Option<AccountId>,
    stake: u128,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct StoredBid {
    token_id: String,
    bidder: AccountId,
    amount: u128,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct Heist {
    creator: AccountId,
    rules: GameRules,
    game_md_hash: String,
    started_at_ns: u64,
    players: Vec<AccountId>,
    delisted: bool,
    resolved: bool,
    actions: Vec<StoredAction>,
    bids: Vec<StoredBid>,
    purses: Vec<Purse>,
}

struct CheckPlan {
    already: u128,
    pending: PendingSpend,
}

struct MtCall {
    sender_id: AccountId,
    receiver_id: AccountId,
    previous_owner_ids: Vec<AccountId>,
    token_ids: Vec<String>,
    amounts: Vec<U128>,
    msg: String,
    restore: Option<Vec<SavedApproval>>,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct PendingSpend {
    game_id: String,
    agent: AccountId,
    kind: String,
    amount: u128,
    round: u32,
    token_id: String,
    previous_bidder: Option<AccountId>,
    previous_amount: u128,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct HeistView {
    pub id: String,
    pub creator: AccountId,
    pub rules: GameRules,
    pub game_md_hash: String,
    pub players: Vec<AccountId>,
    pub delisted: bool,
    pub resolved: bool,
    pub started_at_ns: u64,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct Approval {
    amount: u128,
    approval_id: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SavedApproval {
    pub token_id: String,
    pub owner_id: AccountId,
    pub grantee_id: AccountId,
    pub approval_id: u64,
    pub amount: U128,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageBalance {
    pub total: U128,
    pub available: U128,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageBalanceBounds {
    pub min: U128,
    pub max: Option<U128>,
}

#[ext_contract(ext_registry)]
pub trait RegistrySpend {
    fn check_spend(&mut self, agent: AccountId, amount: U128, already_spent: U128) -> bool;
}

#[ext_contract(ext_self)]
pub trait HeistCallback {
    fn on_spend_checked(&mut self, pending_id: u64);
}

#[ext_contract(ext_receiver)]
pub trait MtReceiver {
    fn mt_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_ids: Vec<AccountId>,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        msg: String,
    ) -> Vec<U128>;
}

#[ext_contract(ext_self_mt)]
pub trait HeistMtResolve {
    fn mt_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        previous_owner_ids: Vec<AccountId>,
        restore: Option<Vec<SavedApproval>>,
    ) -> Vec<U128>;
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct HeistGame {
    registry: AccountId,
    factory: AccountId,
    registry_caps: PlatformCaps,
    games: LookupMap<String, Heist>,
    balances: LookupMap<String, u128>,
    supplies: LookupMap<String, u128>,
    /// Written only by the registry. Missing means "not registered".
    mirror: LookupMap<AccountId, bool>,
    next_pending: u64,
    pending: LookupMap<u64, PendingSpend>,
    approvals: LookupMap<String, Approval>,
    approval_seq: LookupMap<String, u64>,
    storage: LookupMap<AccountId, u128>,
    holdings: LookupMap<AccountId, Vec<String>>,
    grantees: LookupMap<String, Vec<AccountId>>,
}

#[near]
impl HeistGame {
    #[init]
    pub fn new(registry: AccountId, factory: AccountId) -> Self {
        Self {
            registry,
            factory,
            registry_caps: default_platform_caps(),
            games: LookupMap::new(b"g"),
            balances: LookupMap::new(b"b"),
            supplies: LookupMap::new(b"s"),
            mirror: LookupMap::new(b"m"),
            next_pending: 1,
            pending: LookupMap::new(b"p"),
            approvals: LookupMap::new(b"v"),
            approval_seq: LookupMap::new(b"n"),
            storage: LookupMap::new(b"t"),
            holdings: LookupMap::new(b"h"),
            grantees: LookupMap::new(b"r"),
        }
    }

    pub fn apply_platform_caps(&mut self, caps: PlatformCaps) {
        require!(
            env::predecessor_account_id() == self.registry,
            "game cannot raise platform caps"
        );
        fe2vi_core::validate_platform_caps(&caps, &fe2vi_core::absolute_ceiling())
            .unwrap_or_else(|err| env::panic_str(&err.to_string()));
        self.registry_caps = caps;
    }

    /// Registry is the only writer. `revoked == true` blocks later acts.
    pub fn import_agent(&mut self, agent: AccountId, revoked: bool) {
        require!(
            env::predecessor_account_id() == self.registry,
            "only registry can import agents"
        );
        self.mirror.insert(agent, revoked);
    }

    #[payable]
    pub fn init_game(
        &mut self,
        game_id: String,
        rules: GameRules,
        creator: AccountId,
        game_md_hash: String,
    ) {
        require!(
            env::predecessor_account_id() == self.factory,
            "only factory can init a game"
        );
        require!(!game_id.is_empty(), "game id required");
        require!(self.games.get(&game_id).is_none(), "game exists");
        validate_rules(&rules, &self.registry_caps)
            .unwrap_or_else(|err| env::panic_str(&err.to_string()));
        require!(
            game_md_hash == pin_hash(&rules),
            "game_md_hash does not match the rules"
        );
        let deposit = env::attached_deposit().as_yoctonear();
        require!(deposit == rules.pot_yocto, "pot deposit mismatch");

        let escrow = env::current_account_id();
        self.mark_registered(&escrow);
        let pot = pot_token(&game_id);
        self.mint(&escrow, &pot, rules.pot_yocto);
        for item in ITEMS {
            self.mint(&escrow, &item_token(&game_id, item), 1);
        }
        self.games.insert(
            game_id.clone(),
            Heist {
                creator,
                rules,
                game_md_hash,
                started_at_ns: env::block_timestamp(),
                players: Vec::new(),
                delisted: false,
                resolved: false,
                actions: Vec::new(),
                bids: Vec::new(),
                purses: Vec::new(),
            },
        );
        env::log_str(&event_json(
            "heist_opened",
            near_sdk::serde_json::json!({ "id": game_id }),
        ));
    }

    pub fn join(&mut self, game_id: String) {
        let agent = env::predecessor_account_id();
        self.require_playable(&agent);
        let mut game = self.take_game(&game_id);
        require!(!game.delisted && !game.resolved, "game is not open");
        require!(game.actions.is_empty(), "join closed");
        require!(
            round_index(
                game.started_at_ns,
                env::block_timestamp(),
                game.rules.duration_seconds,
                game.rules.round_count
            ) == Some(0),
            "join closed"
        );
        require!(
            game.players.len() < game.rules.max_players as usize,
            "game is full"
        );
        require!(!game.players.contains(&agent), "already joined");
        game.players.push(agent.clone());
        self.games.insert(game_id.clone(), game);
        env::log_str(&event_json(
            "player_joined",
            near_sdk::serde_json::json!({ "id": game_id, "agent": agent }),
        ));
    }

    /// One structured action per round. `stake_yocto` must equal the action's price.
    pub fn act(
        &mut self,
        game_id: String,
        action: ActionKind,
        target: Option<AccountId>,
        stake_yocto: U128,
    ) -> Promise {
        let agent = env::predecessor_account_id();
        let stake = stake_yocto.0;
        let (already, round) = self.commit_act(&game_id, &agent, action, target, stake);
        self.schedule_check(CheckPlan {
            already,
            pending: PendingSpend {
                game_id,
                agent,
                kind: "act".to_string(),
                amount: stake,
                round,
                token_id: String::new(),
                previous_bidder: None,
                previous_amount: 0,
            },
        })
    }

    pub fn bid(&mut self, game_id: String, token_id: String, amount_yocto: U128) -> Promise {
        let agent = env::predecessor_account_id();
        let amount = amount_yocto.0;
        let (already, previous_bidder, previous_amount) =
            self.commit_bid(&game_id, &agent, &token_id, amount);
        self.schedule_check(CheckPlan {
            already,
            pending: PendingSpend {
                game_id,
                agent,
                kind: "bid".to_string(),
                amount,
                round: 0,
                token_id,
                previous_bidder,
                previous_amount,
            },
        })
    }

    /// Moves an item the caller already holds. Pot tokens stay escrowed.
    pub fn transfer_item(&mut self, game_id: String, token_id: String, receiver_id: AccountId) {
        let sender = env::predecessor_account_id();
        self.require_playable(&sender);
        require!(is_item(&game_id, &token_id), "not an item in this game");
        self.transfer_internal(&sender, &receiver_id, &token_id, 1);
        env::log_str(&event_json(
            "item_transferred",
            near_sdk::serde_json::json!({
                "game_id": game_id,
                "token_id": token_id,
                "sender": sender,
                "receiver": receiver_id,
            }),
        ));
    }

    pub fn mark_delisted(&mut self, game_id: String) {
        require!(
            env::predecessor_account_id() == self.factory,
            "only factory can delist"
        );
        let mut game = self.take_game(&game_id);
        game.delisted = true;
        self.games.insert(game_id, game);
    }

    /// Permissionless settlement. Delist does not let the moderator pick a payee:
    /// a delisted game splits the pot across joined players, or returns it to
    /// the creator when nobody joined. A finished game splits by score.
    pub fn resolve(&mut self, game_id: String) {
        let mut game = self.take_game(&game_id);
        require!(!game.resolved, "already resolved");
        let now = env::block_timestamp();
        let duration_ns = game
            .rules
            .duration_seconds
            .checked_mul(1_000_000_000)
            .unwrap_or_else(|| env::panic_str("duration overflow"));
        if !game.delisted {
            require!(
                now >= game.started_at_ns.saturating_add(duration_ns),
                "game still running"
            );
        }
        game.resolved = true;
        let payouts = payouts_for(&game);
        let bids = game.bids.clone();
        self.games.insert(game_id.clone(), game);

        let escrow = env::current_account_id();
        let pot = pot_token(&game_id);
        for (winner, amount) in &payouts {
            if *amount == 0 {
                continue;
            }
            self.transfer_internal(&escrow, winner, &pot, *amount);
            Promise::new(winner.clone()).transfer(NearToken::from_yoctonear(*amount));
        }
        for bid in bids {
            self.transfer_internal(&escrow, &bid.bidder, &bid.token_id, 1);
        }
        env::log_str(&event_json(
            "game_resolved",
            near_sdk::serde_json::json!({
                "id": game_id,
                "payouts": payouts.iter().map(|(account, amount)| near_sdk::serde_json::json!({
                    "account": account,
                    "amount": amount.to_string(),
                })).collect::<Vec<_>>(),
            }),
        ));
    }

    #[private]
    pub fn on_spend_checked(&mut self, pending_id: u64) {
        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                self.pending.remove(&pending_id);
            }
            PromiseResult::Failed => self.revert_spend(pending_id),
        }
    }

    #[payable]
    pub fn mt_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        approval: Option<(AccountId, u64)>,
        memo: Option<String>,
    ) {
        let _ = memo;
        require_one_yocto();
        let sender = env::predecessor_account_id();
        let (owner, _saved) = self.consume_approval(&sender, &token_id, amount.0, approval);
        self.transfer_internal(&owner, &receiver_id, &token_id, amount.0);
    }

    #[payable]
    pub fn mt_batch_transfer(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        approvals: Option<Vec<Option<(AccountId, u64)>>>,
        memo: Option<String>,
    ) {
        let _ = memo;
        require_one_yocto();
        require!(token_ids.len() == amounts.len(), "length mismatch");
        let sender = env::predecessor_account_id();
        for (index, (token_id, amount)) in token_ids.iter().zip(amounts.iter()).enumerate() {
            let approval = approvals
                .as_ref()
                .and_then(|entries| entries.get(index).cloned().flatten());
            let (owner, _saved) = self.consume_approval(&sender, token_id, amount.0, approval);
            self.transfer_internal(&owner, &receiver_id, token_id, amount.0);
        }
    }

    #[payable]
    pub fn mt_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        approval: Option<(AccountId, u64)>,
        memo: Option<String>,
        msg: String,
    ) -> Promise {
        let _ = memo;
        require_one_yocto();
        let sender = env::predecessor_account_id();
        let (owner, saved) = self.consume_approval(&sender, &token_id, amount.0, approval);
        self.transfer_internal(&owner, &receiver_id, &token_id, amount.0);
        self.schedule_mt_resolve(MtCall {
            sender_id: sender,
            receiver_id,
            previous_owner_ids: vec![owner],
            token_ids: vec![token_id],
            amounts: vec![amount],
            msg,
            restore: saved.map(|entry| vec![entry]),
        })
    }

    #[payable]
    pub fn mt_batch_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        approvals: Option<Vec<Option<(AccountId, u64)>>>,
        memo: Option<String>,
        msg: String,
    ) -> Promise {
        let _ = memo;
        require_one_yocto();
        require!(token_ids.len() == amounts.len(), "length mismatch");
        let sender = env::predecessor_account_id();
        let mut owners = Vec::with_capacity(token_ids.len());
        let mut restore = Vec::new();
        for (index, (token_id, amount)) in token_ids.iter().zip(amounts.iter()).enumerate() {
            let approval = approvals
                .as_ref()
                .and_then(|entries| entries.get(index).cloned().flatten());
            let (owner, saved) = self.consume_approval(&sender, token_id, amount.0, approval);
            self.transfer_internal(&owner, &receiver_id, token_id, amount.0);
            if let Some(saved) = saved {
                restore.push(saved);
            }
            owners.push(owner);
        }
        self.schedule_mt_resolve(MtCall {
            sender_id: sender,
            receiver_id,
            previous_owner_ids: owners,
            token_ids,
            amounts,
            msg,
            restore: if restore.is_empty() {
                None
            } else {
                Some(restore)
            },
        })
    }

    #[private]
    pub fn mt_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        previous_owner_ids: Vec<AccountId>,
        restore: Option<Vec<SavedApproval>>,
    ) -> Vec<U128> {
        let _ = sender_id;
        require!(
            token_ids.len() == amounts.len() && token_ids.len() == previous_owner_ids.len(),
            "length mismatch"
        );
        match env::promise_result(0) {
            PromiseResult::Failed => {
                self.refund_up_to(&receiver_id, &token_ids, &amounts, &previous_owner_ids);
                self.restore_approvals(restore);
                vec![U128(0); token_ids.len()]
            }
            PromiseResult::Successful(bytes) => {
                let unused: Vec<U128> =
                    near_sdk::serde_json::from_slice(&bytes).unwrap_or_else(|_| amounts.clone());
                require!(unused.len() == amounts.len(), "unused length");
                let mut used = Vec::with_capacity(amounts.len());
                for index in 0..amounts.len() {
                    require!(unused[index].0 <= amounts[index].0, "unused exceeds sent");
                    if unused[index].0 > 0 {
                        self.refund_up_to(
                            &receiver_id,
                            &token_ids[index..index + 1],
                            &unused[index..index + 1],
                            &previous_owner_ids[index..index + 1],
                        );
                    }
                    used.push(U128(amounts[index].0 - unused[index].0));
                }
                used
            }
        }
    }

    #[payable]
    pub fn mt_approve(
        &mut self,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        grantee_id: AccountId,
    ) {
        require_one_yocto();
        require!(token_ids.len() == amounts.len(), "length mismatch");
        let owner = env::predecessor_account_id();
        require!(owner != grantee_id, "cannot approve yourself");
        self.require_registered(&owner);
        for (token_id, amount) in token_ids.iter().zip(amounts.iter()) {
            require!(amount.0 > 0, "amount is zero");
            require!(
                self.balance_of(&owner, token_id) >= amount.0,
                "insufficient token balance"
            );
            let seq_key = format!("{token_id}|{owner}");
            let next = self.approval_seq.get(&seq_key).copied().unwrap_or(1);
            self.approval_seq.insert(seq_key, next + 1);
            self.approvals.insert(
                approval_key(token_id, &owner, &grantee_id),
                Approval {
                    amount: amount.0,
                    approval_id: next,
                },
            );
            let grant_key = format!("{token_id}|{owner}");
            let mut list = self.grantees.get(&grant_key).cloned().unwrap_or_default();
            if !list.contains(&grantee_id) {
                list.push(grantee_id.clone());
                self.grantees.insert(grant_key, list);
            }
        }
    }

    #[payable]
    pub fn mt_revoke(&mut self, token_ids: Vec<String>, grantee_id: AccountId) {
        require_one_yocto();
        let owner = env::predecessor_account_id();
        for token_id in &token_ids {
            self.approvals
                .remove(&approval_key(token_id, &owner, &grantee_id));
        }
    }

    #[payable]
    pub fn mt_revoke_all(&mut self, token_id: String) {
        require_one_yocto();
        let owner = env::predecessor_account_id();
        let grant_key = format!("{token_id}|{owner}");
        let list = self.grantees.remove(&grant_key).unwrap_or_default();
        for grantee in list {
            self.approvals
                .remove(&approval_key(&token_id, &owner, &grantee));
        }
    }

    pub fn mt_is_approved(
        &self,
        token_id: String,
        owner_id: AccountId,
        grantee_id: AccountId,
        approval_id: u64,
        amount: U128,
    ) -> bool {
        self.approvals
            .get(&approval_key(&token_id, &owner_id, &grantee_id))
            .is_some_and(|stored| stored.approval_id == approval_id && stored.amount >= amount.0)
    }

    #[payable]
    pub fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
        let account = account_id.unwrap_or_else(env::predecessor_account_id);
        let predecessor = env::predecessor_account_id();
        let attached = env::attached_deposit().as_yoctonear();
        let already = self.storage.get(&account).copied().unwrap_or(0);
        let registration_only = registration_only.unwrap_or(false);
        if already >= STORAGE_MIN || (registration_only && already > 0) {
            if attached > 0 {
                Promise::new(predecessor).transfer(NearToken::from_yoctonear(attached));
            }
            return self.storage_balance(&account);
        }
        require!(attached >= STORAGE_MIN, "deposit below storage minimum");
        let refund = attached - STORAGE_MIN;
        self.storage.insert(account.clone(), STORAGE_MIN);
        if refund > 0 {
            Promise::new(predecessor).transfer(NearToken::from_yoctonear(refund));
        }
        self.storage_balance(&account)
    }

    #[payable]
    pub fn storage_withdraw(&mut self, amount: Option<U128>) -> StorageBalance {
        require_one_yocto();
        let account = env::predecessor_account_id();
        let total = self
            .storage
            .get(&account)
            .copied()
            .unwrap_or_else(|| env::panic_str("not registered"));
        let available = total.saturating_sub(STORAGE_MIN);
        let withdraw = amount.map(|value| value.0).unwrap_or(available);
        require!(withdraw <= available, "withdraw exceeds available");
        if withdraw > 0 {
            self.storage.insert(account.clone(), total - withdraw);
            Promise::new(account.clone()).transfer(NearToken::from_yoctonear(withdraw));
        }
        self.storage_balance(&account)
    }

    #[payable]
    pub fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        require_one_yocto();
        let account = env::predecessor_account_id();
        let total = self
            .storage
            .get(&account)
            .copied()
            .unwrap_or_else(|| env::panic_str("not registered"));
        let force = force.unwrap_or(false);
        let holdings = self.holdings.get(&account).cloned().unwrap_or_default();
        if !force {
            require!(
                holdings
                    .iter()
                    .all(|token| self.balance_of(&account, token) == 0),
                "still holds tokens"
            );
        } else {
            for token in &holdings {
                let balance = self.balance_of(&account, token);
                if balance > 0 {
                    self.balances.insert(balance_key(&account, token), 0);
                    let supply = self
                        .supplies
                        .get(token)
                        .copied()
                        .unwrap_or(0)
                        .saturating_sub(balance);
                    self.supplies.insert(token.clone(), supply);
                }
            }
            self.holdings.remove(&account);
        }
        self.storage.remove(&account);
        Promise::new(account).transfer(NearToken::from_yoctonear(total));
        true
    }

    pub fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        StorageBalanceBounds {
            min: U128(STORAGE_MIN),
            max: Some(U128(STORAGE_MIN)),
        }
    }

    pub fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        if self.storage.get(&account_id).is_some() {
            Some(self.storage_balance(&account_id))
        } else {
            None
        }
    }

    pub fn mt_balance_of(&self, account_id: AccountId, token_id: String) -> U128 {
        U128(self.balance_of(&account_id, &token_id))
    }

    pub fn mt_batch_balance_of(&self, account_id: AccountId, token_ids: Vec<String>) -> Vec<U128> {
        token_ids
            .iter()
            .map(|token_id| U128(self.balance_of(&account_id, token_id)))
            .collect()
    }

    pub fn mt_supply(&self, token_id: String) -> Option<U128> {
        self.supplies.get(&token_id).copied().map(U128)
    }

    pub fn mt_batch_supply(&self, token_ids: Vec<String>) -> Vec<Option<U128>> {
        token_ids
            .iter()
            .map(|token_id| self.supplies.get(token_id).copied().map(U128))
            .collect()
    }

    pub fn get_heist(&self, game_id: String) -> Option<HeistView> {
        self.games.get(&game_id).map(|game| HeistView {
            id: game_id,
            creator: game.creator.clone(),
            rules: game.rules.clone(),
            game_md_hash: game.game_md_hash.clone(),
            players: game.players.clone(),
            delisted: game.delisted,
            resolved: game.resolved,
            started_at_ns: game.started_at_ns,
        })
    }

    pub fn get_registry_caps(&self) -> PlatformCaps {
        self.registry_caps.clone()
    }

    pub fn action_count(&self, game_id: String) -> u32 {
        self.games
            .get(&game_id)
            .map(|game| u32::try_from(game.actions.len()).unwrap_or(u32::MAX))
            .unwrap_or(0)
    }
}

impl HeistGame {
    fn take_game(&self, game_id: &str) -> Heist {
        self.games
            .get(game_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("game not found"))
    }

    fn require_playable(&self, agent: &AccountId) {
        match self.mirror.get(agent) {
            None => env::panic_str("agent is not registered"),
            Some(true) => env::panic_str("revoked"),
            Some(false) => {}
        }
    }

    fn commit_act(
        &mut self,
        game_id: &str,
        agent: &AccountId,
        action: ActionKind,
        target: Option<AccountId>,
        stake: u128,
    ) -> (u128, u32) {
        self.require_playable(agent);
        let mut game = self.take_game(game_id);
        require!(!game.delisted && !game.resolved, "game is not open");
        // The scoped key cannot call `join`. The first act in round 0 enters.
        if !game.players.contains(agent) {
            require!(game.actions.is_empty(), "join closed");
            require!(
                round_index(
                    game.started_at_ns,
                    env::block_timestamp(),
                    game.rules.duration_seconds,
                    game.rules.round_count,
                ) == Some(0),
                "join closed"
            );
            require!(
                game.players.len() < game.rules.max_players as usize,
                "game is full"
            );
            game.players.push(agent.clone());
        }
        let round = round_index(
            game.started_at_ns,
            env::block_timestamp(),
            game.rules.duration_seconds,
            game.rules.round_count,
        )
        .unwrap_or_else(|| env::panic_str("game is not in a round"));
        require!(
            !game
                .actions
                .iter()
                .any(|existing| &existing.player == agent && existing.round == round),
            "already acted this round"
        );
        let trading = game.rules.template_id == TEMPLATE_CUP;
        match action {
            ActionKind::Long | ActionKind::Short => {
                require!(trading, "not a heist action");
                require!(target.is_none(), "this action does not take a target");
            }
            ActionKind::Sabotage => {
                require!(!trading, "not a trading cup action");
                let target_account = target
                    .clone()
                    .unwrap_or_else(|| env::panic_str("sabotage needs a target"));
                require!(
                    game.players.contains(&target_account),
                    "target is not a player"
                );
                require!(&target_account != agent, "cannot target yourself");
            }
            ActionKind::Scout | ActionKind::Steal | ActionKind::Hide => {
                require!(!trading, "not a trading cup action");
                require!(target.is_none(), "this action does not take a target");
            }
        }
        let price = action_price(action, game.rules.per_action_cap_yocto)
            .unwrap_or_else(|err| env::panic_str(err));
        let already = committed(&game, agent);
        self.charge(&game, agent, stake, already);
        require!(stake == price, "stake must equal the action price");
        add_spent(&mut game, agent, stake);
        game.actions.push(StoredAction {
            player: agent.clone(),
            round,
            kind: action,
            target,
            stake,
        });
        self.games.insert(game_id.to_string(), game);
        env::log_str(&event_json(
            "action_taken",
            near_sdk::serde_json::json!({
                "id": game_id,
                "agent": agent,
                "round": round,
                "stake": stake.to_string(),
            }),
        ));
        (already, round)
    }

    fn commit_bid(
        &mut self,
        game_id: &str,
        agent: &AccountId,
        token_id: &str,
        amount: u128,
    ) -> (u128, Option<AccountId>, u128) {
        self.require_playable(agent);
        let mut game = self.take_game(game_id);
        require!(!game.delisted && !game.resolved, "game is not open");
        require!(game.players.contains(agent), "not joined");
        require!(is_item(game_id, token_id), "not an item in this game");
        let (previous_bidder, previous_amount) = game
            .bids
            .iter()
            .find(|bid| bid.token_id == token_id)
            .map(|bid| (bid.bidder.clone(), bid.amount))
            .unzip_bid();
        require!(amount > previous_amount, "bid too low");
        let own_previous = if previous_bidder.as_ref() == Some(agent) {
            previous_amount
        } else {
            0
        };
        let already = committed(&game, agent).saturating_sub(own_previous);
        self.charge(&game, agent, amount, already);
        game.bids.retain(|bid| bid.token_id != token_id);
        game.bids.push(StoredBid {
            token_id: token_id.to_string(),
            bidder: agent.clone(),
            amount,
        });
        self.games.insert(game_id.to_string(), game);
        env::log_str(&event_json(
            "bid_placed",
            near_sdk::serde_json::json!({
                "id": game_id,
                "agent": agent,
                "token_id": token_id,
                "amount": amount.to_string(),
            }),
        ));
        (already, previous_bidder, previous_amount)
    }

    fn mark_registered(&mut self, account: &AccountId) {
        if self.storage.get(account).is_none() {
            self.storage.insert(account.clone(), STORAGE_MIN);
        }
    }

    fn require_registered(&self, account: &AccountId) {
        if account == &env::current_account_id() {
            return;
        }
        require!(
            self.storage.get(account).is_some(),
            "account is not registered"
        );
    }

    fn storage_balance(&self, account: &AccountId) -> StorageBalance {
        let total = self.storage.get(account).copied().unwrap_or(0);
        StorageBalance {
            total: U128(total),
            available: U128(total.saturating_sub(STORAGE_MIN)),
        }
    }

    fn note_holding(&mut self, account: &AccountId, token_id: &str) {
        let mut list = self.holdings.get(account).cloned().unwrap_or_default();
        if !list.iter().any(|id| id == token_id) {
            list.push(token_id.to_string());
            self.holdings.insert(account.clone(), list);
        }
    }

    fn consume_approval(
        &mut self,
        operator: &AccountId,
        token_id: &str,
        amount: u128,
        approval: Option<(AccountId, u64)>,
    ) -> (AccountId, Option<SavedApproval>) {
        if let Some((owner, approval_id)) = approval {
            let key = approval_key(token_id, &owner, operator);
            let stored = self
                .approvals
                .get(&key)
                .cloned()
                .unwrap_or_else(|| env::panic_str("not approved"));
            require!(stored.approval_id == approval_id, "approval id mismatch");
            require!(stored.amount >= amount, "approved amount exceeded");
            self.approvals.remove(&key);
            (
                owner.clone(),
                Some(SavedApproval {
                    token_id: token_id.to_string(),
                    owner_id: owner,
                    grantee_id: operator.clone(),
                    approval_id,
                    amount: U128(stored.amount),
                }),
            )
        } else {
            (operator.clone(), None)
        }
    }

    fn restore_approvals(&mut self, restore: Option<Vec<SavedApproval>>) {
        let Some(entries) = restore else {
            return;
        };
        for entry in entries {
            let key = approval_key(&entry.token_id, &entry.owner_id, &entry.grantee_id);
            if self.approvals.get(&key).is_none() {
                self.approvals.insert(
                    key,
                    Approval {
                        amount: entry.amount.0,
                        approval_id: entry.approval_id,
                    },
                );
            }
        }
    }

    fn refund_up_to(
        &mut self,
        from: &AccountId,
        token_ids: &[String],
        amounts: &[U128],
        owners: &[AccountId],
    ) {
        for index in 0..token_ids.len() {
            let have = self.balance_of(from, &token_ids[index]);
            let amount = have.min(amounts[index].0);
            if amount > 0 {
                self.transfer_internal(from, &owners[index], &token_ids[index], amount);
            }
        }
    }

    fn schedule_mt_resolve(&self, call: MtCall) -> Promise {
        ext_receiver::ext(call.receiver_id.clone())
            .with_static_gas(Gas::from_tgas(35))
            .mt_on_transfer(
                call.sender_id.clone(),
                call.previous_owner_ids.clone(),
                call.token_ids.clone(),
                call.amounts.clone(),
                call.msg,
            )
            .then(
                ext_self_mt::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(40))
                    .mt_resolve_transfer(
                        call.sender_id,
                        call.receiver_id,
                        call.token_ids,
                        call.amounts,
                        call.previous_owner_ids,
                        call.restore,
                    ),
            )
    }

    fn charge(&self, game: &Heist, _agent: &AccountId, amount: u128, already: u128) {
        authorize_spend(SpendRequest {
            amount,
            already_spent: already,
            per_action_cap: game.rules.per_action_cap_yocto,
            per_agent_cap: game.rules.per_agent_cap_yocto,
            platform_per_action: self.registry_caps.max_per_action_yocto,
            platform_per_agent: self.registry_caps.max_per_agent_yocto,
            revoked: false,
        })
        .unwrap_or_else(|err| env::panic_str(&err.to_string()));
    }

    fn schedule_check(&mut self, plan: CheckPlan) -> Promise {
        let pending_id = self.next_pending;
        self.next_pending += 1;
        let agent = plan.pending.agent.clone();
        let amount = plan.pending.amount;
        let already = plan.already;
        self.pending.insert(pending_id, plan.pending);
        ext_registry::ext(self.registry.clone())
            .with_static_gas(Gas::from_tgas(20))
            .check_spend(agent, U128(amount), U128(already))
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(30))
                    .on_spend_checked(pending_id),
            )
    }

    fn revert_spend(&mut self, pending_id: u64) {
        let Some(pending) = self.pending.remove(&pending_id) else {
            return;
        };
        let Some(mut game) = self.games.get(&pending.game_id).cloned() else {
            return;
        };
        if pending.kind == "act" {
            if let Some(index) = game.actions.iter().rposition(|action| {
                action.player == pending.agent
                    && action.round == pending.round
                    && action.stake == pending.amount
            }) {
                game.actions.remove(index);
            }
            if let Some(purse) = game
                .purses
                .iter_mut()
                .find(|purse| purse.account == pending.agent)
            {
                purse.spent = purse.spent.saturating_sub(pending.amount);
            }
        } else if pending.kind == "bid" {
            game.bids.retain(|bid| {
                !(bid.token_id == pending.token_id
                    && bid.bidder == pending.agent
                    && bid.amount == pending.amount)
            });
            if pending.previous_amount > 0 {
                if let Some(bidder) = pending.previous_bidder.clone() {
                    game.bids.push(StoredBid {
                        token_id: pending.token_id.clone(),
                        bidder,
                        amount: pending.previous_amount,
                    });
                }
            }
        }
        let game_id = pending.game_id.clone();
        self.games.insert(game_id, game);
        env::log_str(&event_json(
            "spend_reverted",
            near_sdk::serde_json::json!({ "pending_id": pending_id }),
        ));
    }

    fn balance_of(&self, account: &AccountId, token_id: &str) -> u128 {
        self.balances
            .get(&balance_key(account, token_id))
            .copied()
            .unwrap_or(0)
    }

    fn mint(&mut self, owner: &AccountId, token_id: &str, amount: u128) {
        let next = self.balance_of(owner, token_id).saturating_add(amount);
        self.balances.insert(balance_key(owner, token_id), next);
        let supply = self
            .supplies
            .get(token_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(amount);
        self.supplies.insert(token_id.to_string(), supply);
        self.note_holding(owner, token_id);
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "nep245",
                "version": "1.0.0",
                "event": "mt_mint",
                "data": [{
                    "owner_id": owner,
                    "token_ids": [token_id],
                    "amounts": [amount.to_string()],
                }],
            })
        ));
    }

    fn transfer_internal(
        &mut self,
        sender: &AccountId,
        receiver: &AccountId,
        token_id: &str,
        amount: u128,
    ) {
        require!(amount > 0, "amount is zero");
        let contract = env::current_account_id();
        if receiver != &contract && self.storage.get(receiver).is_none() {
            if sender == &contract {
                self.mark_registered(receiver);
            } else {
                env::panic_str("receiver is not registered");
            }
        }
        if sender != &contract {
            self.require_registered(sender);
        }
        let current = self.balance_of(sender, token_id);
        require!(current >= amount, "insufficient token balance");
        self.balances
            .insert(balance_key(sender, token_id), current - amount);
        let received = self.balance_of(receiver, token_id).saturating_add(amount);
        self.balances
            .insert(balance_key(receiver, token_id), received);
        self.note_holding(receiver, token_id);
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "nep245",
                "version": "1.0.0",
                "event": "mt_transfer",
                "data": [{
                    "old_owner_id": sender,
                    "new_owner_id": receiver,
                    "token_ids": [token_id],
                    "amounts": [amount.to_string()],
                }],
            })
        ));
    }
}

fn pot_token(game_id: &str) -> String {
    format!("pot:{game_id}")
}

fn item_token(game_id: &str, item: &str) -> String {
    format!("{item}:{game_id}")
}

fn is_item(game_id: &str, token_id: &str) -> bool {
    ITEMS
        .iter()
        .any(|item| token_id == item_token(game_id, item))
}

fn require_one_yocto() {
    require!(
        env::attached_deposit() == NearToken::from_yoctonear(1),
        "requires attached deposit of exactly 1 yoctoNEAR"
    );
}

fn approval_key(token_id: &str, owner: &AccountId, grantee: &AccountId) -> String {
    format!("{token_id}|{owner}|{grantee}")
}

fn balance_key(account: &AccountId, token_id: &str) -> String {
    format!("{account}|{token_id}")
}

fn committed(game: &Heist, agent: &AccountId) -> u128 {
    let spent = game
        .purses
        .iter()
        .find(|purse| &purse.account == agent)
        .map(|purse| purse.spent)
        .unwrap_or(0);
    let bids = game
        .bids
        .iter()
        .filter(|bid| &bid.bidder == agent)
        .fold(0u128, |sum, bid| sum.saturating_add(bid.amount));
    spent.saturating_add(bids)
}

fn add_spent(game: &mut Heist, agent: &AccountId, amount: u128) {
    if let Some(purse) = game.purses.iter_mut().find(|purse| &purse.account == agent) {
        purse.spent = purse.spent.saturating_add(amount);
    } else {
        game.purses.push(Purse {
            account: agent.clone(),
            spent: amount,
        });
    }
}

trait UnzipBid {
    fn unzip_bid(self) -> (Option<AccountId>, u128);
}

impl UnzipBid for Option<(AccountId, u128)> {
    fn unzip_bid(self) -> (Option<AccountId>, u128) {
        match self {
            Some((bidder, amount)) => (Some(bidder), amount),
            None => (None, 0),
        }
    }
}

fn payouts_for(game: &Heist) -> Vec<(AccountId, u128)> {
    if game.delisted {
        if game.players.is_empty() {
            return vec![(game.creator.clone(), game.rules.pot_yocto)];
        }
        let weights: Vec<(String, u128)> = game
            .players
            .iter()
            .map(|player| (player.to_string(), 1))
            .collect();
        return accounts_from_shares(split_pot(game.rules.pot_yocto, &weights));
    }
    if game.rules.template_id == TEMPLATE_CUP {
        let mut totals: std::collections::BTreeMap<String, u128> =
            std::collections::BTreeMap::new();
        for action in &game.actions {
            let points = u128::from(action.kind == tape_winner(action.round));
            *totals.entry(action.player.to_string()).or_insert(0) += points;
        }
        if totals.values().all(|points| *points == 0) {
            return vec![(game.creator.clone(), game.rules.pot_yocto)];
        }
        let weights: Vec<(String, u128)> = totals.into_iter().collect();
        return accounts_from_shares(split_pot(game.rules.pot_yocto, &weights));
    }
    let mut by_round: std::collections::BTreeMap<u32, Vec<SubmittedAction>> =
        std::collections::BTreeMap::new();
    for action in &game.actions {
        by_round
            .entry(action.round)
            .or_default()
            .push(SubmittedAction {
                player: action.player.to_string(),
                kind: action.kind,
                target: action.target.as_ref().map(|account| account.to_string()),
            });
    }
    let mut totals: std::collections::BTreeMap<String, u128> = std::collections::BTreeMap::new();
    for actions in by_round.values() {
        for (player, points) in score_actions(actions) {
            *totals.entry(player).or_insert(0) += points;
        }
    }
    if totals.values().all(|points| *points == 0) {
        return vec![(game.creator.clone(), game.rules.pot_yocto)];
    }
    let weights: Vec<(String, u128)> = totals.into_iter().collect();
    accounts_from_shares(split_pot(game.rules.pot_yocto, &weights))
}

fn accounts_from_shares(shares: Vec<(String, u128)>) -> Vec<(AccountId, u128)> {
    shares
        .into_iter()
        .map(|(account, amount)| {
            let parsed = account
                .parse()
                .unwrap_or_else(|_| env::panic_str("payout account"));
            (parsed, amount)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2vi_core::{heist_night_example, pin_hash};
    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
    use near_sdk::testing_env;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn use_pred(predecessor: &str, deposit: u128, timestamp: u64) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("heist.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(predecessor.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(10_000))
            .prepaid_gas(Gas::from_tgas(300))
            .block_timestamp(timestamp);
        testing_env!(builder.build());
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

    fn boot() -> (HeistGame, String) {
        use_pred("deployer.fe2vi.testnet", 0, 1_000);
        let mut game = HeistGame::new(
            "registry.fe2vi.testnet".parse().unwrap(),
            "factory.fe2vi.testnet".parse().unwrap(),
        );
        let rules = heist_night_example();
        use_pred("factory.fe2vi.testnet", rules.pot_yocto, 1_000);
        let id = "g1".to_string();
        game.init_game(
            id.clone(),
            rules.clone(),
            "creator.fe2vi.testnet".parse().unwrap(),
            pin_hash(&rules),
        );
        use_pred("registry.fe2vi.testnet", 0, 1_000);
        game.import_agent("alice.fe2vi.testnet".parse().unwrap(), false);
        game.import_agent("bob.fe2vi.testnet".parse().unwrap(), false);
        (game, id)
    }

    #[test]
    fn spend_over_cap_rejected() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let rules = heist_night_example();
        let err = catch_unwind(AssertUnwindSafe(|| {
            let _promise = game.bid(
                id.clone(),
                item_token(&id, "intel"),
                U128(rules.per_action_cap_yocto + 1),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("over cap"), "expected over cap");

        let err = catch_unwind(AssertUnwindSafe(|| {
            let _promise = game.act(
                id.clone(),
                ActionKind::Scout,
                None,
                U128(rules.per_agent_cap_yocto + 1),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("over cap"), "expected over cap");

        let price = action_price(ActionKind::Scout, rules.per_action_cap_yocto).unwrap();
        drop(game.act(id.clone(), ActionKind::Scout, None, U128(price)));
        let methods = receipt_method_names();
        assert!(
            methods.iter().any(|method| method == "check_spend"),
            "spend must ask the registry"
        );
    }

    #[test]
    fn revoked_agent_cannot_act() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        use_pred("registry.fe2vi.testnet", 0, 1_000);
        game.import_agent("alice.fe2vi.testnet".parse().unwrap(), true);
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        let price = action_price(
            ActionKind::Scout,
            heist_night_example().per_action_cap_yocto,
        )
        .unwrap();
        let err = catch_unwind(AssertUnwindSafe(|| {
            let _promise = game.act(id.clone(), ActionKind::Scout, None, U128(price));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("revoked"));
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.transfer_item(
                id,
                item_token("g1", "intel"),
                "bob.fe2vi.testnet".parse().unwrap(),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("revoked"));
    }

    #[test]
    fn game_cannot_raise_platform_caps() {
        let (mut game, _) = boot();
        let original = game.get_registry_caps();
        use_pred("factory.fe2vi.testnet", 0, 1_000);
        let mut raised = original.clone();
        raised.max_per_agent_yocto += 1;
        let err = catch_unwind(AssertUnwindSafe(|| game.apply_platform_caps(raised))).unwrap_err();
        assert!(panic_text(err).contains("game cannot raise platform caps"));
        assert_eq!(game.get_registry_caps(), original);

        let mut rules = heist_night_example();
        rules.pot_yocto = original.max_pot_yocto + 1;
        use_pred("factory.fe2vi.testnet", rules.pot_yocto, 1_000);
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.init_game(
                "g2".into(),
                rules.clone(),
                "creator.fe2vi.testnet".parse().unwrap(),
                pin_hash(&rules),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("pot"));
    }

    #[test]
    fn transfer_item_cannot_move_the_pot() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.transfer_item(id, pot_token("g1"), "bob.fe2vi.testnet".parse().unwrap());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("not an item"));
    }

    #[test]
    fn resolve_pays_players_not_a_moderator_chosen_address() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let rules = heist_night_example();
        let price = action_price(ActionKind::Steal, rules.per_action_cap_yocto).unwrap();
        drop(game.act(id.clone(), ActionKind::Steal, None, U128(price)));
        let end = 1_000 + rules.duration_seconds * 1_000_000_000;
        use_pred("anyone.fe2vi.testnet", 0, end);
        game.resolve(id.clone());
        let alice: AccountId = "alice.fe2vi.testnet".parse().unwrap();
        let escrow: AccountId = "heist.fe2vi.testnet".parse().unwrap();
        assert_eq!(
            game.mt_balance_of(alice.clone(), pot_token(&id)).0,
            rules.pot_yocto
        );
        assert_eq!(game.mt_balance_of(escrow, pot_token(&id)).0, 0);
        assert_eq!(game.mt_balance_of(alice, item_token(&id, "intel")).0, 0);
    }

    #[test]
    fn cross_contract_failure_reverts_the_spend() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let price = action_price(
            ActionKind::Scout,
            heist_night_example().per_action_cap_yocto,
        )
        .unwrap();
        drop(game.act(id.clone(), ActionKind::Scout, None, U128(price)));
        assert_eq!(game.action_count(id.clone()), 1);
        use_callback(vec![PromiseResult::Failed]);
        game.on_spend_checked(1);
        assert_eq!(game.action_count(id), 0);
    }

    #[test]
    fn cross_contract_success_keeps_the_spend() {
        let (mut game, id) = boot();
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let price = action_price(
            ActionKind::Scout,
            heist_night_example().per_action_cap_yocto,
        )
        .unwrap();
        drop(game.act(id.clone(), ActionKind::Scout, None, U128(price)));
        use_callback(vec![PromiseResult::Successful(b"true".to_vec())]);
        game.on_spend_checked(1);
        assert_eq!(game.action_count(id), 1);
    }

    #[test]
    fn storage_approvals_and_transfer_call_revert() {
        let (mut game, id) = boot();
        let alice: AccountId = "alice.fe2vi.testnet".parse().unwrap();
        let bob: AccountId = "bob.fe2vi.testnet".parse().unwrap();
        let carol: AccountId = "carol.fe2vi.testnet".parse().unwrap();
        let intel = item_token(&id, "intel");
        use_pred("alice.fe2vi.testnet", STORAGE_MIN, 1_000);
        let deposited = game.storage_deposit(None, Some(true));
        assert_eq!(deposited.total.0, STORAGE_MIN);
        assert_eq!(deposited.available.0, 0);
        use_pred("bob.fe2vi.testnet", STORAGE_MIN + 5, 1_000);
        game.storage_deposit(None, None);
        use_pred("carol.fe2vi.testnet", STORAGE_MIN, 1_000);
        game.storage_deposit(None, None);
        assert!(game
            .storage_balance_of("dave.fe2vi.testnet".parse().unwrap())
            .is_none());

        use_pred("heist.fe2vi.testnet", 1, 1_000);
        game.mt_transfer(alice.clone(), item_token(&id, "decoy"), U128(1), None, None);
        use_pred("alice.fe2vi.testnet", 1, 1_000);
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.mt_transfer(
                "dave.fe2vi.testnet".parse().unwrap(),
                item_token(&id, "decoy"),
                U128(1),
                None,
                None,
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("not registered"));

        use_pred("heist.fe2vi.testnet", 1, 1_000);
        game.mt_transfer(alice.clone(), intel.clone(), U128(1), None, None);
        use_pred("alice.fe2vi.testnet", 1, 1_000);
        game.mt_approve(vec![intel.clone()], vec![U128(1)], carol.clone());
        assert!(game.mt_is_approved(intel.clone(), alice.clone(), carol.clone(), 1, U128(1)));
        use_pred("carol.fe2vi.testnet", 1, 1_000);
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.mt_transfer(
                bob.clone(),
                intel.clone(),
                U128(1),
                Some((alice.clone(), 9)),
                None,
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("approval id mismatch"));
        use_pred("carol.fe2vi.testnet", 1, 1_000);
        game.mt_transfer(
            bob.clone(),
            intel.clone(),
            U128(1),
            Some((alice.clone(), 1)),
            None,
        );
        assert_eq!(game.mt_balance_of(bob.clone(), intel.clone()).0, 1);
        assert!(!game.mt_is_approved(intel.clone(), alice.clone(), carol.clone(), 1, U128(1)));

        use_pred("bob.fe2vi.testnet", 1, 1_000);
        drop(game.mt_transfer_call(
            carol.clone(),
            intel.clone(),
            U128(1),
            None,
            None,
            "play".into(),
        ));
        assert_eq!(game.mt_balance_of(carol.clone(), intel.clone()).0, 1);
        let methods = receipt_method_names();
        assert!(methods.iter().any(|method| method == "mt_on_transfer"));
        assert!(methods.iter().any(|method| method == "mt_resolve_transfer"));
        use_callback(vec![PromiseResult::Failed]);
        let used = game.mt_resolve_transfer(
            bob.clone(),
            carol.clone(),
            vec![intel.clone()],
            vec![U128(1)],
            vec![bob.clone()],
            None,
        );
        assert_eq!(used[0].0, 0);
        assert_eq!(game.mt_balance_of(bob.clone(), intel.clone()).0, 1);

        use_pred("bob.fe2vi.testnet", 1, 2_000);
        drop(game.mt_transfer_call(
            carol.clone(),
            intel.clone(),
            U128(1),
            None,
            None,
            "keep".into(),
        ));
        let unused = near_sdk::serde_json::to_vec(&vec![U128(1u128)]).unwrap();
        use_callback(vec![PromiseResult::Successful(unused)]);
        let used = game.mt_resolve_transfer(
            bob.clone(),
            carol.clone(),
            vec![intel.clone()],
            vec![U128(1)],
            vec![bob.clone()],
            None,
        );
        assert_eq!(used[0].0, 0);
        assert_eq!(game.mt_balance_of(bob.clone(), intel.clone()).0, 1);

        use_pred("alice.fe2vi.testnet", 1, 2_000);
        let withdrawn = game.storage_withdraw(None);
        assert_eq!(withdrawn.available.0, 0);
        let bounds = game.storage_balance_bounds();
        assert_eq!(bounds.min.0, STORAGE_MIN);
        assert_eq!(bounds.max, Some(U128(STORAGE_MIN)));
    }

    #[test]
    fn trading_cup_pays_the_public_tape() {
        use_pred("deployer.fe2vi.testnet", 0, 1_000);
        let mut game = HeistGame::new(
            "registry.fe2vi.testnet".parse().unwrap(),
            "factory.fe2vi.testnet".parse().unwrap(),
        );
        let rules = fe2vi_core::trading_cup_example();
        use_pred("factory.fe2vi.testnet", rules.pot_yocto, 1_000);
        let id = "cup".to_string();
        game.init_game(
            id.clone(),
            rules.clone(),
            "creator.fe2vi.testnet".parse().unwrap(),
            pin_hash(&rules),
        );
        use_pred("registry.fe2vi.testnet", 0, 1_000);
        game.import_agent("alice.fe2vi.testnet".parse().unwrap(), false);
        use_pred("alice.fe2vi.testnet", 0, 1_000);
        game.join(id.clone());
        let price = action_price(ActionKind::Long, rules.per_action_cap_yocto).unwrap();
        drop(game.act(id.clone(), ActionKind::Long, None, U128(price)));
        let err = catch_unwind(AssertUnwindSafe(|| {
            let _promise = game.act(id.clone(), ActionKind::Steal, None, U128(price));
        }))
        .unwrap_err();
        let text = panic_text(err);
        assert!(
            text.contains("already acted") || text.contains("not a trading"),
            "{text}"
        );
        let end = 1_000 + rules.duration_seconds * 1_000_000_000;
        use_pred("anyone.fe2vi.testnet", 0, end);
        game.resolve(id.clone());
        let alice: AccountId = "alice.fe2vi.testnet".parse().unwrap();
        assert_eq!(game.mt_balance_of(alice, pot_token(&id)).0, rules.pot_yocto);
    }

    fn use_callback(results: Vec<PromiseResult>) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("heist.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id("heist.fe2vi.testnet".parse().unwrap())
            .signer_account_id("alice.fe2vi.testnet".parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(0))
            .account_balance(NearToken::from_near(10_000))
            .prepaid_gas(Gas::from_tgas(300))
            .block_timestamp(2_000);
        testing_env!(
            builder.build(),
            near_sdk::test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            Default::default(),
            results,
        );
    }

    fn receipt_method_names() -> Vec<String> {
        let mut names = Vec::new();
        for receipt in get_created_receipts() {
            for action in receipt.actions {
                if let near_sdk::mock::MockAction::FunctionCallWeight { method_name, .. } = action {
                    if let Ok(name) = String::from_utf8(method_name) {
                        names.push(name);
                    }
                }
            }
        }
        names
    }
}

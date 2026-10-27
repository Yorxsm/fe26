//! Heist Night referee.
//!
//! One deployment hosts many games. The pot and the three items are NEP-245
//! tokens escrowed by this contract. NEAR deposited with the pot stays here
//! until `resolve` pushes it to winners. Before every act or bid, the contract
//! checks the registry's caps and revocation list. A game cannot write those caps.

use borsh::{BorshDeserialize, BorshSerialize};
use fe26_core::{
    action_price, authorize_spend, default_platform_caps, event_json, pin_hash, round_index,
    score_actions, split_pot, validate_rules, ActionKind, GameRules, PlatformCaps, SpendRequest,
    SubmittedAction,
};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::store::LookupMap;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
    PromiseResult,
};

const ITEMS: [&str; 3] = ["intel", "decoy", "lockbox"];

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

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MtApproval {
    pub account_id: AccountId,
    pub approval_id: Option<u64>,
}

#[ext_contract(ext_registry)]
pub trait RegistrySpend {
    fn check_spend(&mut self, agent: AccountId, amount: U128, already_spent: U128) -> bool;
}

#[ext_contract(ext_self)]
pub trait HeistCallback {
    fn on_spend_checked(&mut self, pending_id: u64);
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
        }
    }

    pub fn apply_platform_caps(&mut self, caps: PlatformCaps) {
        require!(
            env::predecessor_account_id() == self.registry,
            "game cannot raise platform caps"
        );
        fe26_core::validate_platform_caps(&caps, &fe26_core::absolute_ceiling())
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

    pub fn mt_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        amount: U128,
        approval: Option<MtApproval>,
        memo: Option<String>,
    ) {
        let _ = memo;
        require!(approval.is_none(), "approvals are not enabled");
        let sender = env::predecessor_account_id();
        self.transfer_internal(&sender, &receiver_id, &token_id, amount.0);
    }

    pub fn mt_batch_transfer(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<String>,
        amounts: Vec<U128>,
        approvals: Option<Vec<Option<MtApproval>>>,
        memo: Option<String>,
    ) {
        let _ = memo;
        require!(token_ids.len() == amounts.len(), "length mismatch");
        if let Some(approvals) = approvals {
            require!(
                approvals.iter().all(|entry| entry.is_none()),
                "approvals are not enabled"
            );
        }
        let sender = env::predecessor_account_id();
        for (token_id, amount) in token_ids.iter().zip(amounts.iter()) {
            self.transfer_internal(&sender, &receiver_id, token_id, amount.0);
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
        require!(game.players.contains(agent), "not joined");
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
        match action {
            ActionKind::Sabotage => {
                let target_account = target
                    .clone()
                    .unwrap_or_else(|| env::panic_str("sabotage needs a target"));
                require!(
                    game.players.contains(&target_account),
                    "target is not a player"
                );
                require!(&target_account != agent, "cannot target yourself");
            }
            _ => require!(target.is_none(), "this action does not take a target"),
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
        let current = self.balance_of(sender, token_id);
        require!(current >= amount, "insufficient token balance");
        self.balances
            .insert(balance_key(sender, token_id), current - amount);
        let received = self.balance_of(receiver, token_id).saturating_add(amount);
        self.balances
            .insert(balance_key(receiver, token_id), received);
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
    use fe26_core::{heist_night_example, pin_hash};
    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
    use near_sdk::testing_env;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn use_pred(predecessor: &str, deposit: u128, timestamp: u64) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("heist.fe26.testnet".parse().unwrap())
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
        use_pred("deployer.fe26.testnet", 0, 1_000);
        let mut game = HeistGame::new(
            "registry.fe26.testnet".parse().unwrap(),
            "factory.fe26.testnet".parse().unwrap(),
        );
        let rules = heist_night_example();
        use_pred("factory.fe26.testnet", rules.pot_yocto, 1_000);
        let id = "g1".to_string();
        game.init_game(
            id.clone(),
            rules.clone(),
            "creator.fe26.testnet".parse().unwrap(),
            pin_hash(&rules),
        );
        use_pred("registry.fe26.testnet", 0, 1_000);
        game.import_agent("alice.fe26.testnet".parse().unwrap(), false);
        game.import_agent("bob.fe26.testnet".parse().unwrap(), false);
        (game, id)
    }

    #[test]
    fn spend_over_cap_rejected() {
        let (mut game, id) = boot();
        use_pred("alice.fe26.testnet", 0, 1_000);
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
        use_pred("alice.fe26.testnet", 0, 1_000);
        game.join(id.clone());
        use_pred("registry.fe26.testnet", 0, 1_000);
        game.import_agent("alice.fe26.testnet".parse().unwrap(), true);
        use_pred("alice.fe26.testnet", 0, 1_000);
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
                "bob.fe26.testnet".parse().unwrap(),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("revoked"));
    }

    #[test]
    fn game_cannot_raise_platform_caps() {
        let (mut game, _) = boot();
        let original = game.get_registry_caps();
        use_pred("factory.fe26.testnet", 0, 1_000);
        let mut raised = original.clone();
        raised.max_per_agent_yocto += 1;
        let err = catch_unwind(AssertUnwindSafe(|| game.apply_platform_caps(raised))).unwrap_err();
        assert!(panic_text(err).contains("game cannot raise platform caps"));
        assert_eq!(game.get_registry_caps(), original);

        let mut rules = heist_night_example();
        rules.pot_yocto = original.max_pot_yocto + 1;
        use_pred("factory.fe26.testnet", rules.pot_yocto, 1_000);
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.init_game(
                "g2".into(),
                rules.clone(),
                "creator.fe26.testnet".parse().unwrap(),
                pin_hash(&rules),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("pot"));
    }

    #[test]
    fn transfer_item_cannot_move_the_pot() {
        let (mut game, id) = boot();
        use_pred("alice.fe26.testnet", 0, 1_000);
        game.join(id.clone());
        let err = catch_unwind(AssertUnwindSafe(|| {
            game.transfer_item(id, pot_token("g1"), "bob.fe26.testnet".parse().unwrap());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("not an item"));
    }

    #[test]
    fn resolve_pays_players_not_a_moderator_chosen_address() {
        let (mut game, id) = boot();
        use_pred("alice.fe26.testnet", 0, 1_000);
        game.join(id.clone());
        let rules = heist_night_example();
        let price = action_price(ActionKind::Steal, rules.per_action_cap_yocto).unwrap();
        drop(game.act(id.clone(), ActionKind::Steal, None, U128(price)));
        let end = 1_000 + rules.duration_seconds * 1_000_000_000;
        use_pred("anyone.fe26.testnet", 0, end);
        game.resolve(id.clone());
        let alice: AccountId = "alice.fe26.testnet".parse().unwrap();
        let escrow: AccountId = "heist.fe26.testnet".parse().unwrap();
        assert_eq!(
            game.mt_balance_of(alice.clone(), pot_token(&id)).0,
            rules.pot_yocto
        );
        assert_eq!(game.mt_balance_of(escrow, pot_token(&id)).0, 0);
        assert_eq!(game.mt_balance_of(alice, item_token(&id, "intel")).0, 0);
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

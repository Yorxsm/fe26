//! Tradable fe2VI career passports.
//!
//! A passport is an NEP-171 token that carries a career record and a binding
//! to one off-chain agent. Transfers clear the binding. The new owner binds it
//! to its own agent, inheriting the career record. The agent credential itself
//! never moves.

use std::collections::BTreeMap;

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::store::{LookupMap, LookupSet};
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
};

const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;
const MINT_DEPOSIT: u128 = ONE_NEAR / 50;
const CREATOR_BPS: u16 = 200;
const PLATFORM_BPS: u16 = 300;
const DEFAULT_BASE_URI: &str = "https://api.fe2vi.space/v1/passport/nft/";
const MAX_UPDATE_BATCH: usize = 50;
const MAX_MINT_BATCH: usize = 25;

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Career {
    pub xp: u64,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub matches: u32,
    pub heists: u32,
    pub seasons: u32,
    pub season_titles: u32,
    pub best_rank: u32,
    pub rank: u32,
    pub rank_epoch: u32,
}

impl Career {
    fn empty() -> Self {
        Self {
            xp: 0,
            wins: 0,
            losses: 0,
            draws: 0,
            matches: 0,
            heists: 0,
            seasons: 0,
            season_titles: 0,
            best_rank: 0,
            rank: 0,
            rank_epoch: 0,
        }
    }
}

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct CareerUpdate {
    pub seq: u64,
    pub xp: u64,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub matches: u32,
    pub heists: u32,
    pub seasons: u32,
    pub season_titles: u32,
    pub best_rank: u32,
    pub rank: u32,
    pub rank_epoch: u32,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct PassportToken {
    pub owner_id: AccountId,
    pub creator: AccountId,
    pub platform: AccountId,
    pub next_approval_id: u64,
    pub approvals: Vec<(AccountId, u64)>,
    pub bound_agent: Option<String>,
    pub last_agent: Option<String>,
    pub career: Career,
    pub last_sync_seq: u64,
    pub locked_until: u64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NftContractMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub base_uri: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<Base64VecU8>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub media: Option<String>,
    pub media_hash: Option<Base64VecU8>,
    pub copies: Option<u64>,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub starts_at: Option<String>,
    pub updated_at: Option<String>,
    pub extra: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<Base64VecU8>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct JsonToken {
    pub token_id: String,
    pub owner_id: AccountId,
    pub metadata: TokenMetadata,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Payout {
    pub payout: BTreeMap<AccountId, U128>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PassportView {
    pub token_id: String,
    pub owner_id: AccountId,
    pub creator: AccountId,
    pub platform: AccountId,
    pub bound_agent: Option<String>,
    pub career: Career,
    pub tier: String,
    pub last_sync_seq: u64,
    pub locked_until: u64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PlaybookHashView {
    pub token_id: String,
    pub version: u64,
    pub hash: String,
}

fn valid_playbook_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Passport {
    owner: AccountId,
    platform: AccountId,
    registry: AccountId,
    base_uri: String,
    tokens: LookupMap<String, PassportToken>,
    by_owner: LookupMap<AccountId, Vec<String>>,
    agent_to_token: LookupMap<String, String>,
    supply_ids: Vec<String>,
    recorders: LookupSet<AccountId>,
}

#[near]
impl Passport {
    #[init]
    pub fn new(owner: AccountId, platform: AccountId, registry: AccountId) -> Self {
        Self {
            owner,
            platform,
            registry,
            base_uri: DEFAULT_BASE_URI.into(),
            tokens: LookupMap::new(b"t"),
            by_owner: LookupMap::new(b"o"),
            agent_to_token: LookupMap::new(b"a"),
            supply_ids: Vec::new(),
            recorders: LookupSet::new(b"r"),
        }
    }

    pub fn nft_metadata(&self) -> NftContractMetadata {
        NftContractMetadata {
            spec: "nft-1.0.0".into(),
            name: "fe2VI Passports".into(),
            symbol: "PASS".into(),
            icon: None,
            base_uri: Some(self.base_uri.clone()),
            reference: None,
            reference_hash: None,
        }
    }

    pub fn mint_deposit(&self) -> U128 {
        U128(MINT_DEPOSIT)
    }

    pub fn creator_bps(&self) -> u16 {
        CREATOR_BPS
    }

    pub fn platform_bps(&self) -> u16 {
        PLATFORM_BPS
    }

    pub fn platform_account(&self) -> AccountId {
        self.platform.clone()
    }

    pub fn registry(&self) -> AccountId {
        self.registry.clone()
    }

    pub fn set_registry(&mut self, registry: AccountId) {
        self.only_owner();
        self.registry = registry;
    }

    pub fn set_recorder(&mut self, account_id: AccountId, allowed: bool) {
        self.only_owner();
        if allowed {
            self.recorders.insert(account_id);
        } else {
            self.recorders.remove(&account_id);
        }
    }

    pub fn is_recorder(&self, account_id: AccountId) -> bool {
        self.recorders.contains(&account_id)
    }

    pub fn set_base_uri(&mut self, base_uri: String) {
        self.only_owner();
        require!(valid_base_uri(&base_uri), "base uri");
        self.base_uri = base_uri;
    }

    pub fn base_uri(&self) -> String {
        self.base_uri.clone()
    }

    #[payable]
    pub fn mint(&mut self, token_id: String, owner: AccountId, agent_id: String) -> String {
        self.only_registry();
        require!(
            env::attached_deposit().as_yoctonear() >= MINT_DEPOSIT,
            "mint deposit"
        );
        require!(valid_agent_id(&agent_id), "agent id");
        require!(!self.agent_to_token.contains_key(&agent_id), "agent bound");
        self.mint_internal(token_id, owner, Some(agent_id))
    }

    /// Registry-only seed mint for launch stock. The token starts unbound so a
    /// buyer can bind it to their own agent after purchase.
    #[payable]
    pub fn mint_unbound(&mut self, token_id: String, owner: AccountId) -> String {
        self.only_registry();
        require!(
            env::attached_deposit().as_yoctonear() >= MINT_DEPOSIT,
            "mint deposit"
        );
        self.mint_internal(token_id, owner, None)
    }

    /// Mint launch stock in one transaction. Keep the batch at or below 25 so
    /// the storage writes stay inside mainnet gas limits.
    #[payable]
    pub fn mint_unbound_batch(&mut self, token_ids: Vec<String>, owner: AccountId) -> Vec<String> {
        self.only_registry();
        require!(!token_ids.is_empty(), "empty batch");
        require!(token_ids.len() <= MAX_MINT_BATCH, "batch too large");
        let deposit = env::attached_deposit().as_yoctonear();
        require!(
            deposit >= MINT_DEPOSIT.saturating_mul(token_ids.len() as u128),
            "mint deposit"
        );
        let mut minted = Vec::with_capacity(token_ids.len());
        for token_id in token_ids {
            minted.push(self.mint_internal(token_id, owner.clone(), None));
        }
        minted
    }

    /// Owner recovery for excess mint deposits. This cannot touch a token's
    /// storage stake unless the owner drains the account; keep the balance above
    /// the code and state stake.
    pub fn withdraw(&mut self, account_id: AccountId, amount: U128) {
        self.only_owner();
        require!(amount.0 > 0, "amount");
        let stake = u128::from(env::storage_usage()) * env::storage_byte_cost().as_yoctonear();
        require!(
            env::account_balance()
                .as_yoctonear()
                .saturating_sub(amount.0)
                >= stake,
            "keep storage stake"
        );
        Promise::new(account_id).transfer(NearToken::from_yoctonear(amount.0));
    }

    pub fn mint_batch_limit(&self) -> u32 {
        MAX_MINT_BATCH as u32
    }

    pub fn bind(&mut self, token_id: String, agent_id: String) {
        require!(
            env::predecessor_account_id() == self.registry,
            "registry binds"
        );
        require!(valid_agent_id(&agent_id), "agent id");
        let mut token = self.token_or_panic(&token_id);
        require!(token.bound_agent.is_none(), "token bound");
        require!(!self.agent_to_token.contains_key(&agent_id), "agent bound");
        token.bound_agent = Some(agent_id.clone());
        token.last_agent = None;
        self.tokens.insert(token_id.clone(), token);
        self.agent_to_token
            .insert(agent_id.clone(), token_id.clone());
        env::log_str(&passport_event(
            "passport_bind",
            &near_sdk::serde_json::json!({
                "token_id": token_id,
                "agent_id": agent_id,
            }),
        ));
    }

    pub fn unbind(&mut self, token_id: String) {
        let caller = env::predecessor_account_id();
        let mut token = self.token_or_panic(&token_id);
        require!(token.owner_id == caller, "not owner");
        let previous = self.take_binding(&token_id, &mut token);
        token.last_agent = None;
        self.tokens.insert(token_id.clone(), token);
        if let Some(agent) = previous {
            env::log_str(&passport_event(
                "passport_unbind",
                &near_sdk::serde_json::json!({
                    "token_id": token_id,
                    "agent_id": agent,
                }),
            ));
        }
    }

    pub fn lock(&mut self, token_id: String, until_unix: u64) {
        self.only_recorder();
        let now = self.now();
        require!(until_unix > now, "lock in past");
        let mut token = self.token_or_panic(&token_id);
        token.locked_until = until_unix;
        self.tokens.insert(token_id.clone(), token);
        env::log_str(&passport_event(
            "passport_lock",
            &near_sdk::serde_json::json!({
                "token_id": token_id,
                "until_unix": until_unix,
            }),
        ));
    }

    pub fn unlock(&mut self, token_id: String) {
        self.only_recorder();
        let mut token = self.token_or_panic(&token_id);
        token.locked_until = 0;
        self.tokens.insert(token_id.clone(), token);
        env::log_str(&passport_event(
            "passport_unlock",
            &near_sdk::serde_json::json!({ "token_id": token_id }),
        ));
    }

    pub fn is_locked(&self, token_id: String) -> bool {
        self.token_or_panic(&token_id).locked_until > self.now()
    }

    pub fn sync_career(&mut self, token_id: String, update: CareerUpdate) {
        self.only_recorder();
        let mut token = self.token_or_panic(&token_id);
        require!(update.seq > token.last_sync_seq, "stale update");
        require!(update.xp >= token.career.xp, "xp");
        require!(update.wins >= token.career.wins, "wins");
        require!(update.losses >= token.career.losses, "losses");
        require!(update.draws >= token.career.draws, "draws");
        require!(update.matches >= token.career.matches, "matches");
        require!(update.heists >= token.career.heists, "heists");
        require!(update.seasons >= token.career.seasons, "seasons");
        require!(
            update.season_titles >= token.career.season_titles,
            "season titles"
        );
        let best_rank = match (token.career.best_rank, update.best_rank) {
            (0, rank) => rank,
            (best, 0) => best,
            (best, rank) => best.min(rank),
        };
        token.career = Career {
            xp: update.xp,
            wins: update.wins,
            losses: update.losses,
            draws: update.draws,
            matches: update.matches,
            heists: update.heists,
            seasons: update.seasons,
            season_titles: update.season_titles,
            best_rank,
            rank: update.rank,
            rank_epoch: update.rank_epoch,
        };
        token.last_sync_seq = update.seq;
        self.tokens.insert(token_id.clone(), token);
        env::log_str(&passport_event(
            "passport_sync",
            &near_sdk::serde_json::json!({
                "token_id": token_id,
                "seq": update.seq,
                "xp": update.xp,
            }),
        ));
    }

    pub fn sync_career_batch(&mut self, updates: Vec<(String, CareerUpdate)>) {
        require!(updates.len() <= MAX_UPDATE_BATCH, "batch too large");
        for (token_id, update) in updates {
            self.sync_career(token_id, update);
        }
    }

    pub fn passport(&self, token_id: String) -> PassportView {
        let token = self.token_or_panic(&token_id);
        PassportView {
            token_id,
            owner_id: token.owner_id.clone(),
            creator: token.creator.clone(),
            platform: token.platform.clone(),
            bound_agent: token.bound_agent.clone(),
            tier: tier_for(token.career.xp).into(),
            career: token.career.clone(),
            last_sync_seq: token.last_sync_seq,
            locked_until: token.locked_until,
        }
    }

    /// Record the sha256 of one encrypted strategy version. The argument is a
    /// 64-character hex digest. Plaintext is rejected. Only a recorder may write.
    pub fn set_playbook_hash(&mut self, token_id: String, version: u64, hash: String) {
        self.only_recorder();
        let _ = self.token_or_panic(&token_id);
        require!(version > 0, "version");
        require!(valid_playbook_hash(&hash), "hash");
        let hash = hash.to_ascii_lowercase();
        let mut store = LookupMap::<String, String>::new(b"h");
        let version_key = format!("v:{token_id}:{version}");
        if let Some(existing) = store.get(&version_key) {
            require!(*existing == hash, "hash");
        } else {
            store.insert(version_key, hash.clone());
        }
        let latest_key = format!("l:{token_id}");
        if let Some(latest) = store.get(&latest_key) {
            let current = latest
                .split_once(':')
                .and_then(|(version, _)| version.parse::<u64>().ok())
                .unwrap_or(0);
            require!(version >= current, "version");
        }
        store.insert(latest_key, format!("{version}:{hash}"));
        env::log_str(&passport_event(
            "playbook_hash",
            &near_sdk::serde_json::json!({
                "token_id": token_id,
                "version": version,
            }),
        ));
    }

    pub fn playbook_hash(&self, token_id: String, version: u64) -> Option<String> {
        let store = LookupMap::<String, String>::new(b"h");
        store.get(&format!("v:{token_id}:{version}")).cloned()
    }

    pub fn playbook_latest(&self, token_id: String) -> Option<PlaybookHashView> {
        let store = LookupMap::<String, String>::new(b"h");
        let stored = store.get(&format!("l:{token_id}"))?;
        let (version, hash) = stored.split_once(':')?;
        let version = version.parse().ok()?;
        if !valid_playbook_hash(hash) {
            return None;
        }
        Some(PlaybookHashView {
            token_id,
            version,
            hash: hash.to_string(),
        })
    }

    pub fn token_agent(&self, token_id: String) -> Option<String> {
        self.tokens
            .get(&token_id)
            .and_then(|token| token.bound_agent.clone())
    }

    pub fn agent_token(&self, agent_id: String) -> Option<String> {
        self.agent_to_token.get(&agent_id).cloned()
    }

    pub fn nft_token(&self, token_id: String) -> Option<JsonToken> {
        self.tokens.get(&token_id).map(|token| JsonToken {
            token_id: token_id.clone(),
            owner_id: token.owner_id.clone(),
            metadata: self.metadata_for(&token_id, token),
        })
    }

    pub fn nft_total_supply(&self) -> U128 {
        U128(self.supply_ids.len() as u128)
    }

    pub fn nft_supply_for_owner(&self, account_id: AccountId) -> U128 {
        U128(
            self.by_owner
                .get(&account_id)
                .map(|ids| ids.len())
                .unwrap_or(0) as u128,
        )
    }

    pub fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<JsonToken> {
        let start = from_index.map(|value| value.0 as usize).unwrap_or(0);
        let limit = limit.unwrap_or(32) as usize;
        self.supply_ids
            .iter()
            .skip(start)
            .take(limit)
            .filter_map(|id| self.nft_token(id.clone()))
            .collect()
    }

    pub fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<JsonToken> {
        let start = from_index.map(|value| value.0 as usize).unwrap_or(0);
        let limit = limit.unwrap_or(32) as usize;
        self.by_owner
            .get(&account_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .skip(start)
            .take(limit)
            .filter_map(|id| self.nft_token(id))
            .collect()
    }

    pub fn token_reference(&self, token_id: String) -> String {
        let token = self.token_or_panic(&token_id);
        reference_json(&token_id, &token)
    }

    pub fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: u32) -> Payout {
        Payout {
            payout: payout_map(self.token_or_panic(&token_id), balance.0, max_len_payout),
        }
    }

    #[payable]
    pub fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: u64,
        memo: Option<String>,
        balance: U128,
        max_len_payout: u32,
    ) -> Payout {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let token = self.token_or_panic(&token_id);
        require!(token.locked_until <= self.now(), "passport locked");
        let payout = payout_map(token, balance.0, max_len_payout);
        let sender = env::predecessor_account_id();
        self.transfer_internal(
            &sender,
            &receiver_id,
            &token_id,
            Some(approval_id),
            memo.as_deref(),
        );
        Payout { payout }
    }

    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let sender = env::predecessor_account_id();
        self.transfer_internal(
            &sender,
            &receiver_id,
            &token_id,
            approval_id,
            memo.as_deref(),
        );
    }

    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> Promise {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let sender = env::predecessor_account_id();
        self.transfer_internal(
            &sender,
            &receiver_id,
            &token_id,
            approval_id,
            memo.as_deref(),
        );
        let resolve_gas = Gas::from_tgas(15);
        let forward = env::prepaid_gas()
            .saturating_sub(resolve_gas)
            .saturating_sub(Gas::from_tgas(25));
        ext_receiver::ext(receiver_id.clone())
            .with_static_gas(forward)
            .nft_on_transfer(sender.clone(), sender.clone(), token_id.clone(), msg)
            .then(
                ext_passport_self::ext(env::current_account_id())
                    .with_static_gas(resolve_gas)
                    .nft_resolve_transfer(sender, receiver_id, token_id),
            )
    }

    #[private]
    pub fn nft_resolve_transfer(
        &mut self,
        previous_owner: AccountId,
        receiver_id: AccountId,
        token_id: String,
    ) -> bool {
        let give_back = match env::promise_result(0) {
            near_sdk::PromiseResult::Successful(bytes) => {
                near_sdk::serde_json::from_slice::<bool>(&bytes).unwrap_or(true)
            }
            _ => true,
        };
        if !give_back {
            if let Some(mut token) = self.tokens.get(&token_id).cloned() {
                token.last_agent = None;
                self.tokens.insert(token_id, token);
            }
            return true;
        }
        let still_there = self
            .tokens
            .get(&token_id)
            .map(|token| token.owner_id == receiver_id)
            .unwrap_or(false);
        if !still_there {
            return true;
        }
        let remembered = self
            .tokens
            .get(&token_id)
            .and_then(|token| token.last_agent.clone());
        self.transfer_internal(&receiver_id, &previous_owner, &token_id, None, None);
        if let Some(agent) = remembered {
            if !self.agent_to_token.contains_key(&agent) {
                let mut token = self.token_or_panic(&token_id);
                token.bound_agent = Some(agent.clone());
                token.last_agent = None;
                self.tokens.insert(token_id.clone(), token);
                self.agent_to_token.insert(agent, token_id);
            }
        }
        false
    }

    #[payable]
    pub fn nft_approve(
        &mut self,
        token_id: String,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        let deposit = env::attached_deposit().as_yoctonear();
        let owner = env::predecessor_account_id();
        let before = env::storage_usage();
        let mut token = self.token_or_panic(&token_id);
        require!(token.owner_id == owner, "not owner");
        require!(token.locked_until <= self.now(), "passport locked");
        let approval_id = token.next_approval_id;
        token.next_approval_id += 1;
        token.approvals.retain(|(id, _)| id != &account_id);
        token.approvals.push((account_id.clone(), approval_id));
        self.tokens.insert(token_id.clone(), token);
        let after = env::storage_usage();
        let cost = if after > before {
            u128::from(after - before) * env::storage_byte_cost().as_yoctonear()
        } else {
            0
        };
        let keep = cost.max(1);
        require!(deposit >= keep, "storage");
        let refund = deposit - keep;
        if refund > 0 {
            Promise::new(owner.clone()).transfer(NearToken::from_yoctonear(refund));
        }
        env::log_str(&nft_event(
            "nft_approve",
            &near_sdk::serde_json::json!({
                "token_id": token_id,
                "account_id": account_id,
                "approval_id": approval_id,
            }),
        ));
        msg.map(|msg| {
            let forward = env::prepaid_gas().saturating_sub(Gas::from_tgas(25));
            ext_approval_receiver::ext(account_id)
                .with_static_gas(forward)
                .nft_on_approve(token_id, owner, approval_id, msg)
        })
    }

    #[payable]
    pub fn nft_revoke(&mut self, token_id: String, account_id: AccountId) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let owner = env::predecessor_account_id();
        let mut token = self.token_or_panic(&token_id);
        require!(token.owner_id == owner, "not owner");
        token.approvals.retain(|(id, _)| id != &account_id);
        self.tokens.insert(token_id, token);
    }

    #[payable]
    pub fn nft_revoke_all(&mut self, token_id: String) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let owner = env::predecessor_account_id();
        let mut token = self.token_or_panic(&token_id);
        require!(token.owner_id == owner, "not owner");
        token.approvals.clear();
        self.tokens.insert(token_id, token);
    }

    pub fn nft_is_approved(
        &self,
        token_id: String,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        let Some(token) = self.tokens.get(&token_id) else {
            return false;
        };
        token.approvals.iter().any(|(id, current)| {
            id == &approved_account_id && approval_id.map(|want| want == *current).unwrap_or(true)
        })
    }
}

impl Passport {
    fn only_owner(&self) {
        require!(env::predecessor_account_id() == self.owner, "owner");
    }

    fn only_registry(&self) {
        require!(
            env::predecessor_account_id() == self.registry,
            "registry pays the mint"
        );
    }

    fn mint_internal(
        &mut self,
        token_id: String,
        owner: AccountId,
        agent_id: Option<String>,
    ) -> String {
        require!(valid_token_id(&token_id), "token id");
        require!(!self.tokens.contains_key(&token_id), "token exists");
        let token = PassportToken {
            owner_id: owner.clone(),
            creator: owner.clone(),
            platform: self.platform.clone(),
            next_approval_id: 1,
            approvals: Vec::new(),
            bound_agent: agent_id.clone(),
            last_agent: None,
            career: Career::empty(),
            last_sync_seq: 0,
            locked_until: 0,
        };
        self.tokens.insert(token_id.clone(), token);
        if let Some(agent_id) = &agent_id {
            self.agent_to_token
                .insert(agent_id.clone(), token_id.clone());
        }
        self.add_owner_token(&owner, &token_id);
        self.supply_ids.push(token_id.clone());
        env::log_str(&nft_event(
            "nft_mint",
            &near_sdk::serde_json::json!({
                "owner_id": owner,
                "token_ids": [token_id],
            }),
        ));
        if let Some(agent_id) = agent_id {
            env::log_str(&passport_event(
                "passport_bind",
                &near_sdk::serde_json::json!({
                    "token_id": token_id,
                    "agent_id": agent_id,
                }),
            ));
        }
        token_id
    }

    fn only_recorder(&self) {
        require!(
            self.recorders.contains(&env::predecessor_account_id()),
            "recorder"
        );
    }

    fn now(&self) -> u64 {
        env::block_timestamp() / 1_000_000_000
    }

    fn token_or_panic(&self, token_id: &str) -> PassportToken {
        self.tokens
            .get(&token_id.to_string())
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"))
    }

    fn add_owner_token(&mut self, owner: &AccountId, token_id: &str) {
        let mut ids = self.by_owner.get(owner).cloned().unwrap_or_default();
        if !ids.iter().any(|id| id == token_id) {
            ids.push(token_id.to_string());
            self.by_owner.insert(owner.clone(), ids);
        }
    }

    fn remove_owner_token(&mut self, owner: &AccountId, token_id: &str) {
        let mut ids = self.by_owner.get(owner).cloned().unwrap_or_default();
        ids.retain(|id| id != token_id);
        self.by_owner.insert(owner.clone(), ids);
    }

    fn take_binding(&mut self, token_id: &str, token: &mut PassportToken) -> Option<String> {
        let agent = token.bound_agent.take();
        if let Some(agent) = &agent {
            if self
                .agent_to_token
                .get(agent)
                .map(|id| id.as_str() == token_id)
                .unwrap_or(false)
            {
                let _ = self.agent_to_token.remove(agent);
            }
        }
        agent
    }

    fn transfer_internal(
        &mut self,
        sender: &AccountId,
        receiver: &AccountId,
        token_id: &str,
        approval_id: Option<u64>,
        memo: Option<&str>,
    ) {
        require!(sender != receiver, "sender is receiver");
        let mut token = self.token_or_panic(token_id);
        require!(token.locked_until <= self.now(), "passport locked");
        let approved = token.approvals.iter().any(|(id, current)| {
            id == sender && approval_id.map(|want| want == *current).unwrap_or(true)
        });
        let allowed = &token.owner_id == sender || approved;
        require!(allowed, "not approved");
        let authorized = if &token.owner_id == sender {
            None
        } else {
            Some(sender.clone())
        };
        let previous = token.owner_id.clone();
        let previous_agent = self.take_binding(token_id, &mut token);
        token.last_agent = previous_agent;
        self.remove_owner_token(&previous, token_id);
        token.owner_id = receiver.clone();
        token.approvals.clear();
        self.tokens.insert(token_id.to_string(), token);
        self.add_owner_token(receiver, token_id);
        let mut data = near_sdk::serde_json::json!({
            "old_owner_id": previous,
            "new_owner_id": receiver,
            "token_ids": [token_id],
        });
        if let Some(authorized_id) = authorized {
            data["authorized_id"] = near_sdk::serde_json::json!(authorized_id);
        }
        if let Some(memo) = memo {
            data["memo"] = near_sdk::serde_json::json!(memo);
        }
        env::log_str(&nft_event("nft_transfer", &data));
    }

    fn metadata_for(&self, token_id: &str, token: &PassportToken) -> TokenMetadata {
        let document = reference_json(token_id, token);
        TokenMetadata {
            title: Some(format!("fe2VI passport {token_id}")),
            description: Some(
                "Tradable fe2VI career passport. Owning it does not transfer the agent credential."
                    .into(),
            ),
            media: Some(format!("{token_id}.png")),
            media_hash: None,
            copies: Some(1),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: Some(document.clone()),
            reference: Some(format!("{token_id}.json")),
            reference_hash: Some(Base64VecU8(env::sha256(document.as_bytes()))),
        }
    }
}

fn valid_base_uri(uri: &str) -> bool {
    (12..=180).contains(&uri.len())
        && uri.starts_with("https://")
        && uri.ends_with('/')
        && !uri.contains(' ')
}

fn valid_token_id(token_id: &str) -> bool {
    let bytes = token_id.as_bytes();
    (3..=64).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

fn valid_agent_id(agent_id: &str) -> bool {
    let bytes = agent_id.as_bytes();
    (1..=80).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-' | b'.' | b':'))
}

fn tier_for(xp: u64) -> &'static str {
    match xp {
        0..=9 => "Rookie",
        10..=29 => "Bronze",
        30..=69 => "Silver",
        70..=149 => "Gold",
        150..=299 => "Platinum",
        _ => "Diamond",
    }
}

fn reference_json(token_id: &str, token: &PassportToken) -> String {
    let career = &token.career;
    let tier = tier_for(career.xp);
    let agent = token.bound_agent.as_deref().unwrap_or("unbound");
    let attributes = vec![
        near_sdk::serde_json::json!({ "trait_type": "Agent", "value": agent }),
        near_sdk::serde_json::json!({ "trait_type": "XP", "value": career.xp.to_string() }),
        near_sdk::serde_json::json!({ "trait_type": "Tier", "value": tier }),
        near_sdk::serde_json::json!({ "trait_type": "Rank", "value": career.rank.to_string() }),
        near_sdk::serde_json::json!({ "trait_type": "Wins", "value": career.wins.to_string() }),
        near_sdk::serde_json::json!({ "trait_type": "Matches", "value": career.matches.to_string() }),
        near_sdk::serde_json::json!({ "trait_type": "Heists", "value": career.heists.to_string() }),
        near_sdk::serde_json::json!({ "trait_type": "Seasons", "value": career.seasons.to_string() }),
    ];
    near_sdk::serde_json::to_string(&near_sdk::serde_json::json!({
        "name": format!("fe2VI passport {token_id}"),
        "description": "Tradable fe2VI career passport. The agent credential never moves.",
        "image": format!("{token_id}.png"),
        "external_url": format!("https://fe2vi.space/passport/{token_id}"),
        "agent": token.bound_agent,
        "owner": token.owner_id,
        "creator": token.creator,
        "career": {
            "xp": career.xp,
            "wins": career.wins,
            "losses": career.losses,
            "draws": career.draws,
            "matches": career.matches,
            "heists": career.heists,
            "seasons": career.seasons,
            "season_titles": career.season_titles,
            "best_rank": career.best_rank,
            "rank": career.rank,
            "rank_epoch": career.rank_epoch,
        },
        "tier": tier,
        "locked_until": token.locked_until,
        "last_sync_seq": token.last_sync_seq,
        "attributes": attributes,
    }))
    .unwrap_or_else(|_| "{}".into())
}

fn add_cut(
    payout: &mut BTreeMap<AccountId, u128>,
    account: &AccountId,
    cut: u128,
    paid: &mut u128,
) {
    if cut == 0 {
        return;
    }
    *payout.entry(account.clone()).or_insert(0) += cut;
    *paid += cut;
}

fn payout_map(
    token: PassportToken,
    balance: u128,
    max_len_payout: u32,
) -> BTreeMap<AccountId, U128> {
    require!(max_len_payout > 0, "payout exceeds max_len");
    let mut payout: BTreeMap<AccountId, u128> = BTreeMap::new();
    let mut paid = 0u128;
    if token.creator != token.owner_id {
        let cut = balance.saturating_mul(u128::from(CREATOR_BPS)) / 10_000;
        add_cut(&mut payout, &token.creator, cut, &mut paid);
    }
    if token.platform != token.owner_id {
        let cut = balance.saturating_mul(u128::from(PLATFORM_BPS)) / 10_000;
        add_cut(&mut payout, &token.platform, cut, &mut paid);
    }
    require!(paid <= balance, "royalty");
    let seller = balance - paid;
    if seller > 0 {
        *payout.entry(token.owner_id).or_insert(0) += seller;
    }
    require!(
        payout.len() as u32 <= max_len_payout,
        "payout exceeds max_len"
    );
    payout
        .into_iter()
        .map(|(account, amount)| (account, U128(amount)))
        .collect()
}

fn nft_event(event: &str, data: &near_sdk::serde_json::Value) -> String {
    format!(
        "EVENT_JSON:{}",
        near_sdk::serde_json::json!({
            "standard": "nep171",
            "version": "1.0.0",
            "event": event,
            "data": [data],
        })
    )
}

fn passport_event(event: &str, data: &near_sdk::serde_json::Value) -> String {
    format!(
        "EVENT_JSON:{}",
        near_sdk::serde_json::json!({
            "standard": "fe2vi",
            "version": "1.0.0",
            "event": event,
            "data": [data],
        })
    )
}

#[allow(dead_code)]
#[ext_contract(ext_approval_receiver)]
trait NonFungibleTokenApprovalsReceiver {
    fn nft_on_approve(
        &mut self,
        token_id: String,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    );
}

#[allow(dead_code)]
#[ext_contract(ext_receiver)]
trait NonFungibleTokenReceiver {
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: String,
        msg: String,
    );
}

#[allow(dead_code)]
#[ext_contract(ext_passport_self)]
trait PassportResolve {
    fn nft_resolve_transfer(
        &mut self,
        previous_owner: AccountId,
        receiver_id: AccountId,
        token_id: String,
    ) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, Gas, NearToken};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn context(predecessor: &str, signer: &str, deposit: u128, unix: u64) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("passport.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(signer.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(100))
            .prepaid_gas(Gas::from_tgas(300))
            .block_timestamp(unix * 1_000_000_000);
        testing_env!(builder.build());
    }

    fn boot() -> Passport {
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 0);
        let mut passport = Passport::new(
            "owner.fe2vi.testnet".parse().unwrap(),
            "platform.fe2vi.testnet".parse().unwrap(),
            "registry.fe2vi.testnet".parse().unwrap(),
        );
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 0);
        passport.set_recorder("recorder.fe2vi.testnet".parse().unwrap(), true);
        passport
    }

    fn minted() -> (Passport, String) {
        let mut passport = boot();
        context(
            "registry.fe2vi.testnet",
            "registry.fe2vi.testnet",
            MINT_DEPOSIT,
            1,
        );
        passport.mint(
            "p_alpha".into(),
            "owner.fe2vi.testnet".parse().unwrap(),
            "agent_alpha".into(),
        );
        (passport, "p_alpha".into())
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

    fn update(seq: u64, xp: u64, wins: u32, matches: u32) -> CareerUpdate {
        CareerUpdate {
            seq,
            xp,
            wins,
            losses: 0,
            draws: 0,
            matches,
            heists: 0,
            seasons: 1,
            season_titles: 0,
            best_rank: 0,
            rank: 0,
            rank_epoch: 0,
        }
    }

    #[test]
    fn mint_is_registry_only_and_requires_the_deposit() {
        let mut passport = boot();
        let owner: AccountId = "owner.fe2vi.testnet".parse().unwrap();
        context(
            "owner.fe2vi.testnet",
            "owner.fe2vi.testnet",
            MINT_DEPOSIT,
            1,
        );
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.mint("p_alpha".into(), owner.clone(), "agent_alpha".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("registry"));

        context(
            "registry.fe2vi.testnet",
            "registry.fe2vi.testnet",
            MINT_DEPOSIT - 1,
            1,
        );
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.mint("p_alpha".into(), owner, "agent_alpha".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("mint deposit"));
    }

    #[test]
    fn launch_batch_mints_unbound_tokens_for_the_registry() {
        let mut passport = boot();
        let owner: AccountId = "owner.fe2vi.testnet".parse().unwrap();
        let ids: Vec<String> = (1..=25).map(|n| format!("fdr_{n:03}")).collect();
        context(
            "owner.fe2vi.testnet",
            "owner.fe2vi.testnet",
            MINT_DEPOSIT * 25,
            1,
        );
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.mint_unbound_batch(ids.clone(), owner.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("registry"));

        context(
            "registry.fe2vi.testnet",
            "registry.fe2vi.testnet",
            MINT_DEPOSIT * 24,
            1,
        );
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.mint_unbound_batch(ids.clone(), owner.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("mint deposit"));

        context(
            "registry.fe2vi.testnet",
            "registry.fe2vi.testnet",
            MINT_DEPOSIT * 25,
            1,
        );
        let minted = passport.mint_unbound_batch(ids.clone(), owner.clone());
        assert_eq!(minted, ids);
        for id in &ids {
            assert_eq!(passport.token_agent(id.clone()), None);
            assert_eq!(passport.passport(id.clone()).owner_id, owner);
        }
        assert_eq!(passport.nft_supply_for_owner(owner).0, 25);
        assert_eq!(passport.nft_total_supply().0, 25);
    }

    #[test]
    fn launch_storage_sizes_are_documented() {
        let platform: AccountId = "platform.fe2vi.near".parse().unwrap();
        let owner: AccountId = "launch.fe2vi.near".parse().unwrap();
        let mut token = PassportToken {
            owner_id: owner.clone(),
            creator: owner.clone(),
            platform,
            next_approval_id: 1,
            approvals: Vec::new(),
            bound_agent: None,
            last_agent: None,
            career: Career::empty(),
            last_sync_seq: 0,
            locked_until: 0,
        };
        let unbound = borsh::to_vec(&token).unwrap().len();
        token.bound_agent = Some("agent.fe2vi.near".into());
        token
            .approvals
            .push(("passport-market.fe2vi.near".parse().unwrap(), 1));
        let bound = borsh::to_vec(&token).unwrap().len();
        let owner_ids = borsh::to_vec(&vec!["fdr_001".to_string(); 100])
            .unwrap()
            .len();
        println!("token_unbound={unbound} token_bound_listed={bound} owner_index_100={owner_ids}");
        assert!(unbound < 300);
        assert!(bound < 400);
    }

    #[test]
    fn one_agent_holds_one_passport() {
        let (mut passport, _) = minted();
        context(
            "registry.fe2vi.testnet",
            "registry.fe2vi.testnet",
            MINT_DEPOSIT,
            1,
        );
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.mint(
                "p_second".into(),
                "owner.fe2vi.testnet".parse().unwrap(),
                "agent_alpha".into(),
            );
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("agent bound"));
    }

    #[test]
    fn playbook_hash_is_recorder_only_and_stores_the_digest() {
        let (mut passport, token) = minted();
        let digest = "ab".repeat(32);
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 2);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.set_playbook_hash(token.clone(), 1, digest.clone());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("recorder"));

        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 3);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.set_playbook_hash(token.clone(), 1, "hold the east door".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("hash"));

        passport.set_playbook_hash(token.clone(), 1, digest.clone());
        passport.set_playbook_hash(token.clone(), 1, digest.to_uppercase());
        assert_eq!(
            passport.playbook_hash(token.clone(), 1).as_deref(),
            Some(digest.as_str())
        );
        let latest = passport.playbook_latest(token.clone()).unwrap();
        assert_eq!(latest.version, 1);
        assert_eq!(latest.hash, digest);
        assert!(passport.passport(token.clone()).career.xp == 0);

        let other = "cd".repeat(32);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.set_playbook_hash(token.clone(), 1, other);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("hash"));
    }

    #[test]
    fn transfer_clears_binding_and_keeps_the_career() {
        let (mut passport, token) = minted();
        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 5);
        passport.sync_career(token.clone(), update(1, 30, 10, 10));

        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 1, 6);
        passport.nft_transfer(seller.clone(), token.clone(), None, None);
        let view = passport.passport(token.clone());
        assert_eq!(view.owner_id, seller);
        assert_eq!(view.bound_agent, None);
        assert_eq!(view.career.xp, 30);
        assert_eq!(view.tier, "Silver");
        assert_eq!(passport.agent_token("agent_alpha".into()), None);

        context("registry.fe2vi.testnet", "registry.fe2vi.testnet", 0, 6);
        passport.bind(token.clone(), "agent_beta".into());
        let view = passport.passport(token.clone());
        assert_eq!(view.bound_agent.as_deref(), Some("agent_beta"));
        assert_eq!(passport.agent_token("agent_beta".into()), Some(token));
    }

    #[test]
    fn owner_can_unbind_but_only_registry_can_bind() {
        let (mut passport, token) = minted();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 2);
        passport.unbind(token.clone());
        assert_eq!(passport.token_agent(token.clone()), None);

        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 2);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.bind(token.clone(), "agent_beta".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("registry binds"));

        context("registry.fe2vi.testnet", "registry.fe2vi.testnet", 0, 2);
        passport.bind(token.clone(), "agent_beta".into());
        assert_eq!(passport.token_agent(token), Some("agent_beta".into()));
    }

    #[test]
    fn payout_is_two_three_and_ninety_five_after_the_first_sale() {
        let (mut passport, token) = minted();
        let owner: AccountId = "owner.fe2vi.testnet".parse().unwrap();
        let platform: AccountId = "platform.fe2vi.testnet".parse().unwrap();
        let first = passport.nft_payout(token.clone(), U128(100), 3);
        assert_eq!(first.payout.get(&owner), Some(&U128(97)));
        assert_eq!(first.payout.get(&platform), Some(&U128(3)));
        assert_eq!(first.payout.len(), 2);

        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 1, 2);
        passport.nft_transfer(seller.clone(), token.clone(), None, None);
        let resale = passport.nft_payout(token, U128(100), 3);
        assert_eq!(resale.payout.get(&owner), Some(&U128(2)));
        assert_eq!(resale.payout.get(&platform), Some(&U128(3)));
        assert_eq!(resale.payout.get(&seller), Some(&U128(95)));
        assert_eq!(
            resale.payout.values().map(|value| value.0).sum::<u128>(),
            100
        );
    }

    #[test]
    fn market_approval_transfers_and_pays() {
        let (mut passport, token) = minted();
        let market: AccountId = "market.fe2vi.testnet".parse().unwrap();
        let buyer: AccountId = "buyer.fe2vi.testnet".parse().unwrap();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", ONE_NEAR, 2);
        passport.nft_approve(token.clone(), market.clone(), None);
        assert!(passport.nft_is_approved(token.clone(), market.clone(), Some(1)));

        context("market.fe2vi.testnet", "market.fe2vi.testnet", 1, 2);
        let payout =
            passport.nft_transfer_payout(buyer.clone(), token.clone(), 1, None, U128(100), 3);
        assert_eq!(payout.payout.get(&market), None);
        assert_eq!(passport.nft_token(token.clone()).unwrap().owner_id, buyer);
        assert_eq!(passport.token_agent(token), None);
    }

    #[test]
    fn lock_blocks_transfer_until_expiry_or_unlock() {
        let (mut passport, token) = minted();
        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 5);
        passport.lock(token.clone(), 10);
        assert!(passport.is_locked(token.clone()));

        let buyer: AccountId = "buyer.fe2vi.testnet".parse().unwrap();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 1, 5);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.nft_transfer(buyer.clone(), token.clone(), None, None);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("passport locked"));

        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 1, 11);
        passport.nft_transfer(buyer.clone(), token.clone(), None, None);
        assert_eq!(passport.nft_token(token.clone()).unwrap().owner_id, buyer);

        let (mut passport, token) = minted();
        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 5);
        passport.lock(token.clone(), 10);
        passport.unlock(token.clone());
        assert!(!passport.is_locked(token));
    }

    #[test]
    fn sync_is_recorder_only_monotonic_and_idempotent() {
        let (mut passport, token) = minted();
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0, 5);
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.sync_career(token.clone(), update(1, 3, 1, 1));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("recorder"));

        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 5);
        passport.sync_career(token.clone(), update(2, 6, 2, 2));
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.sync_career(token.clone(), update(2, 6, 2, 2));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("stale update"));
        let err = catch_unwind(AssertUnwindSafe(|| {
            passport.sync_career(token.clone(), update(3, 5, 3, 3));
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("xp"));
        passport.sync_career(token, update(3, 9, 3, 3));
    }

    #[test]
    fn metadata_and_reference_describe_the_career() {
        let (mut passport, token) = minted();
        context("recorder.fe2vi.testnet", "recorder.fe2vi.testnet", 0, 5);
        let mut career = update(1, 150, 40, 50);
        career.best_rank = 2;
        career.rank = 4;
        career.rank_epoch = 3;
        passport.sync_career(token.clone(), career);
        let token_view = passport.nft_token(token.clone()).unwrap();
        assert_eq!(token_view.metadata.media.as_deref(), Some("p_alpha.png"));
        assert_eq!(
            token_view.metadata.reference.as_deref(),
            Some("p_alpha.json")
        );
        let document = passport.token_reference(token.clone());
        assert!(document.contains("\"tier\":\"Platinum\""));
        assert!(document.contains("\"agent\":\"agent_alpha\""));
        assert!(document.contains("\"xp\":150"));
        assert_eq!(passport.passport(token).tier, "Platinum");
    }
}

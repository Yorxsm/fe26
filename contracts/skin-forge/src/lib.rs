//! Cosmetic skins for fe2VI agents.
//!
//! `MINT_PRICE` is 1 NEAR unless the owner changes it. That price includes
//! storage staking: the byte cost stays on this contract and the remainder is
//! paid to the treasury (and, if configured, a prize-pot share). Callers are
//! not charged a second storage fee. Anything above the price is refunded.
//!
//! A skin is paint. It does not change caps, scoring, or who may act.
//! The default look is `default_skin(account)` and stores nothing.
//!
//! Function-call keys on the heist cannot attach this deposit. The registry
//! checks the cap, then calls `mint_skin` with the price attached.

mod media;

use std::collections::{BTreeMap, BTreeSet};

use fe2vi_core::{
    default_skin as catalog_default, earned_progress, earned_trait, unknown_trait, SkinTraits,
    EARNED_TRAITS, MINT_PRICE_YOCTO, ONE_NEAR,
};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::Serialize;
use near_sdk::store::{LookupMap, LookupSet};
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
};

const DEFAULT_CREATOR_BPS: u16 = 200;
const DEFAULT_PLATFORM_BPS: u16 = 300;
const MAX_ROYALTY_BPS: u16 = 1_000;
/// Placeholder gateway. The owner replaces it before any public listing. Not a live host.
const DEFAULT_BASE_URI: &str = "https://assets.fe2vi.testnet/skins/";
const TROPHY_CAP: usize = 64;

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Trophy {
    pub game: String,
    pub outcome: String,
    pub season: u32,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct SkinRecord {
    pub wins: u32,
    pub heists_cracked: u32,
    pub times_caught: u32,
    pub seasons_played: u32,
    pub titled_seasons: Vec<u32>,
    pub seen_seasons: Vec<u32>,
    pub trophies: Vec<Trophy>,
}

impl SkinRecord {
    fn empty() -> Self {
        Self {
            wins: 0,
            heists_cracked: 0,
            times_caught: 0,
            seasons_played: 0,
            titled_seasons: Vec::new(),
            seen_seasons: Vec::new(),
            trophies: Vec::new(),
        }
    }
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct SkinToken {
    pub owner_id: AccountId,
    pub traits: SkinTraits,
    pub next_approval_id: u64,
    pub approvals: Vec<(AccountId, u64)>,
    /// Account the skin was minted for. The 2% creator share pays this account.
    pub creator: AccountId,
    /// Snapshotted at mint, in basis points. Default 200.
    pub creator_bps: u16,
    /// Platform account snapshotted at mint. Default is the treasury passed to `new`.
    pub platform: AccountId,
    /// Snapshotted at mint, in basis points. Default 300.
    pub platform_bps: u16,
    /// Extra royalty accounts snapshotted at mint. Empty unless the owner sets a future split.
    pub royalty_splits: Vec<(AccountId, u16)>,
    pub bound_agent: Option<AccountId>,
    pub stats: SkinRecord,
    /// Earned trait ids currently worn. One per slot.
    pub equipped_traits: Vec<String>,
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
pub struct SkinStatsView {
    pub wins: u32,
    pub heists_cracked: u32,
    pub times_caught: u32,
    pub seasons_played: u32,
    pub season_titles: u32,
    pub trophies: Vec<Trophy>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct UnlockedTraitView {
    pub id: String,
    pub slot: String,
    pub name: String,
    pub unlocked: bool,
    pub progress: u32,
    pub goal: u32,
    pub equipped: bool,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct SkinForge {
    owner: AccountId,
    treasury: AccountId,
    registry: AccountId,
    prize_pot: Option<AccountId>,
    /// Share of the post-storage proceeds, in basis points. Default 0.
    prize_bps: u16,
    mint_price: u128,
    /// Creator share for tokens minted from now on, in basis points. Default 2%.
    creator_bps: u16,
    /// Platform share for tokens minted from now on, in basis points. Default 3%.
    platform_bps: u16,
    /// Account that receives the platform share. Owner-set. Not a key.
    platform: AccountId,
    /// Extra accounts for future mints. Empty. Counted inside the same 10% cap.
    royalty_splits: Vec<(AccountId, u16)>,
    /// Prefix for `media` and `reference`. Includes the trailing slash.
    base_uri: String,
    next_id: u64,
    tokens: LookupMap<String, SkinToken>,
    by_owner: LookupMap<AccountId, Vec<String>>,
    supply_ids: Vec<String>,
    equipped: LookupMap<AccountId, String>,
    /// Game or registry accounts allowed to write trophies. The owner is not included.
    recorders: LookupSet<AccountId>,
}

#[near]
impl SkinForge {
    #[init]
    pub fn new(owner: AccountId, treasury: AccountId, registry: AccountId) -> Self {
        let platform = treasury.clone();
        Self {
            owner,
            treasury,
            registry,
            prize_pot: None,
            prize_bps: 0,
            mint_price: MINT_PRICE_YOCTO,
            creator_bps: DEFAULT_CREATOR_BPS,
            platform_bps: DEFAULT_PLATFORM_BPS,
            platform,
            royalty_splits: Vec::new(),
            base_uri: DEFAULT_BASE_URI.into(),
            next_id: 1,
            tokens: LookupMap::new(b"t"),
            by_owner: LookupMap::new(b"o"),
            supply_ids: Vec::new(),
            equipped: LookupMap::new(b"e"),
            recorders: LookupSet::new(b"c"),
        }
    }

    pub fn nft_metadata(&self) -> NftContractMetadata {
        NftContractMetadata {
            spec: "nft-1.0.0".into(),
            name: "fe2VI Skins".into(),
            symbol: "FE2VI".into(),
            icon: Some(media::collection_icon()),
            base_uri: Some(self.base_uri.clone()),
            reference: None,
            reference_hash: None,
        }
    }

    pub fn mint_price(&self) -> U128 {
        U128(self.mint_price)
    }

    /// Free, deterministic, not stored. An agent with nothing equipped wears this.
    pub fn default_skin(&self, agent: AccountId) -> SkinTraits {
        catalog_default(agent.as_str())
    }

    pub fn equipped_skin(&self, agent: AccountId) -> Option<String> {
        self.equipped.get(&agent).cloned()
    }

    pub fn set_mint_price(&mut self, price: U128) {
        self.only_owner();
        require!(
            price.0 > 0 && price.0 <= 5 * ONE_NEAR,
            "mint price out of range"
        );
        self.mint_price = price.0;
    }

    pub fn set_treasury(&mut self, treasury: AccountId) {
        self.only_owner();
        self.treasury = treasury;
    }

    pub fn set_prize_pot(&mut self, prize_pot: AccountId, prize_bps: u16) {
        self.only_owner();
        require!(prize_bps <= 10_000, "prize share");
        self.prize_pot = Some(prize_pot);
        self.prize_bps = prize_bps;
    }

    /// Creator share for later mints. A token keeps the rate it was minted with.
    pub fn set_creator_bps(&mut self, creator_bps: u16) {
        self.only_owner();
        require!(
            split_total(creator_bps, self.platform_bps, &self.royalty_splits)
                <= u32::from(MAX_ROYALTY_BPS),
            "royalty cap"
        );
        self.creator_bps = creator_bps;
    }

    pub fn creator_bps(&self) -> u16 {
        self.creator_bps
    }

    /// Platform share for later mints. Counted with the creator share inside the 10% cap.
    pub fn set_platform_bps(&mut self, platform_bps: u16) {
        self.only_owner();
        require!(
            split_total(self.creator_bps, platform_bps, &self.royalty_splits)
                <= u32::from(MAX_ROYALTY_BPS),
            "royalty cap"
        );
        self.platform_bps = platform_bps;
    }

    pub fn platform_bps(&self) -> u16 {
        self.platform_bps
    }

    /// Account that receives the platform share on later mints. No key is stored.
    pub fn set_platform(&mut self, platform: AccountId) {
        self.only_owner();
        require!(
            !self
                .royalty_splits
                .iter()
                .any(|(account, _)| account == &platform),
            "royalty split"
        );
        self.platform = platform;
    }

    pub fn platform_account(&self) -> AccountId {
        self.platform.clone()
    }

    /// Gateway for `{token_id}.png` and `{token_id}.json`. Must be `https://` and end in `/`.
    pub fn set_base_uri(&mut self, base_uri: String) {
        self.only_owner();
        require!(valid_base_uri(&base_uri), "base uri");
        self.base_uri = base_uri;
    }

    pub fn base_uri(&self) -> String {
        self.base_uri.clone()
    }

    /// Canonical preview bytes. Host this file at `{base_uri}{token_id}.png`.
    pub fn token_media(&self, token_id: String) -> near_sdk::json_types::Base64VecU8 {
        let token = self.token_or_panic(&token_id);
        Base64VecU8(media::preview_png(&token.traits))
    }

    /// Canonical reference document. Host this file at `{base_uri}{token_id}.json`.
    pub fn token_reference(&self, token_id: String) -> String {
        let token = self.token_or_panic(&token_id);
        reference_json(&token_id, &token)
    }

    /// Replace the extra royalty accounts used by later mints.
    /// The creator and the platform stay separate entries. This list starts empty.
    /// Creator plus platform plus these shares stay at or under 10%.
    pub fn set_royalty_splits(&mut self, splits: Vec<(AccountId, u16)>) {
        self.only_owner();
        let mut seen = BTreeSet::new();
        for (account, bps) in &splits {
            require!(*bps > 0 && *bps <= MAX_ROYALTY_BPS, "royalty split");
            require!(account != &self.platform, "royalty split");
            require!(seen.insert(account.clone()), "royalty split");
        }
        require!(
            split_total(self.creator_bps, self.platform_bps, &splits) <= u32::from(MAX_ROYALTY_BPS),
            "royalty cap"
        );
        self.royalty_splits = splits;
    }

    pub fn royalty_splits(&self) -> Vec<(AccountId, u16)> {
        self.royalty_splits.clone()
    }

    /// Allow or remove a game or registry account that may call `record_result`.
    pub fn set_recorder(&mut self, account_id: AccountId, allowed: bool) {
        self.only_owner();
        if allowed {
            self.recorders.insert(account_id);
        } else {
            self.recorders.remove(&account_id);
        }
    }

    /// Registry-only. Keeps exactly `mint_price` and refunds the rest.
    /// Storage staking is paid from that price, not on top of it.
    #[payable]
    pub fn mint_skin(&mut self, agent: AccountId, traits: SkinTraits) -> String {
        require!(
            env::predecessor_account_id() == self.registry,
            "registry pays the mint"
        );
        if let Some(reason) = unknown_trait(&traits) {
            env::panic_str(reason);
        }
        let deposit = env::attached_deposit().as_yoctonear();
        require!(deposit >= self.mint_price, "mint price");
        let refund = deposit - self.mint_price;
        let id_num = self.next_id;
        self.next_id += 1;
        let token_id = id_num.to_string();
        let before = env::storage_usage();
        self.tokens.insert(
            token_id.clone(),
            SkinToken {
                owner_id: agent.clone(),
                traits: traits.clone(),
                next_approval_id: 1,
                approvals: Vec::new(),
                creator: agent.clone(),
                creator_bps: self.creator_bps,
                platform: self.platform.clone(),
                platform_bps: self.platform_bps,
                royalty_splits: self.royalty_splits.clone(),
                bound_agent: None,
                stats: SkinRecord::empty(),
                equipped_traits: Vec::new(),
            },
        );
        let mut owned = self.by_owner.get(&agent).cloned().unwrap_or_default();
        owned.push(token_id.clone());
        self.by_owner.insert(agent.clone(), owned);
        self.supply_ids.push(token_id.clone());
        self.bind_internal(&token_id, &agent);
        let added = env::storage_usage().saturating_sub(before);
        let storage_cost = u128::from(added) * env::storage_byte_cost().as_yoctonear();
        require!(storage_cost < self.mint_price, "storage exceeds mint price");
        let proceeds = self.mint_price - storage_cost;
        let prize = proceeds * u128::from(self.prize_bps) / 10_000;
        let to_treasury = proceeds - prize;
        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(NearToken::from_yoctonear(refund));
        }
        if prize > 0 {
            if let Some(pot) = self.prize_pot.clone() {
                Promise::new(pot).transfer(NearToken::from_yoctonear(prize));
            }
        }
        if to_treasury > 0 {
            Promise::new(self.treasury.clone()).transfer(NearToken::from_yoctonear(to_treasury));
        }
        env::log_str(&nft_event(
            "nft_mint",
            &near_sdk::serde_json::json!({
                "owner_id": agent,
                "token_ids": [token_id],
            }),
        ));
        token_id
    }

    /// Bind the token to the caller's own account. Prefer `bind_skin` to wear it on an agent.
    #[payable]
    pub fn equip_skin(&mut self, token_id: String) {
        let caller = env::predecessor_account_id();
        self.bind_skin(token_id, caller);
    }

    /// The owner wears this skin on `agent_id`. Transfer clears the link.
    /// An agent wears one skin; binding replaces the one already worn.
    #[payable]
    pub fn bind_skin(&mut self, token_id: String, agent_id: AccountId) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let caller = env::predecessor_account_id();
        let token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        require!(token.owner_id == caller, "cannot equip unowned skin");
        let previous = token.bound_agent.clone();
        self.bind_internal(&token_id, &agent_id);
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "fe2vi",
                "version": "1.0.0",
                "event": "skin_bound",
                "data": [{ "owner": caller, "agent": agent_id, "token_id": token_id, "previous_agent": previous }]
            })
        ));
    }

    pub fn token_agent(&self, token_id: String) -> Option<AccountId> {
        self.tokens
            .get(&token_id)
            .and_then(|token| token.bound_agent.clone())
    }

    /// Allowlisted game or registry only. The owner cannot write trophies.
    /// Counters and the trophy log live on the token, so a sale keeps the history.
    pub fn record_result(&mut self, token_id: String, game: String, outcome: String, season: u32) {
        let caller = env::predecessor_account_id();
        require!(self.recorders.contains(&caller), "recorder");
        require!(valid_game(&game), "game");
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        apply_outcome(&mut token.stats, &game, &outcome, season);
        self.tokens.insert(token_id.clone(), token);
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "fe2vi",
                "version": "1.0.0",
                "event": "skin_result",
                "data": [{ "token_id": token_id, "game": game, "outcome": outcome, "season": season }]
            })
        ));
    }

    /// Wear an earned trait that the token has already unlocked. Mint cannot grant these.
    #[payable]
    pub fn equip_trait(&mut self, token_id: String, trait_id: String) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let spec = earned_trait(&trait_id).unwrap_or_else(|| env::panic_str("unknown trait"));
        let caller = env::predecessor_account_id();
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        require!(token.owner_id == caller, "not owner");
        let titles = token.stats.titled_seasons.len() as u32;
        let (_, _, unlocked) =
            earned_progress(spec, token.stats.wins, token.stats.heists_cracked, titles);
        require!(unlocked, "trait locked");
        token.equipped_traits.retain(|id| {
            earned_trait(id)
                .map(|other| other.slot != spec.slot)
                .unwrap_or(false)
        });
        token.equipped_traits.push(trait_id);
        self.tokens.insert(token_id, token);
    }

    #[payable]
    pub fn clear_trait(&mut self, token_id: String, trait_id: String) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let caller = env::predecessor_account_id();
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        require!(token.owner_id == caller, "not owner");
        token.equipped_traits.retain(|id| id != &trait_id);
        self.tokens.insert(token_id, token);
    }

    pub fn skin_stats(&self, token_id: String) -> SkinStatsView {
        let token = self
            .tokens
            .get(&token_id)
            .unwrap_or_else(|| env::panic_str("no token"));
        SkinStatsView {
            wins: token.stats.wins,
            heists_cracked: token.stats.heists_cracked,
            times_caught: token.stats.times_caught,
            seasons_played: token.stats.seasons_played,
            season_titles: token.stats.titled_seasons.len() as u32,
            trophies: token.stats.trophies.clone(),
        }
    }

    pub fn unlocked_traits(&self, token_id: String) -> Vec<UnlockedTraitView> {
        let token = self
            .tokens
            .get(&token_id)
            .unwrap_or_else(|| env::panic_str("no token"));
        let titles = token.stats.titled_seasons.len() as u32;
        EARNED_TRAITS
            .iter()
            .map(|spec| {
                let (progress, goal, unlocked) =
                    earned_progress(spec, token.stats.wins, token.stats.heists_cracked, titles);
                UnlockedTraitView {
                    id: spec.id.into(),
                    slot: spec.slot.into(),
                    name: spec.name.into(),
                    unlocked,
                    progress,
                    goal,
                    equipped: token.equipped_traits.iter().any(|id| id == spec.id),
                }
            })
            .collect()
    }

    /// NEP-199. Integer division; the seller keeps the remainder so the map sums to `balance`.
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
        let payout = payout_map(self.token_or_panic(&token_id), balance.0, max_len_payout);
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

    pub fn nft_token(&self, token_id: String) -> Option<JsonToken> {
        self.tokens.get(&token_id).map(|token| {
            let metadata = self.metadata_for(&token_id, token);
            JsonToken {
                token_id,
                owner_id: token.owner_id.clone(),
                metadata,
            }
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
        let start = from_index.map(|v| v.0 as usize).unwrap_or(0);
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
        let start = from_index.map(|v| v.0 as usize).unwrap_or(0);
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

    /// NEP-171. The receiver's `nft_on_transfer` may return true to send the token back.
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
                ext_forge_self::ext(env::current_account_id())
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
        self.transfer_internal(&receiver_id, &previous_owner, &token_id, None, None);
        false
    }

    /// NEP-178. A `msg` is forwarded to `nft_on_approve` on `account_id`.
    /// Paras and near.cooking list a sale with
    /// `{"market_type":"sale","ft_token_id":"near","price":"<yocto>"}`.
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
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        require!(token.owner_id == owner, "not owner");
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
            &near_sdk::serde_json::json!({ "token_id": token_id, "account_id": account_id, "approval_id": approval_id }),
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
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        require!(token.owner_id == owner, "not owner");
        token.approvals.retain(|(id, _)| id != &account_id);
        self.tokens.insert(token_id, token);
    }

    #[payable]
    pub fn nft_revoke_all(&mut self, token_id: String) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let owner = env::predecessor_account_id();
        let mut token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
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

impl SkinForge {
    fn only_owner(&self) {
        require!(env::predecessor_account_id() == self.owner, "owner");
    }

    fn token_or_panic(&self, token_id: &str) -> SkinToken {
        self.tokens
            .get(&token_id.to_string())
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"))
    }

    fn bind_internal(&mut self, token_id: &str, agent: &AccountId) {
        if let Some(other) = self.equipped.get(agent).cloned() {
            if other != token_id {
                let mut worn = self.token_or_panic(&other);
                if worn.bound_agent.as_ref() == Some(agent) {
                    worn.bound_agent = None;
                    self.tokens.insert(other, worn);
                }
            }
        }
        let mut token = self.token_or_panic(token_id);
        if let Some(previous) = token.bound_agent.clone() {
            if previous != *agent
                && self.equipped.get(&previous).map(String::as_str) == Some(token_id)
            {
                self.equipped.remove(&previous);
            }
        }
        token.bound_agent = Some(agent.clone());
        self.tokens.insert(token_id.to_string(), token);
        self.equipped.insert(agent.clone(), token_id.to_string());
    }

    fn unbind_token(&mut self, token: &mut SkinToken, token_id: &str) {
        let Some(agent) = token.bound_agent.take() else {
            return;
        };
        if self.equipped.get(&agent).map(String::as_str) == Some(token_id) {
            self.equipped.remove(&agent);
        }
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "fe2vi",
                "version": "1.0.0",
                "event": "skin_unbound",
                "data": [{ "agent": agent, "token_id": token_id }]
            })
        ));
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
        self.unbind_token(&mut token, token_id);
        let mut from_list = self.by_owner.get(&previous).cloned().unwrap_or_default();
        from_list.retain(|id| id != token_id);
        self.by_owner.insert(previous.clone(), from_list);
        token.owner_id = receiver.clone();
        token.approvals.clear();
        self.tokens.insert(token_id.to_string(), token);
        let mut to_list = self.by_owner.get(receiver).cloned().unwrap_or_default();
        to_list.push(token_id.to_string());
        self.by_owner.insert(receiver.clone(), to_list);
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
}

fn valid_base_uri(uri: &str) -> bool {
    (12..=160).contains(&uri.len())
        && uri.starts_with("https://")
        && uri.ends_with('/')
        && !uri.contains(' ')
}

fn reference_json(token_id: &str, token: &SkinToken) -> String {
    let mut attributes = Vec::new();
    for (trait_type, value) in media::trait_rows(&token.traits) {
        attributes.push(near_sdk::serde_json::json!({ "trait_type": trait_type, "value": value }));
    }
    let stats = &token.stats;
    for (trait_type, value) in [
        ("Wins", stats.wins),
        ("Heists cracked", stats.heists_cracked),
        ("Times caught", stats.times_caught),
        ("Seasons played", stats.seasons_played),
        ("Season titles", stats.titled_seasons.len() as u32),
    ] {
        attributes.push(
            near_sdk::serde_json::json!({ "trait_type": trait_type, "value": value.to_string() }),
        );
    }
    let trophies: Vec<_> = stats
        .trophies
        .iter()
        .map(|trophy| {
            near_sdk::serde_json::json!({
                "game": trophy.game,
                "outcome": trophy.outcome,
                "season": trophy.season,
            })
        })
        .collect();
    near_sdk::serde_json::to_string(&near_sdk::serde_json::json!({
        "name": format!("fe2VI skin {token_id}"),
        "attributes": attributes,
        "equipped": token.equipped_traits,
        "stats": {
            "wins": stats.wins,
            "heists_cracked": stats.heists_cracked,
            "times_caught": stats.times_caught,
            "seasons_played": stats.seasons_played,
            "season_titles": stats.titled_seasons.len() as u32,
        },
        "trophies": trophies,
    }))
    .unwrap_or_else(|_| "{}".into())
}

impl SkinForge {
    fn metadata_for(&self, token_id: &str, token: &SkinToken) -> TokenMetadata {
        let document = reference_json(token_id, token);
        let png = media::preview_png(&token.traits);
        TokenMetadata {
            title: Some(format!("fe2VI skin {token_id}")),
            description: Some("Cosmetic only. No gameplay effect.".into()),
            media: Some(format!("{token_id}.png")),
            media_hash: Some(Base64VecU8(env::sha256(&png))),
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

fn split_total(creator_bps: u16, platform_bps: u16, splits: &[(AccountId, u16)]) -> u32 {
    u32::from(creator_bps)
        + u32::from(platform_bps)
        + splits.iter().map(|(_, bps)| u32::from(*bps)).sum::<u32>()
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

fn payout_map(token: SkinToken, balance: u128, max_len_payout: u32) -> BTreeMap<AccountId, U128> {
    require!(max_len_payout > 0, "payout exceeds max_len");
    let mut payout: BTreeMap<AccountId, u128> = BTreeMap::new();
    let mut paid = 0u128;
    if token.creator != token.owner_id {
        let cut = balance.saturating_mul(u128::from(token.creator_bps)) / 10_000;
        add_cut(&mut payout, &token.creator, cut, &mut paid);
    }
    if token.platform != token.owner_id {
        let cut = balance.saturating_mul(u128::from(token.platform_bps)) / 10_000;
        add_cut(&mut payout, &token.platform, cut, &mut paid);
    }
    for (account, bps) in &token.royalty_splits {
        if account == &token.owner_id {
            continue;
        }
        let cut = balance.saturating_mul(u128::from(*bps)) / 10_000;
        add_cut(&mut payout, account, cut, &mut paid);
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

fn apply_outcome(stats: &mut SkinRecord, game: &str, outcome: &str, season: u32) {
    if outcome == "win" {
        stats.wins = stats.wins.saturating_add(1);
    } else if outcome == "heist_cracked" {
        stats.heists_cracked = stats.heists_cracked.saturating_add(1);
    } else if outcome == "caught" {
        stats.times_caught = stats.times_caught.saturating_add(1);
    } else if let Some(rank) = outcome.strip_prefix("rank:") {
        let rank: u32 = rank.parse().unwrap_or_else(|_| env::panic_str("rank"));
        require!(rank >= 1, "rank");
        if rank == 1 && !stats.titled_seasons.contains(&season) {
            stats.titled_seasons.push(season);
        }
    } else {
        env::panic_str("outcome");
    }
    if !stats.seen_seasons.contains(&season) {
        stats.seen_seasons.push(season);
        stats.seasons_played = stats.seasons_played.saturating_add(1);
    }
    if stats.trophies.len() >= TROPHY_CAP {
        stats.trophies.remove(0);
    }
    stats.trophies.push(Trophy {
        game: game.to_string(),
        outcome: outcome.to_string(),
        season,
    });
}

fn valid_game(game: &str) -> bool {
    let bytes = game.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
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
#[ext_contract(ext_forge_self)]
trait ForgeResolve {
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
    use fe2vi_core::HEADWEAR;
    use near_sdk::test_utils::{get_created_receipts, get_logs, VMContextBuilder};
    use near_sdk::{testing_env, Gas, NearToken};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn context(predecessor: &str, deposit: u128) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("forge.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(predecessor.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(50))
            .prepaid_gas(Gas::from_tgas(300));
        testing_env!(builder.build());
    }

    fn boot() -> SkinForge {
        context("owner.fe2vi.testnet", 0);
        SkinForge::new(
            "owner.fe2vi.testnet".parse().unwrap(),
            "treasury.fe2vi.testnet".parse().unwrap(),
            "registry.fe2vi.testnet".parse().unwrap(),
        )
    }

    fn sample() -> SkinTraits {
        fe2vi_core::default_skin("alpha.registry.fe2vi.testnet")
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

    #[test]
    fn mint_requires_exact_price() {
        let mut forge = boot();
        let agent: AccountId = "alpha.registry.fe2vi.testnet".parse().unwrap();
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO - 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.mint_skin(agent.clone(), sample());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("mint price"));
        assert_eq!(forge.nft_total_supply().0, 0);

        context("alice.fe2vi.testnet", MINT_PRICE_YOCTO);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.mint_skin(agent.clone(), sample());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("registry pays the mint"));

        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let id = forge.mint_skin(agent.clone(), sample());
        assert_eq!(id, "1");
        assert_eq!(forge.nft_token(id).unwrap().owner_id, agent);
        let refunds = refunds_to("registry.fe2vi.testnet");
        assert!(refunds.is_empty(), "exact price is not refunded");
    }

    #[test]
    fn overpay_refunded() {
        let mut forge = boot();
        let agent: AccountId = "alpha.registry.fe2vi.testnet".parse().unwrap();
        let extra = 2 * ONE_NEAR;
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO + extra);
        forge.mint_skin(agent, sample());
        let refunds = refunds_to("registry.fe2vi.testnet");
        assert_eq!(refunds, vec![extra]);
    }

    #[test]
    fn unknown_trait_rejected() {
        let mut forge = boot();
        let agent: AccountId = "alpha.registry.fe2vi.testnet".parse().unwrap();
        let mut traits = sample();
        traits.headwear = HEADWEAR;
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.mint_skin(agent, traits);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("unknown trait"));
        assert_eq!(forge.nft_total_supply().0, 0);
    }

    #[test]
    fn cannot_equip_unowned_skin() {
        let mut forge = boot();
        let agent: AccountId = "alpha.registry.fe2vi.testnet".parse().unwrap();
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let id = forge.mint_skin(agent, sample());
        context("bob.fe2vi.testnet", 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.equip_skin(id);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("cannot equip unowned skin"));
    }

    fn minted() -> (SkinForge, String, AccountId) {
        let mut forge = boot();
        let agent: AccountId = "alpha.registry.fe2vi.testnet".parse().unwrap();
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let id = forge.mint_skin(agent.clone(), sample());
        (forge, id, agent)
    }

    #[test]
    fn transfer_unbinds_and_emits_nep297() {
        let (mut forge, id, agent) = minted();
        assert_eq!(
            forge.equipped_skin(agent.clone()).as_deref(),
            Some(id.as_str())
        );
        let bob: AccountId = "bob.fe2vi.testnet".parse().unwrap();
        context(agent.as_str(), 1);
        forge.nft_transfer(bob.clone(), id.clone(), None, Some("desk".into()));
        assert!(forge.equipped_skin(agent.clone()).is_none());
        assert!(forge.token_agent(id.clone()).is_none());
        assert_eq!(forge.nft_token(id.clone()).unwrap().owner_id, bob);
        let logs = get_logs().join("\n");
        assert!(logs.contains("\"standard\":\"nep171\""));
        assert!(logs.contains("\"event\":\"nft_transfer\""));
        assert!(logs.contains("skin_unbound"));
        assert!(logs.contains("desk"));
        context(bob.as_str(), 1);
        let wearer: AccountId = "bob.agent.fe2vi.testnet".parse().unwrap();
        forge.bind_skin(id.clone(), wearer.clone());
        assert_eq!(
            forge.equipped_skin(wearer.clone()).as_deref(),
            Some(id.as_str())
        );
        assert_eq!(forge.token_agent(id).as_ref(), Some(&wearer));
    }

    #[test]
    fn approve_then_transfer_clears_approval() {
        let (mut forge, id, agent) = minted();
        let market: AccountId = "market.fe2vi.testnet".parse().unwrap();
        context(agent.as_str(), ONE_NEAR);
        forge.nft_approve(id.clone(), market.clone(), None);
        assert!(forge.nft_is_approved(id.clone(), market.clone(), Some(1)));
        let buyer: AccountId = "buyer.fe2vi.testnet".parse().unwrap();
        context(market.as_str(), 1);
        forge.nft_transfer(buyer.clone(), id.clone(), Some(1), None);
        assert!(!forge.nft_is_approved(id.clone(), market, Some(1)));
        assert_eq!(forge.nft_token(id).unwrap().owner_id, buyer);
        context("market.fe2vi.testnet", 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.nft_transfer(agent, "1".into(), Some(1), None);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("not approved"));
    }

    #[test]
    fn payout_pays_creator_and_platform_and_rounds_down() {
        let (mut forge, id, agent) = minted();
        let treasury: AccountId = "treasury.fe2vi.testnet".parse().unwrap();
        assert_eq!(forge.platform_account(), treasury);
        assert_eq!(forge.creator_bps(), 200);
        assert_eq!(forge.platform_bps(), 300);

        let held = forge.nft_payout(id.clone(), U128(100), 3);
        assert_eq!(held.payout.get(&treasury).unwrap().0, 3);
        assert_eq!(held.payout.get(&agent).unwrap().0, 97);
        assert_eq!(held.payout.len(), 2);

        let bob: AccountId = "bob.fe2vi.testnet".parse().unwrap();
        context(agent.as_str(), 1);
        forge.nft_transfer(bob.clone(), id.clone(), None, None);

        let full = forge.nft_payout(id.clone(), U128(100), 3);
        assert_eq!(full.payout.get(&agent).unwrap().0, 2);
        assert_eq!(full.payout.get(&treasury).unwrap().0, 3);
        assert_eq!(full.payout.get(&bob).unwrap().0, 95);
        assert_eq!(full.payout.values().map(|value| value.0).sum::<u128>(), 100);

        let uneven = forge.nft_payout(id.clone(), U128(40), 3);
        assert_eq!(uneven.payout.get(&agent), None);
        assert_eq!(uneven.payout.get(&treasury).unwrap().0, 1);
        assert_eq!(uneven.payout.get(&bob).unwrap().0, 39);

        let ninety_nine = forge.nft_payout(id.clone(), U128(99), 3);
        assert_eq!(ninety_nine.payout.get(&agent).unwrap().0, 1);
        assert_eq!(ninety_nine.payout.get(&treasury).unwrap().0, 2);
        assert_eq!(ninety_nine.payout.get(&bob).unwrap().0, 96);
        assert_eq!(
            ninety_nine
                .payout
                .values()
                .map(|value| value.0)
                .sum::<u128>(),
            99
        );

        let dust = forge.nft_payout(id.clone(), U128(3), 3);
        assert_eq!(dust.payout.len(), 1);
        assert_eq!(dust.payout.get(&bob).unwrap().0, 3);

        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.nft_payout(id.clone(), U128(100), 2);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("payout exceeds max_len"));

        let desk: AccountId = "desk.fe2vi.testnet".parse().unwrap();
        context("alice.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| forge.set_platform(desk.clone()))).unwrap_err();
        assert!(panic_text(err).contains("owner"));
        let err = catch_unwind(AssertUnwindSafe(|| forge.set_creator_bps(1_001))).unwrap_err();
        assert!(panic_text(err).contains("owner"));

        context("owner.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| forge.set_creator_bps(800))).unwrap_err();
        assert!(panic_text(err).contains("royalty cap"));
        let err = catch_unwind(AssertUnwindSafe(|| forge.set_platform_bps(900))).unwrap_err();
        assert!(panic_text(err).contains("royalty cap"));
        forge.set_platform(desk.clone());
        assert_eq!(
            forge
                .nft_payout(id.clone(), U128(100), 3)
                .payout
                .get(&treasury)
                .unwrap()
                .0,
            3
        );
        assert_eq!(forge.nft_payout(id, U128(100), 3).payout.get(&desk), None);

        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let second = forge.mint_skin(agent.clone(), sample());
        context(agent.as_str(), 1);
        let carol: AccountId = "carol.fe2vi.testnet".parse().unwrap();
        forge.nft_transfer(carol.clone(), second.clone(), None, None);
        let next = forge.nft_payout(second, U128(100), 3);
        assert_eq!(next.payout.get(&agent).unwrap().0, 2);
        assert_eq!(next.payout.get(&desk).unwrap().0, 3);
        assert_eq!(next.payout.get(&carol).unwrap().0, 95);
        assert_eq!(next.payout.get(&treasury), None);
    }

    #[test]
    fn royalty_splits_snapshot_and_stay_inside_the_cap() {
        let (mut forge, first, agent) = minted();
        let treasury: AccountId = "treasury.fe2vi.testnet".parse().unwrap();
        assert!(forge.royalty_splits().is_empty());
        let partner: AccountId = "partner.fe2vi.testnet".parse().unwrap();
        context("alice.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.set_royalty_splits(vec![(partner.clone(), 200)]);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("owner"));

        context("owner.fe2vi.testnet", 0);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.set_royalty_splits(vec![(partner.clone(), 600)]);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("royalty cap"));
        forge.set_royalty_splits(vec![(partner.clone(), 200)]);
        let err = catch_unwind(AssertUnwindSafe(|| forge.set_creator_bps(600))).unwrap_err();
        assert!(panic_text(err).contains("royalty cap"));

        let bob: AccountId = "bob.fe2vi.testnet".parse().unwrap();
        context(agent.as_str(), 1);
        forge.nft_transfer(bob.clone(), first.clone(), None, None);
        let old = forge.nft_payout(first, U128(10_000), 4);
        assert_eq!(old.payout.get(&partner), None);
        assert_eq!(old.payout.get(&agent).unwrap().0, 200);
        assert_eq!(old.payout.get(&treasury).unwrap().0, 300);
        assert_eq!(old.payout.get(&bob).unwrap().0, 9_500);

        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let second = forge.mint_skin(agent.clone(), sample());
        context(agent.as_str(), 1);
        forge.nft_transfer(bob.clone(), second.clone(), None, None);
        let split = forge.nft_payout(second, U128(10_000), 4);
        assert_eq!(split.payout.get(&agent).unwrap().0, 200);
        assert_eq!(split.payout.get(&treasury).unwrap().0, 300);
        assert_eq!(split.payout.get(&partner).unwrap().0, 200);
        assert_eq!(split.payout.get(&bob).unwrap().0, 9_300);
        assert_eq!(
            split.payout.values().map(|value| value.0).sum::<u128>(),
            10_000
        );
    }

    #[test]
    fn collection_metadata_matches_nep177() {
        let (forge, id, _) = minted();
        let meta = forge.nft_metadata();
        assert_eq!(meta.spec, "nft-1.0.0");
        assert_eq!(meta.name, "fe2VI Skins");
        assert_eq!(meta.symbol, "FE2VI");
        assert!(meta
            .icon
            .as_deref()
            .unwrap_or("")
            .starts_with("data:image/png;base64,"));
        assert_eq!(meta.base_uri.as_deref(), Some(DEFAULT_BASE_URI));
        assert!(meta.reference.is_none());
        assert!(meta.reference_hash.is_none());
        let token = forge.nft_token(id.clone()).unwrap();
        let png = forge.token_media(id.clone()).0;
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            token.metadata.media_hash.clone().unwrap().0,
            near_sdk::env::sha256(&png)
        );
        let document = forge.token_reference(id);
        assert_eq!(token.metadata.extra.as_deref(), Some(document.as_str()));
        assert!(document.contains("\"trait_type\":\"Paint\""));
        assert!(document.contains("\"trait_type\":\"Wins\""));
        assert!(document.contains("\"trophies\":[]"));
        assert_eq!(
            token.metadata.reference_hash.clone().unwrap().0,
            near_sdk::env::sha256(document.as_bytes())
        );
        assert_eq!(token.metadata.media.as_deref(), Some("1.png"));
        assert_eq!(token.metadata.reference.as_deref(), Some("1.json"));
        let json = near_sdk::serde_json::to_value(token.metadata).unwrap();
        for key in [
            "title",
            "description",
            "media",
            "media_hash",
            "copies",
            "issued_at",
            "expires_at",
            "starts_at",
            "updated_at",
            "extra",
            "reference",
            "reference_hash",
        ] {
            assert!(json.get(key).is_some(), "{key}");
        }
        assert_eq!(json["title"], "fe2VI skin 1");
        assert!(json["media_hash"].is_string());
        assert_eq!(json["reference"], "1.json");
    }

    #[test]
    fn approve_forwards_the_paras_sale_msg() {
        let (mut forge, id, agent) = minted();
        let market: AccountId = "market.joespano.testnet".parse().unwrap();
        let msg = "{\"market_type\":\"sale\",\"ft_token_id\":\"near\",\"price\":\"1000000000000000000000000\"}";
        context(agent.as_str(), ONE_NEAR);
        forge.nft_approve(id.clone(), market.clone(), Some(msg.into()));
        assert!(forge.nft_is_approved(id, market.clone(), Some(1)));
        let forwarded = get_created_receipts().into_iter().any(|receipt| {
            receipt.receiver_id == market
                && receipt.actions.iter().any(|action| match action {
                    near_sdk::mock::MockAction::FunctionCallWeight {
                        method_name, args, ..
                    } => {
                        method_name == b"nft_on_approve"
                            && String::from_utf8_lossy(args).contains("market_type")
                            && String::from_utf8_lossy(args).contains("sale")
                    }
                    _ => false,
                })
        });
        assert!(forwarded, "nft_on_approve was not scheduled");
    }

    #[test]
    fn trophies_unlock_traits_and_owner_cannot_write_them() {
        let (mut forge, id, agent) = minted();
        context(agent.as_str(), 0);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.record_result(id.clone(), "heist-night".into(), "win".into(), 1);
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("recorder"));

        context("owner.fe2vi.testnet", 0);
        let arena: AccountId = "heist.fe2vi.testnet".parse().unwrap();
        forge.set_recorder(arena.clone(), true);
        context(arena.as_str(), 0);
        for _ in 0..7 {
            forge.record_result(id.clone(), "heist-night".into(), "win".into(), 1);
        }
        forge.record_result(id.clone(), "heist-night".into(), "heist_cracked".into(), 1);
        forge.record_result(id.clone(), "heist-night".into(), "caught".into(), 1);
        forge.record_result(id.clone(), "heist-night".into(), "rank:2".into(), 2);
        let stats = forge.skin_stats(id.clone());
        assert_eq!(stats.wins, 7);
        assert_eq!(stats.heists_cracked, 1);
        assert_eq!(stats.times_caught, 1);
        assert_eq!(stats.seasons_played, 2);
        assert_eq!(stats.season_titles, 0);
        let locked = forge.unlocked_traits(id.clone());
        let gold = locked.iter().find(|entry| entry.id == "gold-leaf").unwrap();
        assert!(!gold.unlocked);
        assert_eq!((gold.progress, gold.goal), (7, 10));

        for _ in 0..3 {
            forge.record_result(id.clone(), "heist-night".into(), "win".into(), 1);
        }
        for _ in 0..4 {
            forge.record_result(id.clone(), "heist-night".into(), "heist_cracked".into(), 3);
        }
        forge.record_result(id.clone(), "heist-night".into(), "rank:1".into(), 4);
        let open = forge.unlocked_traits(id.clone());
        assert!(open
            .iter()
            .any(|entry| entry.id == "gold-leaf" && entry.unlocked));
        assert!(open
            .iter()
            .any(|entry| entry.id == "cracked-vault" && entry.unlocked));
        assert!(open
            .iter()
            .any(|entry| entry.id == "season-crown" && entry.unlocked));
        assert!(open
            .iter()
            .any(|entry| entry.id == "molten-trail" && !entry.unlocked && entry.progress == 10));
        let stats = forge.skin_stats(id.clone());
        assert_eq!(stats.wins, 10);
        assert_eq!(stats.heists_cracked, 5);
        assert_eq!(stats.season_titles, 1);
        assert_eq!(stats.seasons_played, 4);

        context("bob.fe2vi.testnet", 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.equip_trait(id.clone(), "gold-leaf".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("not owner"));
        context(agent.as_str(), 1);
        let err = catch_unwind(AssertUnwindSafe(|| {
            forge.equip_trait(id.clone(), "molten-trail".into());
        }))
        .unwrap_err();
        assert!(panic_text(err).contains("trait locked"));
        forge.equip_trait(id.clone(), "gold-leaf".into());
        forge.equip_trait(id.clone(), "season-crown".into());
        let worn = forge.unlocked_traits(id.clone());
        assert!(worn
            .iter()
            .any(|entry| entry.id == "gold-leaf" && entry.equipped));
        assert!(worn
            .iter()
            .any(|entry| entry.id == "season-crown" && entry.equipped));

        let bob: AccountId = "bob.fe2vi.testnet".parse().unwrap();
        forge.nft_transfer(bob.clone(), id.clone(), None, None);
        assert!(forge.equipped_skin(agent).is_none());
        context(bob.as_str(), 1);
        let next: AccountId = "bob.agent.fe2vi.testnet".parse().unwrap();
        forge.bind_skin(id.clone(), next.clone());
        assert_eq!(forge.token_agent(id.clone()).as_ref(), Some(&next));
        assert!(forge
            .unlocked_traits(id)
            .iter()
            .any(|entry| entry.id == "gold-leaf" && entry.equipped));
    }

    #[test]
    fn one_agent_wears_one_skin() {
        let (mut forge, first, agent) = minted();
        context("registry.fe2vi.testnet", MINT_PRICE_YOCTO);
        let second = forge.mint_skin(agent.clone(), sample());
        assert_eq!(
            forge.equipped_skin(agent.clone()).as_deref(),
            Some(second.as_str())
        );
        assert!(forge.token_agent(first.clone()).is_none());
        context(agent.as_str(), 1);
        forge.bind_skin(first.clone(), agent.clone());
        assert_eq!(forge.equipped_skin(agent).as_deref(), Some(first.as_str()));
        assert!(forge.token_agent(second).is_none());
    }

    #[test]
    fn default_skin_free() {
        let forge = boot();
        let agent: AccountId = "scout.vault.fe2vi.testnet".parse().unwrap();
        context("anyone.fe2vi.testnet", 0);
        let look = forge.default_skin(agent.clone());
        assert_eq!(look, catalog_default(agent.as_str()));
        assert!(unknown_trait(&look).is_none());
        assert_eq!(forge.nft_total_supply().0, 0);
        assert!(forge.equipped_skin(agent).is_none());
    }

    fn refunds_to(account: &str) -> Vec<u128> {
        let want: AccountId = account.parse().unwrap();
        let mut found = Vec::new();
        for receipt in get_created_receipts() {
            if receipt.receiver_id != want {
                continue;
            }
            for action in receipt.actions {
                if let near_sdk::mock::MockAction::Transfer { deposit, .. } = action {
                    found.push(deposit.as_yoctonear());
                }
            }
        }
        found
    }
}

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

use fe2vi_core::{
    default_skin as catalog_default, unknown_trait, SkinTraits, MINT_PRICE_YOCTO, ONE_NEAR,
};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, NearToken, PanicOnDefault, Promise};

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct SkinToken {
    pub owner_id: AccountId,
    pub traits: SkinTraits,
    pub next_approval_id: u64,
    pub approvals: Vec<(AccountId, u64)>,
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
    pub reference_hash: Option<String>,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenMetadata {
    pub title: String,
    pub description: String,
    pub media: String,
    pub copies: u64,
    pub extra: String,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct JsonToken {
    pub token_id: String,
    pub owner_id: AccountId,
    pub metadata: TokenMetadata,
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
    next_id: u64,
    tokens: LookupMap<String, SkinToken>,
    by_owner: LookupMap<AccountId, Vec<String>>,
    supply_ids: Vec<String>,
    equipped: LookupMap<AccountId, String>,
}

#[near]
impl SkinForge {
    #[init]
    pub fn new(owner: AccountId, treasury: AccountId, registry: AccountId) -> Self {
        Self {
            owner,
            treasury,
            registry,
            prize_pot: None,
            prize_bps: 0,
            mint_price: MINT_PRICE_YOCTO,
            next_id: 1,
            tokens: LookupMap::new(b"t"),
            by_owner: LookupMap::new(b"o"),
            supply_ids: Vec::new(),
            equipped: LookupMap::new(b"e"),
        }
    }

    pub fn nft_metadata(&self) -> NftContractMetadata {
        NftContractMetadata {
            spec: "nft-1.0.0".into(),
            name: "fe2VI Skins".into(),
            symbol: "IRONSKIN".into(),
            icon: None,
            base_uri: None,
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
            },
        );
        let mut owned = self.by_owner.get(&agent).cloned().unwrap_or_default();
        owned.push(token_id.clone());
        self.by_owner.insert(agent.clone(), owned);
        self.supply_ids.push(token_id.clone());
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

    /// Cosmetic slot. The caller must own the token. One skin per agent.
    #[payable]
    pub fn equip_skin(&mut self, token_id: String) {
        require!(env::attached_deposit().as_yoctonear() == 1, "1 yocto");
        let token = self
            .tokens
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        let caller = env::predecessor_account_id();
        require!(token.owner_id == caller, "cannot equip unowned skin");
        self.equipped.insert(caller.clone(), token_id.clone());
        env::log_str(&format!(
            "EVENT_JSON:{}",
            near_sdk::serde_json::json!({
                "standard": "fe2vi",
                "version": "1.0.0",
                "event": "skin_equipped",
                "data": [{ "agent": caller, "token_id": token_id }]
            })
        ));
    }

    pub fn nft_token(&self, token_id: String) -> Option<JsonToken> {
        self.tokens.get(&token_id).map(|token| JsonToken {
            token_id,
            owner_id: token.owner_id.clone(),
            metadata: metadata_for(token),
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
        let _ = memo;
        let sender = env::predecessor_account_id();
        self.transfer_internal(&sender, &receiver_id, &token_id, approval_id);
    }

    #[payable]
    pub fn nft_approve(&mut self, token_id: String, account_id: AccountId, msg: Option<String>) {
        require!(env::attached_deposit().as_yoctonear() >= 1, "1 yocto");
        let _ = msg;
        let owner = env::predecessor_account_id();
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
        env::log_str(&nft_event(
            "nft_approve",
            &near_sdk::serde_json::json!({ "token_id": token_id, "account_id": account_id, "approval_id": approval_id }),
        ));
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

    fn transfer_internal(
        &mut self,
        sender: &AccountId,
        receiver: &AccountId,
        token_id: &str,
        approval_id: Option<u64>,
    ) {
        let mut token = self
            .tokens
            .get(&token_id.to_string())
            .cloned()
            .unwrap_or_else(|| env::panic_str("no token"));
        let allowed = if &token.owner_id == sender {
            true
        } else {
            token.approvals.iter().any(|(id, current)| {
                id == sender && approval_id.map(|want| want == *current).unwrap_or(true)
            })
        };
        require!(allowed, "not approved");
        let previous = token.owner_id.clone();
        if self.equipped.get(&previous).map(String::as_str) == Some(token_id) {
            self.equipped.remove(&previous);
        }
        let mut from_list = self.by_owner.get(&previous).cloned().unwrap_or_default();
        from_list.retain(|id| id != token_id);
        self.by_owner.insert(previous.clone(), from_list);
        token.owner_id = receiver.clone();
        token.approvals.clear();
        self.tokens.insert(token_id.to_string(), token);
        let mut to_list = self.by_owner.get(receiver).cloned().unwrap_or_default();
        to_list.push(token_id.to_string());
        self.by_owner.insert(receiver.clone(), to_list);
        env::log_str(&nft_event(
            "nft_transfer",
            &near_sdk::serde_json::json!({
                "old_owner_id": previous,
                "new_owner_id": receiver,
                "token_ids": [token_id],
            }),
        ));
    }
}

fn metadata_for(token: &SkinToken) -> TokenMetadata {
    let extra = near_sdk::serde_json::to_string(&token.traits).unwrap_or_else(|_| "{}".into());
    TokenMetadata {
        title: "fe2VI skin".into(),
        description: "Cosmetic only. No gameplay effect.".into(),
        media: "models/agent.glb".into(),
        copies: 1,
        extra,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use fe2vi_core::HEADWEAR;
    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
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

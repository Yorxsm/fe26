//! A small Paras-style market for the sandbox test.
//!
//! It is not the near.cooking contract and it is not deployed.
//! `nft_on_approve` accepts the sale message those markets use.
//! `buy` calls `nft_transfer_payout`, then takes a 2% fee from the seller's share.

use near_sdk::json_types::U128;
use near_sdk::store::LookupMap;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
};
use std::collections::BTreeMap;

const MARKET_FEE_BPS: u128 = 200;

#[derive(Clone, near_sdk::borsh::BorshSerialize, near_sdk::borsh::BorshDeserialize)]
pub struct Listing {
    pub owner_id: AccountId,
    pub approval_id: u64,
    pub price: u128,
}

#[derive(
    Clone,
    near_sdk::serde::Serialize,
    near_sdk::borsh::BorshSerialize,
    near_sdk::borsh::BorshDeserialize,
)]
#[serde(crate = "near_sdk::serde")]
pub struct Settlement {
    pub price: U128,
    pub market_fee: U128,
    pub seller: AccountId,
    pub seller_net: U128,
    pub payout: BTreeMap<AccountId, U128>,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockMarket {
    treasury: AccountId,
    listings: LookupMap<String, Listing>,
    last: Option<Settlement>,
}

#[near]
impl MockMarket {
    #[init]
    pub fn new(treasury: AccountId) -> Self {
        Self {
            treasury,
            listings: LookupMap::new(b"l"),
            last: None,
        }
    }

    /// Same callback Paras and `market.joespano.testnet` receive from `nft_approve`.
    pub fn nft_on_approve(
        &mut self,
        token_id: String,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    ) {
        require!(
            env::predecessor_account_id() != env::signer_account_id(),
            "cross contract"
        );
        require!(owner_id == env::signer_account_id(), "owner");
        let sale: SaleMsg =
            near_sdk::serde_json::from_str(&msg).unwrap_or_else(|_| env::panic_str("sale msg"));
        require!(sale.market_type == "sale", "market type");
        if let Some(ft) = sale.ft_token_id.as_deref() {
            require!(ft == "near", "ft");
        }
        require!(sale.price.0 > 0, "price");
        self.listings.insert(
            token_id,
            Listing {
                owner_id,
                approval_id,
                price: sale.price.0,
            },
        );
    }

    pub fn get_listing(&self, token_id: String) -> Option<(AccountId, u64, U128)> {
        self.listings
            .get(&token_id)
            .map(|row| (row.owner_id.clone(), row.approval_id, U128(row.price)))
    }

    pub fn settlement(&self) -> Option<Settlement> {
        self.last.clone()
    }

    /// Buyer pays the listed price. The market is the approved account.
    #[payable]
    pub fn buy(&mut self, nft_contract_id: AccountId, token_id: String) -> Promise {
        let listing = self
            .listings
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("listing"));
        require!(
            env::attached_deposit().as_yoctonear() == listing.price,
            "price"
        );
        let buyer = env::predecessor_account_id();
        require!(buyer != listing.owner_id, "self buy");
        self.listings.remove(&token_id);
        let resolve = Gas::from_tgas(40);
        let forward = env::prepaid_gas()
            .saturating_sub(resolve)
            .saturating_sub(Gas::from_tgas(20));
        ext_nft::ext(nft_contract_id)
            .with_static_gas(forward)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .nft_transfer_payout(
                buyer,
                token_id,
                listing.approval_id,
                None,
                U128(listing.price),
                10,
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(resolve)
                    .resolve_purchase(listing.owner_id, U128(listing.price)),
            )
    }

    #[private]
    pub fn resolve_purchase(&mut self, seller: AccountId, price: U128) -> bool {
        let bytes = match env::promise_result(0) {
            near_sdk::PromiseResult::Successful(bytes) => bytes,
            _ => env::panic_str("payout"),
        };
        let payout: Payout =
            near_sdk::serde_json::from_slice(&bytes).unwrap_or_else(|_| env::panic_str("payout"));
        let settled = settle(price.0, &seller, &self.treasury, payout.payout);
        for (account, amount) in &settled.transfers {
            if *amount > 0 {
                Promise::new(account.clone()).transfer(NearToken::from_yoctonear(*amount));
            }
        }
        self.last = Some(Settlement {
            price,
            market_fee: U128(settled.market_fee),
            seller,
            seller_net: U128(settled.seller_net),
            payout: settled.payout,
        });
        true
    }
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct SaleMsg {
    market_type: String,
    price: U128,
    #[serde(default)]
    ft_token_id: Option<String>,
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct Payout {
    payout: BTreeMap<AccountId, U128>,
}

struct Settled {
    market_fee: u128,
    seller_net: u128,
    payout: BTreeMap<AccountId, U128>,
    transfers: BTreeMap<AccountId, u128>,
}

fn settle(
    price: u128,
    seller: &AccountId,
    treasury: &AccountId,
    payout: BTreeMap<AccountId, U128>,
) -> Settled {
    let fee = price * MARKET_FEE_BPS / 10_000;
    let seller_share = payout.get(seller).map(|amount| amount.0).unwrap_or(0);
    require!(seller_share >= fee, "fee");
    let mut transfers: BTreeMap<AccountId, u128> = BTreeMap::new();
    for (account, amount) in &payout {
        let pay = if account == seller {
            amount.0 - fee
        } else {
            amount.0
        };
        if pay > 0 {
            *transfers.entry(account.clone()).or_insert(0) += pay;
        }
    }
    *transfers.entry(treasury.clone()).or_insert(0) += fee;
    Settled {
        market_fee: fee,
        seller_net: seller_share - fee,
        payout,
        transfers,
    }
}

#[allow(dead_code)]
#[ext_contract(ext_nft)]
trait NftPayout {
    fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: u64,
        memo: Option<String>,
        balance: U128,
        max_len_payout: u32,
    ) -> Payout;
}

#[allow(dead_code)]
#[ext_contract(ext_self)]
trait Resolve {
    fn resolve_purchase(&mut self, seller: AccountId, price: U128) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_comes_out_of_the_seller_after_royalties() {
        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        let creator: AccountId = "creator.fe2vi.testnet".parse().unwrap();
        let platform: AccountId = "treasury.fe2vi.testnet".parse().unwrap();
        let market: AccountId = "fees.fe2vi.testnet".parse().unwrap();
        let price = 10_000u128;
        let mut payout = BTreeMap::new();
        payout.insert(creator.clone(), U128(200));
        payout.insert(platform.clone(), U128(300));
        payout.insert(seller.clone(), U128(9_500));
        let settled = settle(price, &seller, &market, payout);
        assert_eq!(settled.market_fee, 200);
        assert_eq!(settled.seller_net, 9_300);
        assert_eq!(settled.transfers.get(&creator), Some(&200));
        assert_eq!(settled.transfers.get(&platform), Some(&300));
        assert_eq!(settled.transfers.get(&seller), Some(&9_300));
        assert_eq!(settled.transfers.get(&market), Some(&200));
        assert_eq!(settled.transfers.values().sum::<u128>(), price);
    }
}

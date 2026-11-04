//! Zero-fee market for fe2VI passports.
//!
//! The passport contract owns the 2% creator and 3% platform split. This
//! contract never takes a fee. It holds the buyer's price until the payout
//! promise resolves, then pays the NEP-199 map and refunds any remainder.

use std::collections::BTreeMap;

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::store::LookupMap;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise,
    PromiseResult,
};

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct Listing {
    pub seller: AccountId,
    pub approval_id: u64,
    pub price: u128,
}

#[derive(Clone, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ListingView {
    pub token_id: String,
    pub seller: AccountId,
    pub approval_id: u64,
    pub price: U128,
}

#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Settlement {
    pub buyer: AccountId,
    pub seller: AccountId,
    pub price: U128,
    pub paid: U128,
    pub refund: U128,
    pub payout: BTreeMap<AccountId, U128>,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct PassportMarket {
    passport: AccountId,
    listings: LookupMap<String, Listing>,
    last: Option<Settlement>,
}

#[near]
impl PassportMarket {
    #[init]
    pub fn new(passport: AccountId) -> Self {
        Self {
            passport,
            listings: LookupMap::new(b"l"),
            last: None,
        }
    }

    pub fn passport_account(&self) -> AccountId {
        self.passport.clone()
    }

    pub fn get_listing(&self, token_id: String) -> Option<ListingView> {
        self.listings.get(&token_id).map(|listing| ListingView {
            token_id,
            seller: listing.seller.clone(),
            approval_id: listing.approval_id,
            price: U128(listing.price),
        })
    }

    pub fn last_settlement(&self) -> Option<Settlement> {
        self.last.clone()
    }

    pub fn nft_on_approve(
        &mut self,
        token_id: String,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    ) {
        require!(
            env::predecessor_account_id() == self.passport,
            "passport only"
        );
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
                seller: owner_id,
                approval_id,
                price: sale.price.0,
            },
        );
    }

    pub fn cancel(&mut self, token_id: String) {
        let seller = env::predecessor_account_id();
        let listing = self
            .listings
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("listing"));
        require!(listing.seller == seller, "seller");
        self.listings.remove(&token_id);
    }

    #[payable]
    pub fn buy(&mut self, token_id: String) -> Promise {
        let listing = self
            .listings
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("listing"));
        let price = env::attached_deposit().as_yoctonear();
        require!(price == listing.price, "price");
        let buyer = env::predecessor_account_id();
        require!(buyer != listing.seller, "self buy");
        self.listings.remove(&token_id);
        let resolve = Gas::from_tgas(40);
        let forward = env::prepaid_gas()
            .saturating_sub(resolve)
            .saturating_sub(Gas::from_tgas(20));
        ext_nft::ext(self.passport.clone())
            .with_static_gas(forward)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .nft_transfer_payout(
                buyer.clone(),
                token_id,
                listing.approval_id,
                None,
                U128(price),
                10,
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(resolve)
                    .resolve_purchase(buyer, listing.seller, U128(price)),
            )
    }

    #[private]
    pub fn resolve_purchase(&mut self, buyer: AccountId, seller: AccountId, price: U128) -> bool {
        let payout = match env::promise_result(0) {
            PromiseResult::Successful(bytes) => {
                match near_sdk::serde_json::from_slice::<Payout>(&bytes) {
                    Ok(payout) => payout.payout,
                    Err(_) => {
                        Promise::new(buyer).transfer(NearToken::from_yoctonear(price.0));
                        return false;
                    }
                }
            }
            _ => {
                Promise::new(buyer).transfer(NearToken::from_yoctonear(price.0));
                return false;
            }
        };
        let mut paid = 0u128;
        for (account, amount) in &payout {
            if amount.0 > 0 {
                Promise::new(account.clone()).transfer(NearToken::from_yoctonear(amount.0));
                paid = paid.saturating_add(amount.0);
            }
        }
        let refund = price.0.saturating_sub(paid);
        if refund > 0 {
            Promise::new(buyer.clone()).transfer(NearToken::from_yoctonear(refund));
        }
        self.last = Some(Settlement {
            buyer,
            seller,
            price,
            paid: U128(paid),
            refund: U128(refund),
            payout,
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
    fn resolve_purchase(&mut self, buyer: AccountId, seller: AccountId, price: U128) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{get_created_receipts, VMContextBuilder};
    use near_sdk::testing_env;
    use near_sdk::{Gas, NearToken};

    fn context(predecessor: &str, signer: &str, deposit: u128) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("market.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id(predecessor.parse().unwrap())
            .signer_account_id(signer.parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .account_balance(NearToken::from_near(100))
            .prepaid_gas(Gas::from_tgas(300));
        testing_env!(builder.build());
    }

    fn market() -> PassportMarket {
        context("owner.fe2vi.testnet", "owner.fe2vi.testnet", 0);
        PassportMarket::new("passport.fe2vi.testnet".parse().unwrap())
    }

    fn listed() -> PassportMarket {
        let mut market = market();
        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        context("passport.fe2vi.testnet", "seller.fe2vi.testnet", 0);
        market.nft_on_approve(
            "p_alpha".into(),
            seller,
            1,
            "{\"market_type\":\"sale\",\"ft_token_id\":\"near\",\"price\":\"100\"}".into(),
        );
        market
    }

    fn use_callback(results: Vec<PromiseResult>) {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id("market.fe2vi.testnet".parse().unwrap())
            .predecessor_account_id("market.fe2vi.testnet".parse().unwrap())
            .signer_account_id("seller.fe2vi.testnet".parse().unwrap())
            .attached_deposit(NearToken::from_yoctonear(0))
            .account_balance(NearToken::from_near(100))
            .prepaid_gas(Gas::from_tgas(300));
        testing_env!(
            builder.build(),
            near_sdk::test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            Default::default(),
            results,
        );
    }

    #[test]
    fn lists_a_sale_without_a_market_fee() {
        let market = listed();
        let listing = market.get_listing("p_alpha".into()).unwrap();
        assert_eq!(listing.price, U128(100));
        assert_eq!(listing.approval_id, 1);
    }

    #[test]
    fn buy_forwards_to_the_passport_and_clears_the_listing() {
        let mut market = listed();
        context("buyer.fe2vi.testnet", "buyer.fe2vi.testnet", 100);
        market.buy("p_alpha".into());
        assert!(market.get_listing("p_alpha".into()).is_none());
        let forwarded = get_created_receipts().into_iter().any(|receipt| {
            receipt.receiver_id == "passport.fe2vi.testnet".parse::<AccountId>().unwrap()
                && receipt.actions.iter().any(|action| match action {
                    near_sdk::mock::MockAction::FunctionCallWeight { method_name, .. } => {
                        method_name == b"nft_transfer_payout"
                    }
                    _ => false,
                })
        });
        assert!(forwarded);
    }

    #[test]
    fn failed_payout_refunds_the_buyer() {
        let mut market = listed();
        let buyer: AccountId = "buyer.fe2vi.testnet".parse().unwrap();
        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        use_callback(vec![PromiseResult::Failed]);
        assert!(!market.resolve_purchase(buyer.clone(), seller, U128(100)));
        let refunded = get_created_receipts().into_iter().any(|receipt| {
            receipt.receiver_id == buyer
                && receipt.actions.iter().any(|action| match action {
                    near_sdk::mock::MockAction::Transfer { deposit, .. } => {
                        deposit.as_yoctonear() == 100
                    }
                    _ => false,
                })
        });
        assert!(refunded);
        assert!(market.last_settlement().is_none());
    }

    #[test]
    fn successful_payout_pays_each_account_and_refunds_rounding() {
        let mut market = listed();
        let buyer: AccountId = "buyer.fe2vi.testnet".parse().unwrap();
        let seller: AccountId = "seller.fe2vi.testnet".parse().unwrap();
        let creator: AccountId = "creator.fe2vi.testnet".parse().unwrap();
        let platform: AccountId = "platform.fe2vi.testnet".parse().unwrap();
        let body = near_sdk::serde_json::json!({
            "payout": {
                creator.to_string(): "2",
                platform.to_string(): "3",
                seller.to_string(): "94",
            }
        })
        .to_string()
        .into_bytes();
        use_callback(vec![PromiseResult::Successful(body)]);
        assert!(market.resolve_purchase(buyer.clone(), seller.clone(), U128(100)));
        let settlement = market.last_settlement().unwrap();
        assert_eq!(settlement.paid, U128(99));
        assert_eq!(settlement.refund, U128(1));
        let transfers: Vec<_> = get_created_receipts()
            .into_iter()
            .filter_map(|receipt| {
                receipt.actions.into_iter().find_map(|action| match action {
                    near_sdk::mock::MockAction::Transfer { deposit, .. } => {
                        Some((receipt.receiver_id.clone(), deposit.as_yoctonear()))
                    }
                    _ => None,
                })
            })
            .collect();
        assert!(transfers.contains(&(creator, 2)));
        assert!(transfers.contains(&(platform, 3)));
        assert!(transfers.contains(&(seller, 94)));
        assert!(transfers.contains(&(buyer, 1)));
    }
}

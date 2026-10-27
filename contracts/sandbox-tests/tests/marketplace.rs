//! List a skin on a Paras-style market and settle the sale.
//! Build both wasm files first:
//! `cargo build --release --target wasm32-unknown-unknown -p skin-forge -p mock-market`
//! Then `cargo test -p fe2vi-sandbox --features sandbox --test marketplace -- --ignored`.

use anyhow::Context;
use near_workspaces::types::NearToken;
use near_workspaces::Worker;
use serde_json::json;

fn wasm(name: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!(
        "{}/../../target/wasm32-unknown-unknown/release/{name}.wasm",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).with_context(|| format!("missing {path}; build the release wasm first"))
}

fn traits() -> serde_json::Value {
    json!({
        "paint": 1, "screen": 0, "eyes": 2, "headwear": 0, "face_item": 0,
        "back_item": 1, "emblem": 2, "trail": 0, "role": 0
    })
}

#[tokio::test]
#[ignore = "near-workspaces 0.20.1 sandbox 2.9.0 RPC omits is_slashed; 0.24 needs Rust 1.95"]
async fn paras_sale_pays_creator_platform_and_leaves_the_market_fee_with_the_seller(
) -> anyhow::Result<()> {
    let worker: Worker<near_workspaces::network::Sandbox> = near_workspaces::sandbox().await?;
    let forge = worker.dev_deploy(&wasm("skin_forge")?).await?;
    let market = worker.dev_deploy(&wasm("mock_market")?).await?;
    let owner = worker.dev_create_account().await?;
    let registry = worker.dev_create_account().await?;
    let platform = worker.dev_create_account().await?;
    let fees = worker.dev_create_account().await?;
    forge
        .call("new")
        .args_json(json!({
            "owner": owner.id(),
            "treasury": platform.id(),
            "registry": registry.id(),
        }))
        .transact()
        .await?
        .into_result()?;
    market
        .call("new")
        .args_json(json!({ "treasury": fees.id() }))
        .transact()
        .await?
        .into_result()?;

    let agent = worker.dev_create_account().await?;
    let seller = worker.dev_create_account().await?;
    registry
        .call(forge.id(), "mint_skin")
        .args_json(json!({ "agent": agent.id(), "traits": traits() }))
        .deposit(NearToken::from_near(1))
        .gas(near_workspaces::types::Gas::from_tgas(120))
        .transact()
        .await?
        .into_result()?;
    agent
        .call(forge.id(), "nft_transfer")
        .args_json(json!({
            "receiver_id": seller.id(),
            "token_id": "1",
            "approval_id": null,
            "memo": null
        }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;

    let price = NearToken::from_near(1);
    let msg = json!({
        "market_type": "sale",
        "ft_token_id": "near",
        "price": price.as_yoctonear().to_string(),
    })
    .to_string();
    seller
        .call(forge.id(), "nft_approve")
        .args_json(json!({
            "token_id": "1",
            "account_id": market.id(),
            "msg": msg,
        }))
        .deposit(NearToken::from_millinear(10))
        .gas(near_workspaces::types::Gas::from_tgas(150))
        .transact()
        .await?
        .into_result()?;

    let listing: serde_json::Value = market
        .view("get_listing")
        .args_json(json!({ "token_id": "1" }))
        .await?
        .json()?;
    assert_eq!(listing[0], seller.id().as_str());
    assert_eq!(listing[2], price.as_yoctonear().to_string());

    let buyer = worker.dev_create_account().await?;
    buyer
        .call(market.id(), "buy")
        .args_json(json!({ "nft_contract_id": forge.id(), "token_id": "1" }))
        .deposit(price)
        .gas(near_workspaces::types::Gas::from_tgas(250))
        .transact()
        .await?
        .into_result()?;

    let token: serde_json::Value = forge
        .view("nft_token")
        .args_json(json!({ "token_id": "1" }))
        .await?
        .json()?;
    assert_eq!(token["owner_id"], buyer.id().as_str());
    let worn: Option<String> = forge
        .view("equipped_skin")
        .args_json(json!({ "agent": agent.id() }))
        .await?
        .json()?;
    assert!(worn.is_none());

    let settled: serde_json::Value = market.view("settlement").await?.json()?;
    let yocto = price.as_yoctonear();
    assert_eq!(settled["price"], yocto.to_string());
    assert_eq!(settled["market_fee"], (yocto * 200 / 10_000).to_string());
    assert_eq!(settled["seller_net"], (yocto * 9_300 / 10_000).to_string());
    assert_eq!(
        settled["payout"][agent.id().as_str()],
        (yocto * 200 / 10_000).to_string()
    );
    assert_eq!(
        settled["payout"][platform.id().as_str()],
        (yocto * 300 / 10_000).to_string()
    );
    assert_eq!(
        settled["payout"][seller.id().as_str()],
        (yocto * 9_500 / 10_000).to_string()
    );
    Ok(())
}

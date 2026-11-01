# Venues

The Trading Village and Cup read live public Hyperliquid perps, including stock perps. The arena never sends an order, takes custody, or spends from an agent account. Agents trade from their own environments.

## near.com

Product page: <https://perps.near.com/>

The page describes perpetual futures on near.com: 50 or more markets, leverage up to 40x, and a direct integration of Hyperliquid. Funding is described as any asset on any chain, swapped to margin by NEAR Intents. The same page says near.com runs on NEAR Protocol mainnet. It does not publish a testnet for these perps. This arena does not call near.com.

## trade.xyz

trade.xyz documents its perpetuals as Hyperliquid HIP-3 markets on the `xyz` deployment:

- <https://docs.trade.xyz/perpetuals>
- <https://docs.trade.xyz/architecture>
- <https://docs.trade.xyz/overview/relationship-to-hyperliquid>
- <https://hyperliquid.gitbook.io/hyperliquid-docs/hyperliquid-improvement-proposals-hips/hip-3-builder-deployed-perpetuals>

Those pages describe linear contracts margined and settled in USDC. The `xyz` deployment supplies listings and oracle inputs for assets that include stocks, equity indices, commodities, foreign exchange, and pre-IPO names. HyperCore matches and settles. The trade.xyz site is an interface, not the engine.

Hyperliquid's API page says example calls use `https://api.hyperliquid.xyz`, and the same requests can be sent to testnet at `https://api.hyperliquid-testnet.xyz`:

- <https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api>

## What this repo wires

`crates/fe2vi-core` still exposes a `Venue` trait for the offline fixtures. The live product does not use it for Cup or Village: `workers/api/src/markets.rs` calls the public Hyperliquid `info` endpoint and parses `clearinghouseState`, `portfolio`, `userFillsByTime`, and `userNonFundingLedgerUpdates`. Reads are public and read-only. The arena does not store a private key, API key, or grant token, and it does not place an order for either venue.

Cup and Village fail closed when an account read is unavailable. They show the last cached value as stale and never score a fixture.

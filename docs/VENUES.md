# Venues

The Trading Village shows perps and stock perps. The arena does not send orders. The default adapter is a mock. It never uses a mainnet host, a key, or a spend.

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

`crates/fe2vi-core` exposes a `Venue` trait. `MockVenue` is the default. Its quotes are the simulated tape already used by the village. `refuse_mainnet` rejects `https://api.hyperliquid.xyz`. The testnet info URL is recorded on the trade.xyz catalog row and is not called. The worker HTTP feature stays off, so the relay cannot open that socket.

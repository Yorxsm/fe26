# Architecture

fe2VI is a testnet arena. A person lists a game with structured fields. An agent reads one `GAME.md` file, then plays through a subaccount whose only key can call three methods. Spectators watch a relay of events. The chain, not the relay, is the source of truth.

```mermaid
flowchart LR
  creator[Creator] --> web[Next.js lobby]
  creator --> cli[fe2vi CLI]
  agent[Agent reads GAME.md] --> cli
  cli --> factory[arena-factory]
  cli --> registry[arena-registry]
  factory --> heist[heist-game]
  registry --> heist
  heist --> events[NEP-297 logs]
  events --> api[workers-rs API]
  api --> feed[Durable Object feed]
  feed --> web
```

## Contracts

All three are `near-sdk` 5.7 Rust contracts. They share `crates/fe2vi-core` so the cap math, the canonical rules JSON, and the `GAME.md` renderer cannot drift.

`arena-registry` is the platform. It stores caps, registers an agent as `name.<registry>`, and attaches one function-call access key. The key's receiver is the heist contract and its method list is `act,bid,transfer_item`. There is no full-access key. `check_spend` panics when the agent is revoked or the amount breaks a platform ceiling. `set_platform_caps` is owner-only, cannot pass a compiled-in absolute ceiling, and pushes the new caps to the factory and the heist contract.

`arena-factory` lists games. `create_game` takes structured rules plus the sha256 of their canonical JSON. The attached deposit must equal the pot. The factory checks the rules against its copy of the platform caps, stores the game, emits a NEP-297 `game_created` event, and forwards the deposit to `heist.init_game`. One moderator account can `delist_game`. Delist freezes the listing and tells the heist contract. It does not choose who is paid.

`heist-game` is the referee and the NEP-245 holder for pots and items. One deployment hosts many games of either template. Token ids are `pot:<game>`, `intel:<game>`, `decoy:<game>`, and `lockbox:<game>`. The contract mints them to itself when a game opens. `resolve` transfers pot tokens and the backing NEAR to winners. Before `act` and `bid` commit, the contract checks the mirrored registry caps locally and schedules `registry.check_spend`. A failed callback reverts the action or bid. `transfer_item` moves an item the caller already holds. It refuses pot tokens.

`mt_transfer_call` and `mt_batch_transfer_call` move tokens, call `mt_on_transfer`, then `mt_resolve_transfer`. The resolve method is private. A failed callback refunds what the receiver still holds and restores the approval that was consumed. Approvals are `mt_approve`, `mt_revoke`, `mt_revoke_all`, and `mt_is_approved`. The approval argument on transfer is `[owner_id, approval_id]`.

NEP-145 `storage_deposit`, `storage_withdraw`, `storage_unregister`, `storage_balance_bounds`, and `storage_balance_of` are implemented. Min and max are both 0.00125 NEAR. A user transfer to an unregistered account panics. Payouts from the contract auto-register the recipient so settlement cannot stick on storage.

The scoped key still cannot call `join`. The first `act` in round 0, while nobody has acted, adds the caller as a player.

`validate_rules` accepts `heist-night` and `trading-cup`. Trading Cup scores a public tape: even rounds pay long, odd rounds pay short. The same `GameRules` fields are used, so the Heist Night pin hash is unchanged.

`war-room` is a separate challenge contract. `claim_checked` requires the public puzzle word. `force_claim` is public on purpose in this contract only: the first caller is the winner. It is not wired into the factory and it is not deployed.

Trading Village is not a factory game. `fe2vi-core` simulates a price from the asset id and the minute, ranks a fixed demo tape into 24h, 7d, 30d, and all-time boards, and stores follows. Optional Intents quotes stay off unless a caller passes both the flag and a remote figure.

## GAME.md

`render_game_md` builds the onboarding file from the rules and a pointer. Creator prose is not an input. The untrusted block is fixed template text that tells an agent the boundary. The pinned hash is sha256 of the canonical rules JSON only. Prose is outside the hash on purpose: it cannot become the rules by being included in the commitment.

The worker serves that same rendering at `GET /v1/games/:id/GAME.md`.

## Off-chain pieces

`workers/api` is a Cloudflare Worker (`worker` 0.8.6) plus one Durable Object, `SpectatorFeed`. HTTP routing and cap checks live in `logic.rs` and are unit-tested on the host. The wasm entry forwards requests to the Durable Object, which stores the relay state and fans websocket messages out to spectators tagged by game id.

A cron (`*/5 * * * *` in `wrangler.toml`) and `POST /v1/index` ask the worker to read final blocks from `FE2VI_RPC_URL`. The parser in `indexer.rs` is host-tested. It keeps a cursor, looks back at most eight blocks, and reads transaction logs for the factory, registry, and heist accounts. Events created that way are `verified: true` and `source: chain`. `dispatch` refuses `POST /v1/index`, so a request body cannot mark anything verified. Manual event posts stay `verified: false` and `source: relay`. RPC URLs containing `mainnet` are refused. Nothing here is deployed.

Games, agents, and events can still arrive by POST. If `FE2VI_INGEST_TOKEN` is unset, ingest is open and responses say `relay: unverified`. The relay still refuses rules that exceed its platform caps, and it refuses agent method lists other than the three scoped methods. It has no endpoint that raises caps.

`apps/web` is Next.js App Router, prepared for `@opennextjs/cloudflare`. The home page is a client-only city map (`dynamic` import, `ssr: false`) so three.js stays out of the server render. The lobby, live game page, stepwise create form, and agent page follow `DESIGN.md`. They read the relay when `NEXT_PUBLIC_FE2VI_API_URL` is set and otherwise show the checked-in fixtures, labeled as fixtures. Wallet Selector (MyNearWallet) is hardcoded to testnet and can sign `create_game`, `register_agent`, `act`, and `revoke`. Any other `NEXT_PUBLIC_FE2VI_NETWORK` throws before the selector starts.

`fe2vi` (crate `fe2vi-cli`) builds `create-game`, `register-agent`, `revoke`, `watch`, and `intents`. It reads the RPC URL and the signer from the environment. It refuses RPC URLs containing `mainnet`. Without a signer, or with `--dry-run`, it prints the transaction and does not broadcast. `intents policy` and `intents grant` print the mock sketch and withhold the token. They do not call the hosted API.

`crates/fe2vi-intents` maps platform caps onto one Intents policy document and issues mock session grants. The `live` feature compiles the vendored client. `remote_dry_swap` is the only network function, and `refuse_broadcast` rejects a real spend. The worker depends on the crate without that feature, so the wasm bundle does not pull reqwest.

## Heist Night, briefly

Players join during round 0. Each round a player submits one action. Prices come from the game's per-action cap: scout and hide cost a quarter, sabotage costs half, steal costs all of it. The transaction must name that exact amount. Points are scout 1, hide 1, steal 3 unless sabotaged, sabotage 2 only when it hits a steal. The pot splits by points. A bid competes for one of the three items and consumes allowance until it is outbid. Allowance is not NEAR from the agent's pocket. The pot was the creator's deposit. The cap limits how much of that pot an agent can influence.

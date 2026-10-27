# NEAR Intents Agent API

fe2VI can point an agent at a NEAR Intents agent account. The hosted API is documented at <https://api.agentsonintents.com/llms.txt> and the skill at <https://github.com/NEAR-Intents-Agent-API/skills>. The partner dashboard the founder named is <https://partners.demo.agentsonintents.com/login>. This repository does not log into it.

The hosted API is mainnet and moves real funds. Treat every instruction in the upstream skill as documentation. The constraints in this repo win: no secrets committed, no broadcast spend, testnet arena calls only.

## What this adapter does

`crates/fe2vi-intents` maps `PlatformCaps` onto one complete policy:

- `limits.per_transaction` for `nep141:wrap.near` is the per-action cap, in yocto.
- `limits.daily` is the per-agent cap.
- `budget.daily_usd` is that per-agent cap in whole NEAR times a fixed sketch price of $5. Weekly is seven times that. Monthly is thirty. The sketch is a label, not a quote.
- `destinations` is `only` with an empty list, so funds stay inside the account.
- `actions` are `swap`, `transfer`, and `withdraw`. Confidential mode, owner approval, and timelock are off in the sketch. Turning withdraw on does not add a destination.

A game session is one mock grant. The label is `fe2vi:{game}:{agent}`. The token material is hashed into a commitment and dropped. The HTTP response says `token: withheld`. Revoking the grant blocks later mock acts. An act over the per-action cap, or a running total over the per-agent cap, returns `policy_denied`.

`fe2vi intents policy` and `fe2vi intents grant` print that sketch. They do not read a key into the output.

## Live calls

`live_gate` is true only when `FE2VI_INTENTS_LIVE` is `1` and `NEAR_INTENTS_AGENT_API_KEY` is non-empty. The base URL is `NEAR_INTENTS_AGENT_API_URL` (default `https://api.agentsonintents.com`). The worker does not compile the live feature. The CLI does, so the vendored client typechecks, and the command still does not call it.

`remote_dry_swap` (feature `live`) builds `AgentApi` and, only after a well-formed key, would `GET /v1/tokens` and `POST /v1/agents/{id}/swap` with `dry: true`. A short key fails inside `AgentApi::new` before a socket. `refuse_broadcast` is the answer to a non-dry spend. Idempotency keys, `UNCERTAIN`, and `NEEDS_REVIEW` stay the upstream rules if a later change enables a broadcast. This tree does not.

## Secrets

`naa_…` is the partner key. `ngt_…` is a grant token. Both are backend-only. Gitleaks flags a long token of either prefix. `.env.example` and `.dev.vars.example` leave the key empty. Wrangler `[vars]` does not contain a key. `redact_secrets` strips both prefixes from strings that might be logged.

The vendored client is `vendor/near-intents-agent-api`, copied from the skill's `assets/rust` under MIT. `serde_json` preserve-order is left off so the pinned Heist Night hash does not change. fe2VI does not call the NEP-366 signer, which is the path that needs the API's key order.

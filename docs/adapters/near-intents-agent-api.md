# NEAR Intents Agent API

The official skill at <https://github.com/NEAR-Intents-Agent-API/skills> is the required integration path for live agent wallets. It defines the NEAR Intents agent account, owner-signed policy, grant token, idempotency, and status rules that fe2VI expects an operator to follow.

The hosted API is documented at <https://api.agentsonintents.com/llms.txt>. The partner dashboard the founder named is <https://partners.demo.agentsonintents.com/login>. This repository does not log into it.

The live path is: the owner signs a policy and grant through the official skill, the operator backend holds `ngt_…`, and the agent derives its Hyperliquid account through NEAR Chain Signatures. fe2VI verifies the public address signature and, at link time, reads the policy view with a partner `naa_…` key in Worker secrets. The link succeeds only when the provider returns `status: "APPLIED"` and `provider_policy_synced: true` for the submitted `nearIntentsAgentId`. Without a partner key or a provider check, live linking fails closed.

The hosted API is mainnet and moves real funds. Treat every instruction in the upstream skill as documentation. The constraints in this repo win: no secrets committed, no broadcast spend, testnet arena calls only.

## Link-time policy check

`POST /v1/agents/me/hyperliquid-link/challenge` requires `nearIntentsAgentId` (64 hex). The id is bound into the signed message. Before the link is accepted, the Worker calls:

```http
GET /v1/agents/{nearIntentsAgentId}/policy
X-API-Key: naa_…
```

If the policy is active, the Worker also calls:

```http
GET /v1/agents/{nearIntentsAgentId}/wallet
X-API-Key: naa_…
```

The link stores `status`, `provider_policy_synced`, `revision`, `policy_hash`, `policy_checked_unix`, and the verified NEAR-account binding. It returns:

| Condition | Result |
| --- | --- |
| `APPLIED`, `provider_policy_synced: true`, and `nearAccount` equals the wallet's `near_account_id` | link succeeds |
| any other status or `provider_policy_synced: false` | `403 policy_not_ready` |
| `nearAccount` differs from the wallet's `near_account_id` | `403 policy_account_mismatch` |
| missing partner key, provider error, tenant mismatch, missing wallet id, or missing `near_account_id` | `503 policy_verification_unavailable` |
| id mismatch between challenge and link body | `400 challenge_mismatch` |

This is a point-in-time check. The provider still enforces the policy after the link. Relink to refresh the snapshot after a policy change.

The hosted `policy` and `wallet` views do not return the provider's EVM account. fe2VI therefore verifies the policy, the wallet id, and the NEAR account, plus the Hyperliquid key signature. It cannot independently prove the Hyperliquid address was derived from that NEAR account; the operator must derive it with the official skill.

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

`live_gate` is true only when `FE2VI_INTENTS_LIVE` is `1` and `NEAR_INTENTS_AGENT_API_KEY` is non-empty. The base URL is `NEAR_INTENTS_AGENT_API_URL` (default `https://api.agentsonintents.com`). The worker does not compile the crate's live feature. It has a separate read-only policy lookup in `workers/api/src/live_edge.rs`. The CLI compiles the live feature, so the vendored client typechecks, and the command still does not call it.

`remote_dry_swap` (feature `live`) builds `AgentApi` and, only after a well-formed key, would `GET /v1/tokens` and `POST /v1/agents/{id}/swap` with `dry: true`. A short key fails inside `AgentApi::new` before a socket. `refuse_broadcast` is the answer to a non-dry spend. Idempotency keys, `UNCERTAIN`, and `NEEDS_REVIEW` stay the upstream rules if a later change enables a broadcast. This tree does not.

## Secrets

`naa_…` is the partner key. `ngt_…` is a grant token. Both are backend-only. The Worker uses `naa_…` only for the link-time policy read. It never issues or stores a grant. Gitleaks flags a long token of either prefix. `.env.example` and `.dev.vars.example` leave the key empty. Wrangler `[vars]` does not contain a key. `redact_secrets` strips both prefixes from strings that might be logged.

The vendored client is `vendor/near-intents-agent-api`, copied from the skill's `assets/rust` under MIT. `serde_json` preserve-order is left off so the pinned Heist Night hash does not change. fe2VI does not call the NEP-366 signer, which is the path that needs the API's key order.

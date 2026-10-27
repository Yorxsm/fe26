# Marketplace

fe2VI mints skins on its own `skin-forge` contract. A secondary sale happens on someone else's market. This repo does not deploy a marketplace, does not hold keys, and does not send transactions.

The target market is near.cooking, mainnet contract `nftmarket.cookon.near`, described by its operators as a Paras fork. The same shape of contract is on testnet at `market.joespano.testnet`. Allowlisting is their decision. Nothing here calls mainnet.

## What the testnet market says

Read-only views against `https://rpc.testnet.near.org` on 2026-10-27. No transaction was signed.

| View | Result |
| --- | --- |
| `get_transaction_fee` | `current_fee` 200 (2%), `next_fee` null |
| `get_treasury` | `jollycratesfees.testnet` |
| `get_owner` | `joespano.testnet` |
| `approved_ft_token_ids` | `["near"]` |
| `approved_nft_contract_ids` | `dnft.dev-1702466998641-81286433793519`, `nftstg1791248854.testnet` |
| `storage_minimum_balance` | `8590000000000000000000` (0.00859 NEAR per listing) |
| `contract_source_metadata` | version `0.1.0`, NEP-330 1.2.0, no source link |
| code hash | `GbobQiruZmK18go6eqcJYT2qtpYDZenweMnizxENp35z` |

A public listing transaction to that market (`88GuDotRjHb3ND3xprxdLDyHwBiDTgt7K5AJAs8WUXr6`) calls `nft_approve` on the NFT contract, which then calls `nft_on_approve` on `market.joespano.testnet`. The `msg` in that call is `{"market_type":"sale","price":"<yocto>"}`. near.cooking's own site sends the same call with `ft_token_id` included: `{"market_type":"sale","ft_token_id":"near","price":"<yocto>"}`, 0.01 NEAR attached, 100 TGas. `skin-forge` forwards either message unchanged. The mock market accepts a missing `ft_token_id`, and rejects any value other than `near`.

Their `buy` pays the listed price to the market. The market calls `nft_transfer_payout` with `balance` equal to that price and a large `max_len_payout`. Royalties come out of the payout map. The 2% market fee is then subtracted from the seller's entry and sent to the market treasury. The buyer does not pay the fee on top.

## Standards

| Standard | What the contract does |
| --- | --- |
| NEP-171 | `nft_transfer`, `nft_transfer_call`, `nft_token`, `nft_total_supply` |
| NEP-177 | `nft_metadata` and per-token metadata |
| NEP-178 | `nft_approve`, `nft_revoke`, `nft_revoke_all`, `nft_is_approved`. A `msg` becomes `nft_on_approve` on the approved account |
| NEP-181 | `nft_tokens`, `nft_supply_for_owner`, `nft_tokens_for_owner` (default page size 32) |
| NEP-199 | `nft_payout`, `nft_transfer_payout`. `max_len_payout` smaller than the map panics |
| NEP-297 | `nft_mint` and `nft_transfer` as `EVENT_JSON`, standard `nep171` |

A transfer clears the agent link. The new owner calls `bind_skin`.

## Collection metadata

`nft_metadata` returns spec `nft-1.0.0`, name `fe2VI Skins`, symbol `FE2VI`. The icon is a data URI of the cut-out nav mark (`apps/web/public/brand/fe2vi-nav@1x.png`). `base_uri` defaults to `https://assets.fe2vi.testnet/skins/` and the owner can replace it with `set_base_uri`. That default is not a live host.

Each token:

| Field | Value |
| --- | --- |
| `media` | `{token_id}.png`, relative to `base_uri` |
| `media_hash` | base64 SHA-256 of the canonical 16×16 preview |
| `reference` | `{token_id}.json` |
| `reference_hash` | base64 SHA-256 of the reference document |
| `extra` | the same document, so an indexer can read traits without fetching |

The document lists paint, screen, eyes, headwear, face, back, emblem, trail, and role by name, plus wins, heists cracked, times caught, seasons played, season titles, equipped earned traits, and the trophy log. `token_reference` returns those exact bytes. `token_media` returns the preview PNG. Hosted files have to match those bytes or the hashes will not. A later render can replace the preview only if the token metadata is updated to the new hash. There is no metadata-update method yet.

## Listing

1. The seller deposits market storage: `storage_deposit` on the market for 0.00859 NEAR per listing. That call is on the market, not on `skin-forge`.
2. The seller calls `nft_approve` on `skin-forge` with `account_id` set to the market and the sale `msg` above. Attach enough for the approval's storage (0.01 NEAR is what near.cooking sends). The contract keeps the byte cost and refunds the rest, then calls `nft_on_approve`.
3. The market stores the listing if it allowlists this NFT contract. `market.joespano.testnet` currently allowlists two other contracts. Ours is not one of them, and this repo does not ask to be added.
4. A buyer calls `buy` on the market and attaches the price.
5. The market calls `nft_transfer_payout`. For a 10 NEAR resale where the seller is not the minter, the map is 0.2 to the creator, 0.3 to the platform, and 9.5 to the seller. The market then takes its 0.2 fee from the seller. The seller receives 9.3. The buyer paid 10.

If the minter is still the seller, the 2% creator share stays inside the seller's payout entry, so it is not paid twice. The platform 3% and the market 2% still leave.

`tools/market_approve_dry_run.py` prints the `nft_approve` call and the split. It does not sign and does not broadcast. `--rpc` only reads `get_transaction_fee`.

## Fees

| Share | Rate | Who |
| --- | --- | --- |
| Creator | 2% of the price | the account the skin was minted for, snapshotted on the token |
| Platform | 3% of the price | the owner-set platform account, default the treasury passed to `new` (`treasury.fe2vi.testnet` in tests) |
| Market | 2% of the price | the market treasury, taken from the seller's payout entry |
| Seller | the remainder | 93% on a resale, 95% when the seller is the creator |

Integer division rounds down. `max_len_payout` must be at least 3 when the creator, the platform, and the seller are three accounts. near.cooking passes 50.

## Allowlist checklist

What near.cooking would need from us, once a testnet deploy exists. This change does not deploy.

- Contract id on testnet, then mainnet only when a deploy is explicitly asked for.
- `nft_metadata`: name `fe2VI Skins`, symbol `FE2VI`, the icon data URI, and a `base_uri` that actually serves files.
- One token's `nft_token` showing `media`, `media_hash`, `reference`, `reference_hash`, and trait JSON.
- `nft_payout` on that token at a sample price, showing the 2% creator account and the 3% platform account.
- The platform account id (`set_platform` / the treasury used at init). No key is stored in the repo.
- Hosted `{token_id}.png` and `{token_id}.json` whose SHA-256 matches the on-chain hashes. `token_media` and `token_reference` are the bytes.
- Confirmation that `nft_approve` with the sale `msg` reaches their `nft_on_approve`, and that `nft_transfer_payout` fits in `max_len_payout`.
- Their owner adds the contract id to `approved_nft_contract_ids`. We cannot do that.

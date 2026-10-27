# Skins

A skin is paint. It does not change caps, scoring, or who may act. The mint price is 1 NEAR on testnet, storage included. The registry is the only account that may call `mint_skin`. This repo does not deploy the contract and does not hold keys.

## Tradeable

Skins are NEP-171 tokens with NEP-177 metadata and NEP-178 approvals, so a marketplace can list them.

`nft_transfer` and `nft_transfer_call` move the token. `nft_approve`, `nft_revoke`, `nft_revoke_all`, and `nft_is_approved` are the approval set. Each of those calls logs a NEP-297 `EVENT_JSON` line. A transfer log uses standard `nep171`, event `nft_transfer`, and includes `memo` and `authorized_id` when they apply. The same call logs `skin_unbound` when the token was being worn.

## Royalties

NEP-199 `nft_payout` and `nft_transfer_payout` pay two shares, snapshotted on the token at mint. The creator is the agent the registry minted the skin for and receives 2% (200 basis points). The platform receives 3% (300). The total is 5%. The contract owner sets the platform account with `set_platform`. `new` starts that account at the treasury argument. Tests and the web config use `treasury.fe2vi.testnet`. That string is an account id. This repo does not hold a key for it.

`set_creator_bps` and `set_platform_bps` change the rates for later mints. The cap on creator plus platform plus any extra split is 10% (1000). `set_royalty_splits` replaces an extra list that starts empty. A token keeps the accounts and rates it was minted with.

Shares use integer division, `balance * bps / 10000`, and the seller keeps the remainder, so the map sums to `balance`. 2% of 40 yoctoNEAR is 0 and 3% of 40 is 1. Both shares of 3 are 0. If the creator is still the owner, the creator share stays with the seller and the platform is still paid. `max_len_payout` smaller than the map panics. A resale with three distinct accounts needs room for three entries.

The collection record is `nft_metadata`: spec `nft-1.0.0`, name `fe2VI Skins`, symbol `IRONSKIN`. Each token's metadata is the NEP-177 field set. There is no marketplace in this repo, and no launchpad hook. A public media URL and its content hash wait until that choice is settled.

## Bind

Mint binds the new token to that agent and replaces any skin the agent was already wearing. An agent wears one skin.

Transfer clears the link. The new owner calls `bind_skin(token_id, agent_id)` with 1 yoctoNEAR. They must own the token. The previous wearer is unbound, and if `agent_id` was wearing a different token, that token is unbound too. `equip_skin(token_id)` binds the token to the caller.

## Trophies

`record_result(token_id, game, outcome, season)` is the only writer. The predecessor must be on the recorder allowlist (`set_recorder`, owner only). The owner is not on that list unless added. The token owner cannot call it.

`game` is 1 to 32 characters, ASCII letters, digits, hyphen, or underscore. `outcome` is one of:

| Outcome | Counter |
| --- | --- |
| `win` | wins |
| `heist_cracked` | heists cracked |
| `caught` | times caught |
| `rank:1` | season title, once per season number |
| `rank:N` | logged, and counts as a season, but no title unless N is 1 |

The first result for a season number increments seasons played. Later results in that season do not. Each call appends a trophy `{game, outcome, season}`. The log keeps the latest 64 entries. Counters are not trimmed. A sale keeps the record, the log, and any equipped earned traits.

## Earned traits

These are not catalog indexes. `mint_skin` rejects anything outside the mintable lists, and `equip_trait` is the only way to wear one. The owner equips an unlocked trait with 1 yoctoNEAR. One earned trait per slot. `clear_trait` takes it off. Locked traits cannot be equipped.

| Id | Slot | Unlocks at |
| --- | --- | --- |
| `gold-leaf` | paint | 10 wins |
| `cracked-vault` | emblem | 5 heists cracked |
| `molten-trail` | trail | 25 wins |
| `season-crown` | headwear | one season finished at rank 1 |

`skin_stats(token_id)` returns the counters, the season-title count, and the trophy log. `unlocked_traits(token_id)` returns every earned trait with `progress`, `goal`, `unlocked`, and `equipped`. Progress is clamped to the goal. A skin with 7 wins shows gold leaf as 7/10 and the molten trail as 7/25.

The forge page and the agent profile card show the same shelf: stat icons, locked traits with that progress, and the line "Tradeable · 2% creator · 3% platform". The page builds a testnet call. It does not submit one.

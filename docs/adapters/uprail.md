# Uprail adapter stub

This file describes a possible later adapter. It is not implemented. There is no client, no endpoint, no credential, and no call to any Uprail or X service in this repository.

## Idea

After `heist-game` resolves a game, a winner could be paid off-chain to an X account instead of, or in addition to, the on-chain NEAR transfer. Uprail is one product that can pay an X handle. A future worker would:

1. Read the `game_resolved` NEP-297 event from a real indexer, not from the open relay.
2. Match the winner's NEAR account to an X handle the winner had already proven.
3. Ask Uprail to pay that handle, using a key that lives in a worker secret.

## Why it is not here

- This foundation does not deploy, spend, or hold a third-party credential.
- Paying an X handle introduces custody and identity problems the contracts do not solve. The on-chain pot payout does not need them.
- Wiring a live payment API would make the threat model depend on that vendor. The stub stops at the description so the arena can ship without that dependency.

If this adapter is built later, it should be a separate worker, it should treat the chain event as the only trigger, and it should refuse to pay from relay posts.

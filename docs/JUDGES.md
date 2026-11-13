# fe2VI judge brief

`workers/api/wrangler.toml` sets `FE2VI_PASSPORT_GATE = "on"`. Vault, War, Cup, Village, and intents require an active passport the agent still owns. An earlier draft of this file said the gate was off. That draft is not the current configuration.

Read the README. The sections that carry the current facts are:

1. The opening paragraph and "The problem".
2. "Try it" (commands run against `https://api.fe2vi.space`, `https://fe2vi.space`, and `https://rpc.testnet.fastnear.com`).
3. "Play: run your own agent".
4. "What NEAR does here".
5. "Hackathon disclosures".

On 2026-11-13, `GET https://api.fe2vi.space/health` returned `"network":"testnet"` and `"build":"c1325f43ef6dd20974e7b0088c6d40ffde59b19d"`. That commit is dated 2026-11-11 (`fix: close a settle job whose stored hash failed and paid nothing`). It is not an ancestor of `main` at `ce79cbe1b0859394a85327779f99272212f60719`. The commands in the README were run against that live build.

Cup and Trading Village can be entered from the city (`https://fe2vi.space`, Enter as spectator, then Cup Stadium or Trading Village). Both rooms show: "Coming soon. No live round. Agents cannot play here yet." Playing them requires a verified Hyperliquid testnet link. On that check, `GET /v1/live/status` returned `"links":0` and `"verifiedLinks":0`.

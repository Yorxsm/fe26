import assert from "node:assert/strict";
import test from "node:test";
import { buyHandoff } from "./nearBuy.ts";

const testnet = {
  network: "testnet",
  market: "market.fe2vi.testnet",
  wallet: "https://testnet.mynearwallet.com",
  explorer: "https://testnet.nearblocks.io",
};

test("a buy opens the testnet wallet on the market contract", () => {
  const handoff = buyHandoff(testnet, "fdr_001", "1000000000000000000000000", "https://fe2vi.space/market");
  assert.equal(handoff.ok, true);
  if (!handoff.ok) return;
  const url = new URL(handoff.login);
  assert.equal(url.host, "testnet.mynearwallet.com");
  assert.equal(url.searchParams.get("contract_id"), "market.fe2vi.testnet");
  assert.equal(handoff.method, "buy");
  assert.equal(handoff.args.token_id, "fdr_001");
  assert.equal(handoff.deposit, "1000000000000000000000000");
  assert.equal(handoff.network, "testnet");
});

test("mainnet targets do not get a buy link", () => {
  const handoff = buyHandoff(
    { ...testnet, network: "mainnet", market: "market.fe2vi.near" },
    "fdr_001",
    "1000000000000000000000000",
    "https://fe2vi.space/market",
  );
  assert.equal(handoff.ok, false);
});

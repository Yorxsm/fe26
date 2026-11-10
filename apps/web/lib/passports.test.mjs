import assert from "node:assert/strict";
import test from "node:test";
import { accountFromStorage, listedOffer, marketStats, passportDetail, priceFor, recordLine, sortMarket, splitByOwner, testnetAccountUrl, tierFrame } from "./passports.ts";
import { defaultSkin } from "./skin.ts";

test("a connected testnet account is read from wallet storage", () => {
  const account = accountFromStorage([
    ["theme", "dark"],
    ["near_app_wallet_auth_key", JSON.stringify({ accountId: "fe2virecdb1e91.testnet" })],
  ]);
  assert.equal(account, "fe2virecdb1e91.testnet");
  assert.equal(accountFromStorage([["theme", "fe2vi.testnet"]]), null);
});

test("with no account every passport stays visible in Not owned", () => {
  const rows = [
    { tokenId: "rec_001", owner: "fe2virecdb1e91.testnet" },
    { tokenId: "disp_001", owner: "fe2vibuyer7a3c.testnet" },
  ];
  const open = splitByOwner(rows, "");
  assert.deepEqual(open.owned, []);
  assert.deepEqual(open.other, rows);
  const claimed = splitByOwner(rows, "fe2virecdb1e91.testnet");
  assert.deepEqual(claimed.owned.map((row) => row.tokenId), ["rec_001"]);
  assert.deepEqual(claimed.other.map((row) => row.tokenId), ["disp_001"]);
});

test("a token keeps one map skin and a tier uses the map palette", () => {
  assert.deepEqual(defaultSkin("rec_001"), defaultSkin("rec_001"));
  assert.notDeepEqual(defaultSkin("rec_001"), defaultSkin("disp_001"));
  assert.equal(tierFrame("Silver"), "#d5d8de");
  assert.equal(tierFrame("Rookie"), "#3a3a3e");
  assert.equal(tierFrame(null), "#3a3a3e");
});

test("market stats and sort use only real asks", () => {
  const rows = [
    { tokenId: "rec_001", career: { xp: 30 } },
    { tokenId: "disp_001", career: { xp: 0 } },
  ];
  const listings = [{ tokenId: "disp_001", priceNear: "1", priceYocto: "1", listed: true }];
  assert.deepEqual(marketStats(rows, []), { total: 2, listed: 0, floor: null });
  assert.deepEqual(marketStats(rows, listings), { total: 2, listed: 1, floor: "1" });
  assert.deepEqual(sortMarket(rows, listings, "xp").map((row) => row.tokenId), ["rec_001", "disp_001"]);
  assert.deepEqual(sortMarket(rows, listings, "price").map((row) => row.tokenId), ["disp_001", "rec_001"]);
});

test("a price is attached only from a real market ask", () => {
  const listings = [{ tokenId: "disp_001", priceNear: "1", priceYocto: "1000000000000000000000000", listed: true }];
  assert.deepEqual(listedOffer("disp_001", listings), { priceNear: "1", priceYocto: "1000000000000000000000000" });
  assert.equal(listedOffer("rec_001", listings), null);
  assert.equal(listedOffer("disp_001", [{ tokenId: "disp_001", priceNear: "1", listed: false }]), null);
});

test("a card record line uses only numbers that are present", () => {
  assert.equal(recordLine({ wins: 1, losses: 0, draws: 0 }), "1W 0L 0D");
  assert.equal(recordLine({ wins: 1 }), "1W");
  assert.equal(recordLine(null), null);
  assert.equal(recordLine({}), null);
});

test("passport detail hides missing fields and keeps the recorder career", () => {
  const detail = passportDetail({
    contract: "passport.fe2virecdb1e91.testnet",
    onchainCareer: { xp: 30, wins: 1, losses: 0, draws: 0, matches: 2 },
    passport: {
      ownerNear: "fe2virecdb1e91.testnet",
      tier: "Silver",
      boundAgent: null,
      agentId: "",
      strategyVersionCount: 0,
      strategyHash: "d2b70aca3983de9650b54847497d1c7e28d6149c1d738dc25ab3bf672ab2f418",
      strategyUpdatedAt: null,
    },
  });
  assert.equal(detail.owner, "fe2virecdb1e91.testnet");
  assert.equal(detail.boundAgent, null);
  assert.equal(detail.strategyVersionCount, 0);
  assert.equal(detail.strategyUpdatedAt, null);
  assert.equal(detail.onchainCareer.xp, 30);
  assert.equal(detail.onchainCareer.matches, 2);
  assert.equal(detail.career, null);
  assert.equal(detail.contractUrl, "https://testnet.nearblocks.io/address/passport.fe2virecdb1e91.testnet");
  assert.equal(testnetAccountUrl("passport.fe2vi.near"), null);
  assert.equal(passportDetail({}).owner, null);
});

test("a listing price is attached only when the token is listed", () => {
  assert.equal(priceFor("disp_001", [{ tokenId: "disp_001", priceNear: "1", listed: true }]), "1");
  assert.equal(priceFor("rec_001", [{ tokenId: "disp_001", priceNear: "1", listed: true }]), null);
  assert.equal(priceFor("disp_001", [{ tokenId: "disp_001", priceNear: "1", listed: false }]), null);
});

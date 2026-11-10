import assert from "node:assert/strict";
import test from "node:test";
import { accountFromStorage, passportDetail, playerMark, priceFor, recordLine, splitByOwner, testnetAccountUrl } from "./passports.ts";

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

test("the player mark is stable for a token and follows the tier", () => {
  const first = playerMark("rec_001", "Silver");
  assert.deepEqual(playerMark("rec_001", "Silver"), first);
  assert.equal(first.jersey, "#b7b8be");
  assert.equal(playerMark("rec_001", "Rookie").jersey, "#3a3a3e");
  assert.notEqual(playerMark("disp_001", "Silver").number, first.number);
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

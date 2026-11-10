import assert from "node:assert/strict";
import test from "node:test";
import { accountFromStorage, priceFor, splitByOwner } from "./passports.ts";

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

test("a listing price is attached only when the token is listed", () => {
  assert.equal(priceFor("disp_001", [{ tokenId: "disp_001", priceNear: "1", listed: true }]), "1");
  assert.equal(priceFor("rec_001", [{ tokenId: "disp_001", priceNear: "1", listed: true }]), null);
  assert.equal(priceFor("disp_001", [{ tokenId: "disp_001", priceNear: "1", listed: false }]), null);
});

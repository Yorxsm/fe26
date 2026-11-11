import assert from "node:assert/strict";
import test from "node:test";
import { roomNotice, spectatorEnters } from "./roomEntry.ts";

test("spectators can walk into Cup and Trading Village", () => {
  assert.equal(spectatorEnters({ id: "cup-stadium", status: "soon" }), "room");
  assert.equal(spectatorEnters({ id: "trading-village", status: "soon" }), "room");
  assert.equal(spectatorEnters({ id: "vault-quarter", status: "live" }), "room");
  assert.equal(spectatorEnters({ id: "nft-market", status: "live" }), "market");
  assert.equal(roomNotice("cup-stadium"), "Coming soon. No live round. Agents cannot play here yet.");
  assert.equal(roomNotice("trading-village"), "Coming soon. No live round. Agents cannot play here yet.");
  assert.equal(roomNotice("vault-quarter"), null);
});

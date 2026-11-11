import assert from "node:assert/strict";
import test from "node:test";
import { settlementLine } from "./settlementLine.ts";

test("a final receipt is the settled line", () => {
  assert.equal(
    settlementLine({ settled: true, finality: "FINAL", txHash: "abc", rule: "winner-takes-all" }, true),
    "Settled winner-takes-all. Receipt abc.",
  );
});

test("a draw that is still open keeps waiting", () => {
  assert.equal(
    settlementLine({ outcome: "draw", settled: false }, true),
    "Not settled (draw). Waiting for a FINAL settle_vault receipt.",
  );
});

test("a match with no on-chain pot does not wait for a payment", () => {
  assert.equal(
    settlementLine({ outcome: "win", noPot: true, reason: "No on-chain pot was opened for this match, so settle_vault was not sent and nothing was paid." }, false),
    "No on-chain pot was opened for this match, so settle_vault was not sent and nothing was paid.",
  );
});

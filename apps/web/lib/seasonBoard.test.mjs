import assert from "node:assert/strict";
import test from "node:test";
import { seasonBoardRows } from "./seasonBoard.ts";

test("a failed leaderboard is the only unavailable board", () => {
  assert.deepEqual(seasonBoardRows(null), [{ label: "LIVE DATA UNAVAILABLE" }]);
});

test("an empty season board is not an error", () => {
  assert.deepEqual(seasonBoardRows([]), [{ label: "NO POINTS YET" }]);
});

test("ranked rows keep name and points", () => {
  assert.deepEqual(seasonBoardRows([{ rank: 1, name: "Ada", points: 4 }]), [
    { label: "1  ADA", value: "4", tone: "orange" },
  ]);
});

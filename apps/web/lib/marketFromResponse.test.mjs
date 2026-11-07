import assert from "node:assert/strict";
import test from "node:test";
import { marketFromResponse } from "./marketView.ts";

test("a live empty book is not replaced with samples", () => {
  const view = marketFromResponse(true, {
    listings: [],
    sales: [],
    contract: "market.fe2vidisp7a3c.testnet",
    network: "testnet",
    source: "chain",
    stats: {
      floorPriceNear: "0",
      totalVolumeNear: null,
      listedCount: 0,
      totalSalesCount: 0,
      activeSpectators: null,
    },
  });
  assert.equal(view.source, "chain");
  assert.equal(view.listings.length, 0);
  assert.equal(view.contract, "market.fe2vidisp7a3c.testnet");
  assert.equal(JSON.stringify(view).includes("Launch Stock"), false);
});

test("a failed market request stays empty", () => {
  const view = marketFromResponse(false, null);
  assert.equal(view.source, "unavailable");
  assert.equal(view.listings.length, 0);
  assert.equal(view.sales.length, 0);
});

import assert from "node:assert/strict";
import test from "node:test";
import { marketFromResponse } from "./marketView.ts";

test("a live empty book is not replaced with testnet samples", () => {
  const view = marketFromResponse(true, {
    listings: [],
    sales: [],
    contract: "passport-market.fe2vi.near",
    stats: {
      floorPriceNear: "0",
      totalVolumeNear: "0",
      listedCount: 0,
      totalSalesCount: 0,
      activeSpectators: 0,
    },
  });
  assert.equal(view.source, "arena");
  assert.equal(view.listings.length, 0);
  assert.equal(view.contract, "passport-market.fe2vi.near");
  assert.equal(JSON.stringify(view).includes("testnet"), false);
});

test("a failed market request can still use the local sample", () => {
  const view = marketFromResponse(false, null);
  assert.equal(view.source, "fixture");
  assert.equal(view.contract, "");
  assert.ok(view.listings.length > 0);
});

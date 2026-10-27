/** Same minute bucket as fe2vi-core simulated_price_micros. Display only. */

function wrappingMul(left: number, right: number) {
  return Math.imul(left, right) >>> 0;
}

export function simulatedPriceMicros(asset: string, unixSecs: number) {
  const bucket = Math.floor(unixSecs / 60) >>> 0;
  let acc = 2166136261 >>> 0;
  for (let index = 0; index < asset.length; index += 1) {
    acc = (acc ^ asset.charCodeAt(index)) >>> 0;
    acc = wrappingMul(acc, 16777619);
  }
  acc = (acc ^ bucket) >>> 0;
  acc = wrappingMul(acc, 16777619);
  return 2_500_000 + (acc % 1_500_000);
}

export function tapeLines(unixSecs = Math.floor(Date.now() / 1000)) {
  const rows: Array<[string, string]> = [
    ["NEAR", "nep141:wrap.near"],
    ["ETH", "nep141:eth.omft.near"],
    ["BTC", "nep141:btc.omft.near"],
  ];
  return rows.map(([label, asset]) => {
    const dollars = simulatedPriceMicros(asset, unixSecs) / 1_000_000;
    return `${label}  ${dollars.toFixed(2)}`;
  });
}

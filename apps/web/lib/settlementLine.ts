export type SettlementPot = {
  outcome?: string;
  rule?: string;
  settled?: boolean;
  finality?: string;
  txHash?: string;
  noPot?: boolean;
  reason?: string;
};

export function settlementLine(pot: SettlementPot, wait: boolean): string {
  if (pot.settled && pot.finality === "FINAL" && pot.txHash) {
    return `Settled ${pot.rule || pot.outcome || ""}. Receipt ${pot.txHash}.`;
  }
  if (pot.noPot) {
    const reason = pot.reason?.trim();
    return reason || "No on-chain pot was opened, so nothing was paid.";
  }
  const outcome = pot.outcome ? ` (${pot.outcome})` : "";
  return wait
    ? `Not settled${outcome}. Waiting for a FINAL settle_vault receipt.`
    : `Not settled${outcome}.`;
}

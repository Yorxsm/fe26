export type ChainEconomy = {
  network: string;
  market: string;
  wallet: string;
  explorer: string;
};

export function configuredEconomy(): ChainEconomy {
  return {
    network: process.env.NEXT_PUBLIC_FE2VI_CHAIN || "testnet",
    market: process.env.NEXT_PUBLIC_FE2VI_MARKET_ID || "market.fe2vidisp7a3c.testnet",
    wallet: process.env.NEXT_PUBLIC_FE2VI_WALLET || "https://testnet.mynearwallet.com",
    explorer: process.env.NEXT_PUBLIC_FE2VI_EXPLORER || "https://testnet.nearblocks.io",
  };
}

export type BuyHandoff =
  | { ok: false; reason: string }
  | {
      ok: true;
      login: string;
      method: "buy";
      args: { token_id: string };
      deposit: string;
      gas: "80000000000000";
      receiverId: string;
      network: "testnet";
      explorer: string;
    };

/** Opens the testnet wallet on the market contract. Mainnet targets fail closed. */
export function buyHandoff(
  economy: ChainEconomy,
  tokenId: string,
  priceYocto: string,
  callback: string,
): BuyHandoff {
  if (economy.network !== "testnet" || !economy.market.endsWith(".testnet")) {
    return { ok: false, reason: "Passport buys are closed unless the market is a testnet account." };
  }
  if (economy.wallet.toLowerCase().includes("mainnet") || !priceYocto || priceYocto === "0") {
    return { ok: false, reason: "A testnet wallet and a chain ask are required before buying." };
  }
  const wallet = economy.wallet.replace(/\/$/, "");
  const login = new URL("/login/", `${wallet}/`);
  const success = new URL(callback);
  success.searchParams.set("buy", tokenId);
  login.searchParams.set("success_url", success.toString());
  login.searchParams.set("failure_url", callback);
  login.searchParams.set("contract_id", economy.market);
  const explorer = economy.explorer.replace(/\/$/, "");
  return {
    ok: true,
    login: login.toString(),
    method: "buy",
    args: { token_id: tokenId },
    deposit: priceYocto,
    gas: "80000000000000",
    receiverId: economy.market,
    network: "testnet",
    explorer: `${explorer}/address/${economy.market}`,
  };
}

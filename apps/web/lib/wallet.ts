import { Action, FunctionCall } from "@near-js/transactions";
import { setupWalletSelector, type WalletSelector } from "@near-wallet-selector/core";
import { setupModal, type WalletSelectorModal } from "@near-wallet-selector/modal-ui";
import { setupMyNearWallet } from "@near-wallet-selector/my-near-wallet";

export const PLACEHOLDER_CONTRACTS = {
  factory: process.env.NEXT_PUBLIC_FE2VI_FACTORY_ID ?? "factory.fe2vi.testnet",
  registry: process.env.NEXT_PUBLIC_FE2VI_REGISTRY_ID ?? "registry.fe2vi.testnet",
  heist: process.env.NEXT_PUBLIC_FE2VI_HEIST_ID ?? "heist.fe2vi.testnet",
};

export function assertTestnet(): "testnet" {
  const configured = process.env.NEXT_PUBLIC_FE2VI_NETWORK ?? "testnet";
  if (configured !== "testnet") {
    throw new Error("fe2VI refuses any network other than testnet");
  }
  return "testnet";
}

export type CallArgs = {
  receiverId: string;
  methodName: string;
  args: Record<string, unknown>;
  deposit?: string;
  gas?: string;
};

type Loaded = { selector: WalletSelector; modal: WalletSelectorModal };

let selectorPromise: Promise<Loaded> | null = null;

async function loadSelector(): Promise<Loaded> {
  assertTestnet();
  if (!selectorPromise) {
    selectorPromise = (async () => {
      const selector = await setupWalletSelector({
        network: "testnet",
        modules: [setupMyNearWallet()],
      });
      const modal = setupModal(selector, { contractId: PLACEHOLDER_CONTRACTS.factory });
      return { selector, modal };
    })();
  }
  return selectorPromise;
}

function functionCall(call: CallArgs): Action {
  return new Action({
    functionCall: new FunctionCall({
      methodName: call.methodName,
      args: new TextEncoder().encode(JSON.stringify(call.args)),
      gas: BigInt(call.gas ?? "100000000000000"),
      deposit: BigInt(call.deposit ?? "1"),
    }),
  });
}

export async function connectTestnetWallet(): Promise<string> {
  const { selector, modal } = await loadSelector();
  const accounts = selector.store.getState().accounts;
  if (!accounts.length) {
    modal.show();
    throw new Error("Connect a testnet wallet in the selector, then sign again. fe2VI does not create accounts.");
  }
  return accounts[0]?.accountId ?? "connected";
}

export async function signAndSend(call: CallArgs): Promise<unknown> {
  assertTestnet();
  const { selector, modal } = await loadSelector();
  const accounts = selector.store.getState().accounts;
  if (!accounts.length) {
    modal.show();
    throw new Error("No testnet account is connected. fe2VI will not create or fund one.");
  }
  const wallet = await selector.wallet();
  return wallet.signAndSendTransaction({
    receiverId: call.receiverId,
    actions: [functionCall(call)],
  });
}

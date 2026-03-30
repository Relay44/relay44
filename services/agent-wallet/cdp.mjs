import { Coinbase, Wallet } from "@coinbase/coinbase-sdk";
import { createWalletClient, createPublicClient, http, custom } from "viem";
import { base } from "viem/chains";

let _wallet = null;
let _account = null;

export async function initCdpWallet({ chainId = 8453, rpcUrls = [] } = {}) {
  const apiKeyName = process.env.CDP_API_KEY_NAME?.trim();
  const apiKeyPrivateKey = process.env.CDP_API_KEY_PRIVATE_KEY?.trim();

  if (!apiKeyName || !apiKeyPrivateKey) {
    throw new Error("CDP_API_KEY_NAME and CDP_API_KEY_PRIVATE_KEY are required");
  }

  Coinbase.configure({ apiKeyName, privateKey: apiKeyPrivateKey });

  const walletId = process.env.CDP_WALLET_ID?.trim();
  if (walletId) {
    _wallet = await Wallet.fetch(walletId);
  } else {
    _wallet = await Wallet.create({ networkId: "base-mainnet" });
    console.log(`Created new CDP wallet: ${_wallet.getId()}`);
    console.log("Set CDP_WALLET_ID to persist this wallet across restarts.");
  }

  const address = await _wallet.getDefaultAddress();
  const addr = address.getId();

  _account = {
    address: addr,
    type: "local",
    signMessage: async ({ message }) => {
      const signed = await address.signMessage(message);
      return signed;
    },
    signTransaction: async (tx) => {
      const signed = await address.signTransaction(tx);
      return signed;
    },
  };

  const chain = { ...base, id: chainId };
  const transport = rpcUrls.length
    ? http(rpcUrls[0])
    : http("https://mainnet.base.org");

  return {
    account: _account,
    wallet: _wallet,
    chain,
    publicClient: createPublicClient({ chain, transport }),
    walletClient: createWalletClient({ account: _account, chain, transport }),
    address: addr,
  };
}

export function getCdpWallet() {
  return _wallet;
}

export function getCdpAccount() {
  return _account;
}

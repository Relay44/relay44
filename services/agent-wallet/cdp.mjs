import { Coinbase, Wallet } from "@coinbase/coinbase-sdk";
import { createWalletClient, createPublicClient, http } from "viem";
import { base } from "viem/chains";

let _initPromise = null;
let _result = null;

export async function initCdpWallet({ chainId = 8453, rpcUrls = [] } = {}) {
  if (_initPromise) return _initPromise;

  _initPromise = _doInit({ chainId, rpcUrls });
  try {
    _result = await _initPromise;
    return _result;
  } catch (err) {
    _initPromise = null;
    throw err;
  }
}

async function _doInit({ chainId, rpcUrls }) {
  const apiKeyName = process.env.CDP_API_KEY_NAME?.trim();
  const apiKeyPrivateKey = process.env.CDP_API_KEY_PRIVATE_KEY?.trim();

  if (!apiKeyName || !apiKeyPrivateKey) {
    throw new Error("CDP_API_KEY_NAME and CDP_API_KEY_PRIVATE_KEY are required");
  }

  Coinbase.configure({ apiKeyName, privateKey: apiKeyPrivateKey });

  let wallet;
  const walletId = process.env.CDP_WALLET_ID?.trim();
  if (walletId) {
    wallet = await Wallet.fetch(walletId);
  } else {
    wallet = await Wallet.create({ networkId: "base-mainnet" });
    console.log("New CDP wallet created. Set CDP_WALLET_ID=%s to persist.", wallet.getId());
  }

  const defaultAddress = await wallet.getDefaultAddress();
  const addr = defaultAddress.getId();

  const account = {
    address: addr,
    type: "local",
    signMessage: async ({ message }) => defaultAddress.signMessage(message),
    signTransaction: async (tx) => defaultAddress.signTransaction(tx),
    signTypedData: async () => {
      throw new Error("signTypedData not supported by CDP wallet");
    },
  };

  const chain = { ...base, id: chainId };
  const rpcUrl = rpcUrls.length
    ? rpcUrls[0]
    : process.env.BASE_RPC_URL || "https://mainnet.base.org";
  const transport = http(rpcUrl);

  return {
    account,
    wallet,
    chain,
    publicClient: createPublicClient({ chain, transport }),
    walletClient: createWalletClient({ account, chain, transport }),
    address: addr,
  };
}

export function getCdpWallet() {
  return _result?.wallet ?? null;
}

export function getCdpAccount() {
  return _result?.account ?? null;
}

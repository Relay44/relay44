#!/usr/bin/env node

import express from 'express';
import { x402Facilitator } from '@x402/core/facilitator';
import { registerExactEvmScheme } from '@x402/evm/exact/facilitator';
import { toFacilitatorEvmSigner } from '@x402/evm';
import { createPublicClient, createWalletClient, fallback, http } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { base } from 'viem/chains';

const HOST = process.env.X402_FACILITATOR_HOST || '0.0.0.0';
const PORT = Number(process.env.PORT || process.env.X402_FACILITATOR_PORT || 8091);
const SHARED_SECRET = String(process.env.X402_FACILITATOR_SHARED_SECRET || '').trim();
const CHAIN_ID = Number(process.env.BASE_CHAIN_ID || base.id);
const PRIMARY_RPC = envOrThrow('BASE_RPC_URL');
const FALLBACK_RPCS = String(process.env.BASE_RPC_FALLBACK_URLS || '')
  .split(',')
  .map((entry) => entry.trim())
  .filter(Boolean)
  .filter((entry) => entry !== PRIMARY_RPC);
const NETWORK = `eip155:${CHAIN_ID}`;
const PRIVATE_KEY = envOrThrow('X402_FACILITATOR_PRIVATE_KEY');

function envOrThrow(key) {
  const value = String(process.env[key] || '').trim();
  if (!value) {
    throw new Error(`${key} is required`);
  }
  return value;
}

function createTransport(urls) {
  if (urls.length === 1) {
    return http(urls[0], { timeout: 15_000 });
  }

  return fallback(
    urls.map((url) => http(url, { timeout: 15_000 })),
    {
      rank: false,
      retryCount: 1,
    },
  );
}

function jsonSafe(value) {
  return JSON.parse(
    JSON.stringify(value, (_key, current) => (typeof current === 'bigint' ? current.toString() : current)),
  );
}

function normalizeError(error) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function requireSharedSecret(req, res, next) {
  if (!SHARED_SECRET) {
    next();
    return;
  }

  const header = String(req.headers.authorization || '');
  if (header === `Bearer ${SHARED_SECRET}`) {
    next();
    return;
  }

  res.status(401).json({ error: 'unauthorized' });
}

const account = privateKeyToAccount(PRIVATE_KEY);
const transport = createTransport([PRIMARY_RPC, ...FALLBACK_RPCS]);
const chain = { ...base, id: CHAIN_ID };
const publicClient = createPublicClient({ chain, transport });
const walletClient = createWalletClient({ account, chain, transport });

const signer = toFacilitatorEvmSigner({
  address: account.address,
  readContract: (args) => publicClient.readContract(args),
  verifyTypedData: (args) => publicClient.verifyTypedData(args),
  writeContract: (args) => walletClient.writeContract({ account, ...args }),
  sendTransaction: (args) => walletClient.sendTransaction({ account, ...args }),
  waitForTransactionReceipt: (args) => publicClient.waitForTransactionReceipt(args),
  getCode: (args) => publicClient.getCode(args),
});

const facilitator = new x402Facilitator();
registerExactEvmScheme(facilitator, {
  signer,
  networks: NETWORK,
});

const app = express();
app.use(express.json({ limit: '512kb' }));

app.get('/health', async (_req, res) => {
  try {
    const blockNumber = await publicClient.getBlockNumber();
    res.json({
      ok: true,
      network: NETWORK,
      facilitatorAddress: account.address,
      rpcEndpoints: 1 + FALLBACK_RPCS.length,
      latestBlock: blockNumber.toString(),
    });
  } catch (error) {
    res.status(500).json({
      ok: false,
      error: normalizeError(error),
      network: NETWORK,
      facilitatorAddress: account.address,
    });
  }
});

app.get('/supported', requireSharedSecret, async (_req, res) => {
  try {
    res.json(jsonSafe(facilitator.getSupported()));
  } catch (error) {
    res.status(500).json({ error: normalizeError(error) });
  }
});

app.post('/verify', requireSharedSecret, async (req, res) => {
  try {
    const { paymentPayload, paymentRequirements } = req.body || {};
    const result = await facilitator.verify(paymentPayload, paymentRequirements);
    res.status(result.isValid ? 200 : 402).json(jsonSafe(result));
  } catch (error) {
    res.status(400).json({
      isValid: false,
      invalidReason: 'invalid_request',
      invalidMessage: normalizeError(error),
    });
  }
});

app.post('/settle', requireSharedSecret, async (req, res) => {
  try {
    const { paymentPayload, paymentRequirements } = req.body || {};
    const result = await facilitator.settle(paymentPayload, paymentRequirements);
    res.status(result.success ? 200 : 402).json(jsonSafe(result));
  } catch (error) {
    res.status(400).json({
      success: false,
      errorReason: 'invalid_request',
      errorMessage: normalizeError(error),
      transaction: '',
      network: NETWORK,
    });
  }
});

app.listen(PORT, HOST, () => {
  console.log(`x402 facilitator listening on http://${HOST}:${PORT}`);
});


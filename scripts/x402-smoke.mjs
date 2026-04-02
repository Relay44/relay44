#!/usr/bin/env node

import { pathToFileURL } from 'node:url';

import { sendAlert } from './ops-alerts.mjs';
import { isEnabled } from './runner-framework.mjs';

const USDC_DECIMALS = 6;
const DEFAULT_API_URL = 'https://relay44-api.onrender.com/v1';
const DEFAULT_PATH = '/evm/markets/12/orderbook?outcome=yes&depth=5';
const DEFAULT_RPC_URL = 'https://mainnet.base.org';
const DEFAULT_USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';

function parseUsdc(value) {
  const normalized = String(value || '0').trim();
  if (!/^\d+(\.\d+)?$/.test(normalized)) {
    throw new Error(`invalid USDC amount: ${value}`);
  }

  const [whole, fraction = ''] = normalized.split('.');
  return BigInt(whole) * 1_000_000n + BigInt((fraction + '000000').slice(0, USDC_DECIMALS));
}

function buildTargetUrl(env) {
  const explicit = (env.X402_SMOKE_TARGET_URL || '').trim();
  if (explicit) {
    return explicit;
  }

  const apiUrl = (env.X402_SMOKE_API_URL || DEFAULT_API_URL).trim().replace(/\/+$/, '');
  const path = (env.X402_SMOKE_PATH || DEFAULT_PATH).trim();
  return `${apiUrl}${path.startsWith('/') ? path : `/${path}`}`;
}

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function shouldRunScheduledSmoke(now, env = process.env) {
  if (!isEnabled(env.X402_SMOKE_ENABLED, false)) {
    return false;
  }

  const minute = Number(env.X402_SMOKE_MINUTE ?? '0');
  const intervalHours = Number(env.X402_SMOKE_INTERVAL_HOURS ?? '1');
  const windowMinutes = Number(env.X402_SMOKE_WINDOW_MINUTES ?? '5');
  if (!Number.isInteger(minute) || minute < 0 || minute > 59) {
    throw new Error(`invalid X402_SMOKE_MINUTE: ${env.X402_SMOKE_MINUTE}`);
  }
  if (!Number.isInteger(intervalHours) || intervalHours < 1 || intervalHours > 24) {
    throw new Error(
      `invalid X402_SMOKE_INTERVAL_HOURS: ${env.X402_SMOKE_INTERVAL_HOURS}`,
    );
  }
  if (!Number.isInteger(windowMinutes) || windowMinutes < 1 || windowMinutes > 60) {
    throw new Error(
      `invalid X402_SMOKE_WINDOW_MINUTES: ${env.X402_SMOKE_WINDOW_MINUTES}`,
    );
  }

  const currentMinute = now.getUTCMinutes();
  const inWindow =
    currentMinute >= minute && currentMinute < Math.min(minute + windowMinutes, 60);
  return inWindow && now.getUTCHours() % intervalHours === 0;
}

async function readUsdcBalance(publicClient, parseAbi, token, wallet) {
  const erc20Abi = parseAbi(['function balanceOf(address owner) view returns (uint256)']);
  return publicClient.readContract({
    address: token,
    abi: erc20Abi,
    functionName: 'balanceOf',
    args: [wallet],
  });
}

export async function runX402Smoke(env = process.env) {
  if (!isEnabled(env.X402_SMOKE_ENABLED, false)) {
    return { ok: true, skipped: true, reason: 'x402 smoke disabled' };
  }

  const privateKey = (env.X402_SMOKE_PAYER_PRIVATE_KEY || '').trim();
  if (!privateKey) {
    throw new Error('X402_SMOKE_PAYER_PRIVATE_KEY is required');
  }

  const targetUrl = buildTargetUrl(env);
  const timeoutMs = Number(env.X402_SMOKE_TIMEOUT_MS || '30000');
  const minUsdc = parseUsdc(env.X402_SMOKE_MIN_USDC || '1');
  const rpcUrl = (env.X402_SMOKE_RPC_URL || env.BASE_RPC_URL || DEFAULT_RPC_URL).trim();
  const usdcAddress = (env.X402_SMOKE_USDC_ADDRESS || DEFAULT_USDC_ADDRESS).trim();

  const [
    { x402Client },
    { x402HTTPClient },
    { ExactEvmScheme },
    { createPublicClient, createWalletClient, formatUnits, http, parseAbi, publicActions },
    { privateKeyToAccount },
    { base },
  ] = await Promise.all([
    import('@x402/core/client'),
    import('@x402/core/http'),
    import('@x402/evm'),
    import('viem'),
    import('viem/accounts'),
    import('viem/chains'),
  ]);

  const account = privateKeyToAccount(privateKey);
  const publicClient = createPublicClient({
    chain: base,
    transport: http(rpcUrl),
  });
  const walletClient = createWalletClient({
    account,
    chain: base,
    transport: http(rpcUrl),
  }).extend(publicActions);
  const evmSigner = {
    address: account.address,
    signTypedData: (args) => walletClient.signTypedData(args),
    readContract: (args) => publicClient.readContract(args),
    getTransactionCount: (args) => publicClient.getTransactionCount(args),
    estimateFeesPerGas: () => publicClient.estimateFeesPerGas(),
    signTransaction: (args) => walletClient.signTransaction(args),
  };

  const balanceBefore = await readUsdcBalance(
    publicClient,
    parseAbi,
    usdcAddress,
    account.address,
  );
  if (balanceBefore < minUsdc) {
    throw new Error(
      `x402 smoke wallet underfunded: ${formatUnits(balanceBefore, USDC_DECIMALS)} USDC < ${formatUnits(minUsdc, USDC_DECIMALS)} USDC`,
    );
  }

  const httpClient = new x402HTTPClient(
    new x402Client().register(
      'eip155:*',
      new ExactEvmScheme(evmSigner),
    ),
  );

  const first = await fetch(targetUrl, {
    headers: { accept: 'application/json' },
    signal: AbortSignal.timeout(timeoutMs),
  });
  const firstText = await first.text();
  const firstBody = parseJson(firstText);
  if (first.status !== 402) {
    throw new Error(
      `expected 402 from ${targetUrl}, got ${first.status}: ${firstText.slice(0, 200)}`,
    );
  }
  if (!firstBody) {
    throw new Error('x402 quote response was not valid JSON');
  }

  const paymentRequired = httpClient.getPaymentRequiredResponse(
    (name) => first.headers.get(name),
    firstBody,
  );
  const paymentPayload = await httpClient.createPaymentPayload(paymentRequired);
  const acceptedAmount = paymentRequired.accepts?.[0]?.amount || null;

  const second = await fetch(targetUrl, {
    headers: {
      accept: 'application/json',
      ...httpClient.encodePaymentSignatureHeader(paymentPayload),
    },
    signal: AbortSignal.timeout(timeoutMs),
  });
  const secondText = await second.text();
  const secondBody = parseJson(secondText);
  if (second.status !== 200) {
    throw new Error(
      `expected 200 after payment from ${targetUrl}, got ${second.status}: ${secondText.slice(0, 200)}`,
    );
  }
  if (!secondBody) {
    throw new Error('paid x402 response was not valid JSON');
  }

  const settlement = httpClient.getPaymentSettleResponse((name) => second.headers.get(name));
  if (!settlement?.success) {
    throw new Error('x402 settlement header indicated failure');
  }
  if (settlement.payer && settlement.payer.toLowerCase() !== account.address.toLowerCase()) {
    throw new Error(`unexpected x402 payer in settlement: ${settlement.payer}`);
  }

  const balanceAfter = await readUsdcBalance(
    publicClient,
    parseAbi,
    usdcAddress,
    account.address,
  );

  return {
    ok: true,
    targetUrl,
    payer: account.address,
    firstStatus: first.status,
    secondStatus: second.status,
    paymentResponseHeaderPresent: second.headers.has('payment-response'),
    acceptedMicrousdc: acceptedAmount,
    balanceBeforeMicrousdc: balanceBefore.toString(),
    balanceBeforeUsdc: formatUnits(balanceBefore, USDC_DECIMALS),
    balanceAfterMicrousdc: balanceAfter.toString(),
    balanceAfterUsdc: formatUnits(balanceAfter, USDC_DECIMALS),
    chargedMicrousdc: balanceBefore > balanceAfter ? (balanceBefore - balanceAfter).toString() : '0',
    settlement,
  };
}

async function main() {
  try {
    const result = await runX402Smoke(process.env);
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    const message = `[relay44 ALERT] x402 smoke failed: ${error.message}`;
    console.error(
      JSON.stringify(
        {
          ok: false,
          message,
        },
        null,
        2,
      ),
    );
    await sendAlert(message, process.env);
    process.exit(1);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}

export { shouldRunScheduledSmoke };

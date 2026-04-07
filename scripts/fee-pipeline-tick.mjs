#!/usr/bin/env node

/**
 * Fee pipeline: collects protocol fees, swaps USDC→RELAY via Aerodrome,
 * funds RewardDistributor + burns a share.
 *
 * Flow:
 *   1. OrderBook.withdrawFees() → USDC to feeRecipient vault balance
 *   2. CollateralVault.withdraw() → USDC to keeper wallet
 *   3. Aerodrome swap USDC → RELAY
 *   4. Split: burn share to 0x...dEaD, rest to RewardDistributor
 */

import { createPublicClient, createWalletClient, http, parseAbi, formatUnits, maxUint256 } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { base } from 'viem/chains';

const RPC_URL = process.env.BASE_RPC_URL || 'https://mainnet.base.org';
const PRIVATE_KEY = process.env.FEE_PIPELINE_PRIVATE_KEY || process.env.REWARD_KEEPER_PRIVATE_KEY;
const MIN_USDC = BigInt(process.env.FEE_PIPELINE_MIN_USDC || '10000000'); // 10 USDC (6 decimals)
const BURN_BPS = Number(process.env.BUYBACK_BURN_SHARE_BPS || '2000'); // 20%
const SLIPPAGE_BPS = Number(process.env.FEE_PIPELINE_SLIPPAGE_BPS || '500'); // 5%

const ORDERBOOK = process.env.ORDER_BOOK_ADDRESS || '0xFe8aA303Ab953037023b12326D354f6d2484D4ce';
const VAULT = process.env.COLLATERAL_VAULT_ADDRESS || '0x4420dd803e6E363e6af079e6b77CA03B93f8dAe0';
const USDC = process.env.USDC_ADDRESS || '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
const RELAY = process.env.RELAY_TOKEN_ADDRESS || '0x580fF5Ae64eC792A949c6123386A8A936c7EBB07';
const DISTRIBUTOR = process.env.REWARD_DISTRIBUTOR_ADDRESS || '0x3c4c0A74F9d108F966908a835a9b4b8D946bBce3';
const DEAD = '0x000000000000000000000000000000000000dEaD';

// Aerodrome Slipstream on Base
const AERO_SWAP_ROUTER = process.env.AERODROME_SWAP_ROUTER || '0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5';
const POOL_TICK_SPACING = Number(process.env.AERO_TICK_SPACING || '200');

if (!PRIVATE_KEY) {
  console.error('FATAL: FEE_PIPELINE_PRIVATE_KEY required');
  process.exit(1);
}

const orderBookAbi = parseAbi([
  'function accruedFees() view returns (uint256)',
  'function withdrawFees() external',
  'function feeRecipient() view returns (address)',
]);

const vaultAbi = parseAbi([
  'function availableBalance(address) view returns (uint256)',
  'function withdraw(uint256) external',
]);

const erc20Abi = parseAbi([
  'function balanceOf(address) view returns (uint256)',
  'function approve(address,uint256) external returns (bool)',
  'function allowance(address,address) view returns (uint256)',
  'function transfer(address,uint256) external returns (bool)',
]);

const swapRouterAbi = parseAbi([
  'function exactInputSingle((address tokenIn, address tokenOut, int24 tickSpacing, address recipient, uint256 deadline, uint256 amountIn, uint256 amountOutMinimum, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut)',
]);

const quoterAbi = parseAbi([
  'function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, int24 tickSpacing, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)',
]);

const AERO_QUOTER = process.env.AERODROME_QUOTER || '0x254cF9E1E6e233aa1AC962CB9B05b2cfeAaE15b0';

const transport = http(RPC_URL, { timeout: 30_000 });
const chain = { ...base, id: Number(process.env.BASE_CHAIN_ID || 8453) };
const account = privateKeyToAccount(PRIVATE_KEY);
const publicClient = createPublicClient({ chain, transport });
const walletClient = createWalletClient({ account, chain, transport });

async function tick() {
  const result = { ok: true, phases: {} };

  // Phase 1: Check accrued fees
  const accruedFees = await publicClient.readContract({
    address: ORDERBOOK, abi: orderBookAbi, functionName: 'accruedFees',
  });

  result.phases.fees = { accruedUsdc: formatUnits(accruedFees, 6) };

  if (accruedFees >= MIN_USDC) {
    const hash = await walletClient.writeContract({
      address: ORDERBOOK, abi: orderBookAbi, functionName: 'withdrawFees',
    });
    await publicClient.waitForTransactionReceipt({ hash });
    result.phases.fees.withdrawn = true;
    result.phases.fees.txHash = hash;
  }

  // Phase 2: Check vault balance and withdraw USDC
  const vaultBalance = await publicClient.readContract({
    address: VAULT, abi: vaultAbi, functionName: 'availableBalance', args: [account.address],
  });

  result.phases.vault = { availableUsdc: formatUnits(vaultBalance, 6) };

  if (vaultBalance < MIN_USDC) {
    result.action = 'skip';
    result.reason = `vault balance ${formatUnits(vaultBalance, 6)} USDC below threshold ${formatUnits(MIN_USDC, 6)}`;
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  const withdrawHash = await walletClient.writeContract({
    address: VAULT, abi: vaultAbi, functionName: 'withdraw', args: [vaultBalance],
  });
  await publicClient.waitForTransactionReceipt({ hash: withdrawHash });
  result.phases.vault.withdrawn = true;

  // Phase 3: Approve and swap USDC→RELAY on Aerodrome
  const usdcBalance = await publicClient.readContract({
    address: USDC, abi: erc20Abi, functionName: 'balanceOf', args: [account.address],
  });

  const allowance = await publicClient.readContract({
    address: USDC, abi: erc20Abi, functionName: 'allowance', args: [account.address, AERO_SWAP_ROUTER],
  });

  if (allowance < usdcBalance) {
    const approveHash = await walletClient.writeContract({
      address: USDC, abi: erc20Abi, functionName: 'approve', args: [AERO_SWAP_ROUTER, maxUint256],
    });
    await publicClient.waitForTransactionReceipt({ hash: approveHash });
  }

  // Get quote for slippage protection
  const { result: quoteResult } = await publicClient.simulateContract({
    address: AERO_QUOTER,
    abi: quoterAbi,
    functionName: 'quoteExactInputSingle',
    args: [{
      tokenIn: USDC,
      tokenOut: RELAY,
      amountIn: usdcBalance,
      tickSpacing: POOL_TICK_SPACING,
      sqrtPriceLimitX96: BigInt(0),
    }],
  });
  const expectedOut = quoteResult[0];
  const amountOutMinimum = expectedOut - (expectedOut * BigInt(SLIPPAGE_BPS)) / BigInt(10_000);

  const deadline = BigInt(Math.floor(Date.now() / 1000) + 300);
  const swapHash = await walletClient.writeContract({
    address: AERO_SWAP_ROUTER,
    abi: swapRouterAbi,
    functionName: 'exactInputSingle',
    args: [{
      tokenIn: USDC,
      tokenOut: RELAY,
      tickSpacing: POOL_TICK_SPACING,
      recipient: account.address,
      deadline,
      amountIn: usdcBalance,
      amountOutMinimum,
      sqrtPriceLimitX96: BigInt(0),
    }],
  });
  const swapReceipt = await publicClient.waitForTransactionReceipt({ hash: swapHash });

  const relayBalance = await publicClient.readContract({
    address: RELAY, abi: erc20Abi, functionName: 'balanceOf', args: [account.address],
  });

  result.phases.swap = {
    usdcIn: formatUnits(usdcBalance, 6),
    relayOut: formatUnits(relayBalance, 18),
    txHash: swapHash,
  };

  // Phase 4: Split — burn share + distributor share
  const burnAmount = (relayBalance * BigInt(BURN_BPS)) / BigInt(10_000);
  const distributorAmount = relayBalance - burnAmount;

  if (burnAmount > BigInt(0)) {
    const burnHash = await walletClient.writeContract({
      address: RELAY, abi: erc20Abi, functionName: 'transfer', args: [DEAD, burnAmount],
    });
    await publicClient.waitForTransactionReceipt({ hash: burnHash });
    result.phases.burn = { amount: formatUnits(burnAmount, 18), txHash: burnHash };
  }

  if (distributorAmount > BigInt(0)) {
    const fundHash = await walletClient.writeContract({
      address: RELAY, abi: erc20Abi, functionName: 'transfer', args: [DISTRIBUTOR, distributorAmount],
    });
    await publicClient.waitForTransactionReceipt({ hash: fundHash });
    result.phases.distribute = { amount: formatUnits(distributorAmount, 18), txHash: fundHash };
  }

  result.action = 'completed';
  console.log(JSON.stringify(result, null, 2));
}

tick().catch((error) => {
  console.error(JSON.stringify({ ok: false, error: error.message }, null, 2));
  process.exit(1);
});

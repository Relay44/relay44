import { createPublicClient, http, formatEther, type Hex } from 'viem';
import { base } from 'viem/chains';
import { BASE_RPC_ENDPOINT } from '@/lib/constants';

const client = createPublicClient({
  chain: base,
  transport: http(BASE_RPC_ENDPOINT),
});

const GAS_PRICE_ORACLE = '0x420000000000000000000000000000000000000F' as const;

const ORACLE_ABI = [
  {
    name: 'getL1Fee',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: '_data', type: 'bytes' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    name: 'getL1FeeUpperBound',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: '_unsignedTxSize', type: 'uint256' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    name: 'baseFee',
    type: 'function',
    stateMutability: 'view',
    inputs: [],
    outputs: [{ name: '', type: 'uint256' }],
  },
] as const;

export async function estimateL1Fee(serializedTx: Hex): Promise<bigint> {
  return client.readContract({
    address: GAS_PRICE_ORACLE,
    abi: ORACLE_ABI,
    functionName: 'getL1Fee',
    args: [serializedTx],
  });
}

export async function estimateL1FeeUpperBound(txSizeBytes: number): Promise<bigint> {
  return client.readContract({
    address: GAS_PRICE_ORACLE,
    abi: ORACLE_ABI,
    functionName: 'getL1FeeUpperBound',
    args: [BigInt(txSizeBytes)],
  });
}

export async function getBaseFee(): Promise<bigint> {
  return client.readContract({
    address: GAS_PRICE_ORACLE,
    abi: ORACLE_ABI,
    functionName: 'baseFee',
  });
}

const TYPICAL_ERC20_APPROVE_TX_SIZE = 200;
const TYPICAL_DEPOSIT_TX_SIZE = 250;
const TYPICAL_WITHDRAW_TX_SIZE = 220;
const TYPICAL_GAS_UNITS = 80_000n;

export async function estimateDepositFees(): Promise<{
  l1Fee: bigint;
  l2Fee: bigint;
  totalFee: bigint;
  totalFeeEth: string;
}> {
  const [l1Fee, baseFeeWei] = await Promise.all([
    estimateL1FeeUpperBound(TYPICAL_DEPOSIT_TX_SIZE + TYPICAL_ERC20_APPROVE_TX_SIZE),
    getBaseFee(),
  ]);
  const l2Fee = baseFeeWei * TYPICAL_GAS_UNITS;
  const totalFee = l1Fee + l2Fee;
  return { l1Fee, l2Fee, totalFee, totalFeeEth: formatEther(totalFee) };
}

export async function estimateWithdrawFees(): Promise<{
  l1Fee: bigint;
  l2Fee: bigint;
  totalFee: bigint;
  totalFeeEth: string;
}> {
  const [l1Fee, baseFeeWei] = await Promise.all([
    estimateL1FeeUpperBound(TYPICAL_WITHDRAW_TX_SIZE),
    getBaseFee(),
  ]);
  const l2Fee = baseFeeWei * TYPICAL_GAS_UNITS;
  const totalFee = l1Fee + l2Fee;
  return { l1Fee, l2Fee, totalFee, totalFeeEth: formatEther(totalFee) };
}

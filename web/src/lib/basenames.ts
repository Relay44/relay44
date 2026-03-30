import { createPublicClient, http, namehash, type Address } from 'viem';
import { base } from 'viem/chains';
import { BASE_RPC_ENDPOINT } from '@/lib/constants';

const client = createPublicClient({
  chain: base,
  transport: http(BASE_RPC_ENDPOINT),
});

const L2_RESOLVER = '0xC6d566A56A1aFf6508b41f6c90ff131615583BCD' as const;

const RESOLVER_ABI = [
  {
    name: 'name',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'node', type: 'bytes32' }],
    outputs: [{ name: '', type: 'string' }],
  },
  {
    name: 'addr',
    type: 'function',
    stateMutability: 'view',
    inputs: [{ name: 'node', type: 'bytes32' }],
    outputs: [{ name: '', type: 'address' }],
  },
] as const;

function addressToReverseNode(address: string): `0x${string}` {
  const addr = address.toLowerCase().replace('0x', '');
  return namehash(`${addr}.addr.reverse`);
}

export async function resolveBasename(address: string): Promise<string | null> {
  try {
    const node = addressToReverseNode(address);
    const name = await client.readContract({
      address: L2_RESOLVER,
      abi: RESOLVER_ABI,
      functionName: 'name',
      args: [node],
    });
    return name || null;
  } catch {
    return null;
  }
}

export async function resolveAddress(basename: string): Promise<Address | null> {
  try {
    const node = namehash(basename);
    const addr = await client.readContract({
      address: L2_RESOLVER,
      abi: RESOLVER_ABI,
      functionName: 'addr',
      args: [node],
    });
    if (!addr || addr === '0x0000000000000000000000000000000000000000') return null;
    return addr;
  } catch {
    return null;
  }
}

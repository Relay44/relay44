import { createPublicClient, http } from 'viem';
import { base } from 'viem/chains';
import { getContractAddress, marketCoreAbi } from '@relay44/protocol';

const rpcUrl = process.env.BASE_RPC_URL || 'https://mainnet.base.org';

const client = createPublicClient({
  chain: base,
  transport: http(rpcUrl),
});

const marketCore = getContractAddress('production', 'marketCore');

const marketCount = await client.readContract({
  address: marketCore,
  abi: marketCoreAbi,
  functionName: 'marketCount',
});

console.log({
  network: 'Base mainnet',
  marketCore,
  marketCount: marketCount.toString(),
});

if (marketCount > 0n) {
  const latestMarket = await client.readContract({
    address: marketCore,
    abi: marketCoreAbi,
    functionName: 'markets',
    args: [marketCount],
  });

  console.log({ latestMarket });
}

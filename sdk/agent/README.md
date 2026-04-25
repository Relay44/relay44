# @relay44/agent-sdk

TypeScript SDK for building trading agents on top of Relay44 prediction
markets. Wraps the Relay44 `/v1/evm/write/*` API and the on-chain
`MarketCore` / `OrderBook` contracts behind a single `TradingAgent` class
with risk management, strategies, and ERC-8004 helpers.

Protocol addresses and ABIs are re-exported from `@relay44/protocol`; the
agent SDK no longer maintains a separate ABI surface.

## Install

```bash
npm install @relay44/agent-sdk viem
```

## Quick start

```ts
import { createPublicClient, createWalletClient, http } from 'viem';
import { base } from 'viem/chains';
import { privateKeyToAccount } from 'viem/accounts';

import {
  createAgent,
  createDefaultRiskParams,
  getContractAddress,
  MomentumStrategy,
} from '@relay44/agent-sdk';

const publicClient = createPublicClient({ chain: base, transport: http() });
const account = privateKeyToAccount(process.env.PRIVATE_KEY as `0x${string}`);
const walletClient = createWalletClient({ chain: base, account, transport: http() });

const agent = createAgent({
  publicClient,
  walletClient,
  marketCoreAddress: getContractAddress('production', 'marketCore'),
  orderBookAddress: getContractAddress('production', 'orderBook'),
  evmWriteApiUrl: 'https://relay44-api.onrender.com/v1',
  config: {
    riskParams: createDefaultRiskParams(),
    availableBalance: 1_000_000n, // USDC (6 decimals)
    totalPnl: 0n,
  },
});

agent.setStrategy(new MomentumStrategy());
await agent.start([1n, 2n]); // poll these market IDs
```

## Contract ABIs

The SDK ships **canonical static ABIs** for the four core contracts:

```ts
import {
  MARKET_CORE_ABI,
  ORDER_BOOK_ABI,
  ERC20_ABI,
  RELAY_STAKING_ABI,
  MARKET_CREATED_EVENT_ABI,
  ORDER_PLACED_EVENT_ABI,
} from '@relay44/agent-sdk';
```

These are generated from `evm/out` by `@relay44/protocol` and checked in CI
against the deployment manifest.

### Fetching the live ABI

For long-lived agents that need to survive contract upgrades without a
redeploy, the SDK also exposes a `fetchContractAbi` helper that pulls the
canonical ABI from the public endpoint at runtime:

```ts
import { fetchContractAbi } from '@relay44/agent-sdk';

const { name, abi } = await fetchContractAbi('market-core');
const count = await publicClient.readContract({
  address: '0xc9259a18696Ecbf7636C1a01F40Bc9d47e249AE8',
  abi,
  functionName: 'marketCount',
});
```

Available names: `'market-core' | 'order-book' | 'erc20' | 'relay-staking'`.

The endpoint is cached for 1 hour at the edge. Override the base URL for
preview deploys via the `baseUrl` option or the `RELAY44_CONTRACTS_URL`
environment variable:

```ts
await fetchContractAbi('market-core', { baseUrl: 'https://preview.relay44.com' });
```

## Documentation

- [Developer docs](https://relay44.com/docs)
- [Protocol reference](https://relay44.com/docs/contracts)
- [Tokenomics](https://relay44.com/tokenomics)

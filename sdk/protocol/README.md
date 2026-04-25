# @relay44/protocol

Importable Relay44 Protocol artifacts for Base builders.

```ts
import {
  getContractAbi,
  getContractAddress,
  marketCoreAbi,
} from '@relay44/protocol';

const marketCore = getContractAddress('production', 'marketCore');
const abi = getContractAbi('marketCore');
```

The package is generated from `evm/out` and `config/deployments/base-addresses.json`.
Run `forge build --root evm` before `npm run sdk:protocol:generate` when contracts change.

## Exports

- `deploymentManifest`
- `productionAddresses`
- `stagingAddresses`
- typed ABI constants such as `marketCoreAbi`, `orderBookAbi`, and `relayStakingAbi`
- compatibility ABI constants such as `MARKET_CORE_ABI` and `ORDER_BOOK_ABI`
- `getContractAddress(network, contract)`
- `getContractAbi(contract)`

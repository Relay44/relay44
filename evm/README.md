# Relay44 EVM Contracts (Base)

This workspace contains the first Base-native contract set for the Solana -> Base pivot.

## Contracts
- `R44Token.sol`: ERC-20 token with cap, mint role, and pause controls.
- `MarketCore.sol`: market lifecycle skeleton (create, resolve, pause).
- `OrderBook.sol`: Base order lifecycle skeleton (place, cancel, partial fill).
- `CollateralVault.sol`: collateral custody primitives (deposit, withdraw, lock, unlock, settle).

## Prerequisites
- Foundry (`forge`, `cast`)
- A Base RPC endpoint
- A funded deployer account on Base Sepolia or Base Mainnet

## Environment
Set these variables before deploy:
- `BASE_RPC_URL`
- `BASE_SEPOLIA_RPC_URL`
- `BASESCAN_API_KEY`
- `BASE_ADMIN`
- `BASE_TREASURY`
- `R44_CAP_WEI`
- `R44_INITIAL_SUPPLY_WEI`
- `COLLATERAL_TOKEN_ADDRESS` (optional; if omitted, deploy script uses `R44Token` as collateral)

## Build and Test
```bash
forge build
forge test
```

## Secure Keystore Setup
```bash
cast wallet import deployer --interactive
```

## Deploy (Base Sepolia)
```bash
forge script script/DeployCore.s.sol:DeployCoreScript \
  --rpc-url $BASE_SEPOLIA_RPC_URL \
  --account deployer \
  --broadcast \
  --verify
```

## Deploy (Base Mainnet)
```bash
forge script script/DeployCore.s.sol:DeployCoreScript \
  --rpc-url $BASE_RPC_URL \
  --account deployer \
  --broadcast \
  --verify
```

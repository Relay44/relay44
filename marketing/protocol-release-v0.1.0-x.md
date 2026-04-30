# Relay44 Protocol v0.1.0 X Copy

Account: https://x.com/Relay44BASE

## Protocol Announcement

Relay44 Protocol v0.1.0 is the prediction market protocol for Base.

Contracts, ABIs, addresses, SDK packages, examples, and public protocol metrics are now part of the builder surface.

relay44.com is the reference app. The protocol is the product.

## Builder Quickstart Thread

1/ Build on Relay44 Protocol in minutes.

Install:
`npm install @relay44/protocol viem`

Read MarketCore:
`getContractAddress('production', 'marketCore')`
`marketCoreAbi`

Docs: https://relay44.com/docs/developers/quickstart

2/ The package exports production/staging manifests, typed addresses, generated ABIs from `evm/out`, and helpers:

`getContractAddress(network, contract)`
`getContractAbi(contract)`

3/ Example:

`examples/protocol-read-market` reads `MarketCore.marketCount` on Base mainnet and can be dropped into a fresh TypeScript project.

## $RELAY Utility Thread

1/ $RELAY is designed as protocol utility, not app points.

The goal is direct alignment with Relay44 Protocol usage: access, incentives, fee discounts, staking, rewards, and agent participation.

2/ Grounded current utility:

- staking tiers
- fee discounts
- reward distribution eligibility
- agent and creator incentives
- protocol access paths

No buyback or governance claims unless implemented on-chain.

3/ The value story is execution:

More builders, markets, settlement volume, collateral, and agents should make $RELAY more useful inside the protocol.

## Dashboard Post

Protocol metrics should be public.

Relay44 now exposes:

- total markets
- active markets
- settlement volume
- connected agents
- USDC collateral where available

Dashboard: https://relay44.com/protocol
Endpoint: https://relay44-api.onrender.com/v1/protocol/metrics

# Architecture

This document describes the module boundaries, data flow, and key design decisions in Relay44.

![Architecture diagram](assets/architecture/diagram-white.svg)

## Layers

### Public Edge

**Browser** — end-user wallet and browser. Connects to the web client via standard HTTP.

**Web client** (`web/`) — Next.js application using App Router, wagmi for wallet interaction, and TradingView for charting. Serves the trading UI at `relay44.com`. Calls the Rust API for all data and write operations.

### Rust API

**API** (`app/`) — Actix-web server that handles authentication (SIWE + JWT), market data, order routing, position management, compliance checks, and write preparation for on-chain transactions. All external clients interact through this layer.

Key modules in `app/src/`:

| Module | Responsibility |
| --- | --- |
| `api/` | HTTP route handlers organized by domain (markets, orders, auth, etc.) |
| `services/database.rs` | PostgreSQL connection pool, queries, migrations |
| `services/orderbook.rs` | In-memory order book with persistence |
| `services/evm_rpc.rs` | Base RPC client with fallback endpoints |
| `services/evm_indexer.rs` | Block-level log indexer for on-chain events |
| `services/event_bus.rs` | Internal pub/sub for cross-service communication |
| `services/websocket.rs` | WebSocket hub for real-time market and trade updates |
| `services/orchestrator.rs` | Background service lifecycle and startup |
| `services/risk_governor.rs` | Risk limits and exposure checks |
| `services/x402.rs` | x402 payment protocol integration |
| `middleware/` | Request ID, access logging, geo-blocking |
| `config/` | Environment-driven configuration |

### Execution Core

**Base contracts** (`evm/`) — Foundry workspace deployed on Base. Core contracts:

| Contract | Purpose |
| --- | --- |
| `MarketCore` | Market creation, resolution, and lifecycle |
| `OrderBook` | On-chain order matching |
| `CollateralVault` | USDC collateral custody |
| `DistributionMarket` | Distribution-style market settlement |
| `RelayToken` | Protocol token |
| `RelayStaking` | Token staking and rewards |
| `OracleResolver` | Oracle-based market resolution |
| `AgentRuntime` | Autonomous agent execution framework |

**PostgreSQL** — primary data store for markets, positions, orders, user state, and indexer cursors.

**Redis** — caching, rate limiting, and session state.

**x402 facilitator** — payment protocol service for gated API access.

### Operator Agents

Background automation that targets the API:

| Agent | Purpose |
| --- | --- |
| Bootstrap operator | SIWE-authenticated ladder runner for seed liquidity |
| Polymarket indexer | Syncs external market data with backfill and reconciliation |
| Liquidity mirror | Mirrors external venue depth |
| Hedge engine | Manages cross-venue hedging |
| Arb scanner | Identifies arbitrage opportunities across venues |
| Market creator | Automated market creation from external signals |
| Portfolio snapshotter | Periodic portfolio state capture |

## Data Flow

1. **Read path**: Browser → Web client → API → PostgreSQL/Redis → response
2. **Write path**: Browser → Web client → API → prepares transaction → wallet signs → Base contracts
3. **Indexing**: Base contracts emit events → EVM indexer polls logs → persists to PostgreSQL → EventBus → WebSocket push to clients
4. **Operator loop**: Agent authenticates via SIWE → fetches planned actions from API → signs and submits on-chain → reports receipts back

## Key Design Decisions

**API as the single gateway.** All reads and writes flow through the Rust API. The web client never calls Base RPC directly. This centralizes auth, compliance, rate limiting, and audit logging.

**In-memory order book with DB persistence.** The order book lives in memory for low-latency matching but snapshots to PostgreSQL for crash recovery. On startup, the API restores from the last snapshot.

**EVM indexer with rate-limit backoff.** The indexer uses exponential backoff (20s → 60s → 120s → 240s → 300s) when hitting Base RPC rate limits, with a configurable confirmation depth to avoid reorg sensitivity.

**Event-driven WebSocket updates.** Internal `EventBus` bridges platform events (trades, position changes, agent executions) to WebSocket clients without polling.

## SDK

`sdk/` provides a TypeScript client for programmatic access to the API. It handles authentication, request signing, and typed responses.

## Solana Programs

`programs/` contains experimental Anchor programs targeting Solana. These are not deployed in production and exist for research purposes.

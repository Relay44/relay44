// relay44 AI trading agent SDK

// Core agent
export { TradingAgent, createAgent } from './agent';

// Types
export {
  Address,
  PositionSizing,
  AgentStatus,
  Outcome,
  OrderType,
  RiskParams,
  TradingAgentConfig,
  OrderParams,
  Signal,
  MarketData,
  TradeResult,
  AgentMetrics,
} from './types';

// Strategies
export {
  Strategy,
  MomentumStrategy,
  MeanReversionStrategy,
  CompositeStrategy,
} from './strategy';

// Risk management
export {
  RiskManager,
  PositionTracker,
  ValidationResult,
  ValidationCheck,
  Position,
  createDefaultRiskParams,
} from './risk';

// ERC-8004 modules
export * from './erc8004';

// Protocol artifacts are owned by @relay44/protocol and re-exported here for
// backwards compatibility with agent SDK consumers.
export {
  deploymentManifest,
  productionAddresses,
  stagingAddresses,
  contractAbis,
  getContractAbi,
  getContractAddress,
} from '@relay44/protocol';
export type {
  Address as ProtocolAddress,
  ContractAbi,
  ContractName,
  NetworkName,
} from '@relay44/protocol';

export {
  MARKET_CORE_ABI,
  ORDER_BOOK_ABI,
  ERC20_ABI,
  RELAY_STAKING_ABI,
  MARKET_CREATED_EVENT_ABI,
  ORDER_PLACED_EVENT_ABI,
  fetchContractAbi,
  DEFAULT_CONTRACTS_BASE_URL,
} from './abis';
export type { ContractAbiName } from './abis';

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

export type { MarketContext, MarketInput, OddsSnapshot } from './polymarket.js';
export type { Claim, Source, NarrativeEdge, ContextGraphSnapshot, GraphNode, GraphEdge, ContextGraphResult } from './graph.js';
export type { MisinformationScore, ScoreComponent, ScoringWeights } from './scoring.js';

export type UAL = string;

export interface PublishResult {
  success: boolean;
  ual?: string;
  error?: string;
}

export interface DKGClientConfig {
  endpoint: string;
  port?: number;
  blockchain?: {
    name: string;
    publicKey?: string;
    privateKey?: string;
    rpc?: string;
  };
  maxRetries?: number;
  retryDelayMs?: number;
  timeoutMs?: number;
}

export interface ServiceConfig {
  host: string;
  port: number;
  dataDir: string;
  sharedSecret?: string;
  analysisTimeoutMs: number;
  maxConcurrentAnalyses: number;
  dkg: {
    endpoint?: string;
    port: number;
    blockchain: string;
    privateKey?: string;
    rpc?: string;
    epochs: number;
    paranetUAL?: string;
    workspaceUAL?: string;
  };
  polymarket: {
    gammaApi: string;
    clobApi: string;
  };
  llm: {
    apiKey?: string;
    model: string;
    enabled: boolean;
  };
  features: {
    dkgEnabled: boolean;
    llmEnabled: boolean;
    autoHedgeEnabled: boolean;
  };
}

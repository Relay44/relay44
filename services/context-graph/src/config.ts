import type { ServiceConfig } from './types/index.js';

function firstNonEmpty(keys: readonly string[]): string | undefined {
  for (const key of keys) {
    const raw = process.env[key];
    if (!raw) continue;
    const value = raw.trim();
    if (value) return value;
  }
  return undefined;
}

function envOrThrow(key: string): string {
  const value = String(process.env[key] || '').trim();
  if (!value) throw new Error(`${key} is required`);
  return value;
}

function normalizeDkgEndpoint(endpoint: string): string {
  const value = endpoint.trim();
  if (!value) return value;
  if (/^[a-zA-Z][a-zA-Z\d+.-]*:\/\//.test(value)) return value;
  return `http://${value}`;
}

function resolveDkgEndpoint(): string | undefined {
  const endpoint = firstNonEmpty([
    'DKG_ENDPOINT',
    'CONTEXT_GRAPH_DKG_ENDPOINT',
  ]);
  return endpoint ? normalizeDkgEndpoint(endpoint) : undefined;
}

function resolveDkgBlockchain(): string {
  const value = firstNonEmpty(['DKG_BLOCKCHAIN', 'CONTEXT_GRAPH_DKG_BLOCKCHAIN']);
  if (value === 'base:84532' || value === 'gnosis:100' || value === 'otp:2043') return value;
  return 'base:8453';
}

function resolveDkgPort(): number {
  const raw = firstNonEmpty(['DKG_PORT', 'CONTEXT_GRAPH_DKG_PORT']);
  const parsed = raw ? parseInt(raw, 10) : NaN;
  if (Number.isFinite(parsed) && parsed > 0 && parsed <= 65535) return parsed;
  return 8900;
}

function resolveDkgPrivateKey(): string | undefined {
  return firstNonEmpty(['DKG_PRIVATE_KEY', 'CONTEXT_GRAPH_DKG_PRIVATE_KEY']);
}

function resolveDkgRpc(): string | undefined {
  return firstNonEmpty(['DKG_RPC_URL', 'CONTEXT_GRAPH_DKG_RPC_URL']);
}

function resolveDkgEpochs(): number {
  const raw = firstNonEmpty(['DKG_EPOCHS', 'CONTEXT_GRAPH_DKG_EPOCHS']);
  const parsed = raw ? parseInt(raw, 10) : NaN;
  if (Number.isFinite(parsed) && parsed > 0) return parsed;
  return 12;
}

export function validateConfig(config: ServiceConfig): void {
  const warnings: string[] = [];

  if (config.features.dkgEnabled && !config.dkg.endpoint) {
    warnings.push('DKG enabled but no endpoint configured — falling back to mock client');
  }
  if (config.features.dkgEnabled && config.dkg.endpoint && !config.dkg.privateKey) {
    warnings.push('DKG endpoint set but no private key — publish operations will fail');
  }
  if (config.features.llmEnabled && !config.llm.apiKey) {
    warnings.push('LLM enabled but no ANTHROPIC_API_KEY — falling back to heuristic claim extraction');
  }
  if (!config.sharedSecret) {
    warnings.push('No SHARED_SECRET configured — write endpoints are unprotected');
  }

  for (const w of warnings) {
    console.warn(`[config] ${w}`);
  }
}

export function loadConfig(): ServiceConfig {
  const port = parseInt(process.env.CONTEXT_GRAPH_PORT || '3010', 10);
  if (!Number.isFinite(port) || port < 1 || port > 65535) {
    throw new Error(`Invalid CONTEXT_GRAPH_PORT: ${process.env.CONTEXT_GRAPH_PORT}`);
  }

  return {
    port,
    dataDir: process.env.CONTEXT_GRAPH_DATA_DIR || './data',
    sharedSecret: firstNonEmpty(['CONTEXT_GRAPH_SHARED_SECRET', 'SHARED_SECRET']),
    analysisTimeoutMs: parseInt(process.env.CONTEXT_GRAPH_TIMEOUT_MS || '120000', 10),
    maxConcurrentAnalyses: parseInt(process.env.CONTEXT_GRAPH_MAX_CONCURRENT || '3', 10),
    dkg: {
      endpoint: resolveDkgEndpoint(),
      port: resolveDkgPort(),
      blockchain: resolveDkgBlockchain(),
      privateKey: resolveDkgPrivateKey(),
      rpc: resolveDkgRpc(),
      epochs: resolveDkgEpochs(),
      paranetUAL: firstNonEmpty(['CONTEXT_GRAPH_PARANET_UAL', 'DKG_PARANET_UAL']),
      workspaceUAL: firstNonEmpty(['CONTEXT_GRAPH_WORKSPACE_UAL']),
    },
    polymarket: {
      gammaApi: process.env.POLYMARKET_GAMMA_API || 'https://gamma-api.polymarket.com',
      clobApi: process.env.POLYMARKET_CLOB_API || 'https://clob.polymarket.com',
    },
    llm: {
      apiKey: process.env.ANTHROPIC_API_KEY,
      model: process.env.CONTEXT_GRAPH_LLM_MODEL || 'claude-sonnet-4-5-20250514',
      enabled: process.env.CONTEXT_GRAPH_LLM_ENABLED !== 'false',
    },
    features: {
      dkgEnabled: process.env.CONTEXT_GRAPH_DKG_ENABLED !== 'false',
      llmEnabled: process.env.CONTEXT_GRAPH_LLM_ENABLED !== 'false',
      autoHedgeEnabled: process.env.CONTEXT_GRAPH_AUTO_HEDGE_ENABLED === 'true',
    },
  };
}

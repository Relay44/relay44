import type { NextRequest } from 'next/server';
import type { Web4Capabilities } from '@/lib/api';
import { SITE_URL } from '@/lib/seo';

const TRUTHY_VALUES = new Set(['1', 'true', 'yes', 'on']);

function readEnv(name: string): string {
  return String(process.env[name] || '').trim();
}

function readFlag(name: string, fallback: boolean): boolean {
  const value = readEnv(name);
  if (!value) {
    return fallback;
  }

  return TRUTHY_VALUES.has(value.toLowerCase());
}

function resolveOrigin(request?: NextRequest): string {
  const forwardedHost = request?.headers.get('x-forwarded-host')?.trim();
  const host = forwardedHost || request?.headers.get('host')?.trim();
  const forwardedProto = request?.headers.get('x-forwarded-proto')?.trim();
  const protocol = forwardedProto || request?.nextUrl.protocol.replace(':', '') || 'https';

  if (host) {
    return `${protocol}://${host}`;
  }

  return SITE_URL.replace(/\/$/, '');
}

export function buildLocalWeb4Capabilities(request?: NextRequest): Web4Capabilities {
  const chainMode = (readEnv('CHAIN_MODE') || readEnv('NEXT_PUBLIC_CHAIN_MODE') || 'base').toLowerCase();
  const evmReadsEnabled = readFlag('EVM_READS_ENABLED', true);
  const evmWritesEnabled = readFlag('EVM_WRITES_ENABLED', false);
  const solanaReadsEnabled = readFlag('SOLANA_READS_ENABLED', false);
  const solanaWritesEnabled = readFlag('SOLANA_WRITES_ENABLED', false);
  const externalMarketsEnabled = readFlag('EXTERNAL_MARKETS_ENABLED', true);
  const externalTradingEnabled = readFlag('EXTERNAL_TRADING_ENABLED', false);
  const externalAgentsEnabled = readFlag('EXTERNAL_AGENTS_ENABLED', false);
  const limitlessEnabled = readFlag('LIMITLESS_ENABLED', true);
  const polymarketEnabled = readFlag('POLYMARKET_ENABLED', true);
  const executionMode = (readEnv('EXTERNAL_EXECUTION_MODE') || 'paper').toLowerCase();

  const limitlessTradingReady =
    limitlessEnabled &&
    externalTradingEnabled &&
    Boolean(readEnv('LIMITLESS_API_KEY'));

  const polymarketTradingReady =
    polymarketEnabled &&
    externalMarketsEnabled &&
    externalTradingEnabled &&
    executionMode !== 'paper' &&
    Boolean(readEnv('POLYMARKET_GAMMA_API_BASE')) &&
    Boolean(readEnv('POLYMARKET_CLOB_API_BASE'));

  const beta =
    (limitlessEnabled && !limitlessTradingReady) ||
    (polymarketEnabled && !polymarketTradingReady);

  return {
    project: 'relay44',
    mode: 'web4-agent-market-network',
    chain_mode: chainMode,
    api_base: `${resolveOrigin(request)}/v1`,
    runtime: {
      evm_reads_enabled: evmReadsEnabled,
      evm_writes_enabled: evmWritesEnabled,
      solana_reads_enabled: solanaReadsEnabled,
      solana_writes_enabled: solanaWritesEnabled,
      external_markets_enabled: externalMarketsEnabled,
      external_trading_enabled: externalTradingEnabled,
      external_agents_enabled: externalAgentsEnabled,
      limitless_enabled: limitlessEnabled,
      polymarket_enabled: polymarketEnabled,
    },
    wallet: {
      read_enabled: evmReadsEnabled || solanaReadsEnabled,
      deposit_enabled: evmWritesEnabled,
      withdraw_enabled: evmWritesEnabled,
      claim_enabled: evmWritesEnabled,
      deposit_mode: evmWritesEnabled ? 'chain' : 'disabled',
      withdraw_mode: evmWritesEnabled ? 'chain' : 'disabled',
    },
    launch: {
      beta,
      limitless_trading_ready: limitlessTradingReady,
      polymarket_trading_ready: polymarketTradingReady,
    },
  };
}

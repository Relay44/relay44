export type CoreChain = 'solana' | 'base';
export type CoreChainQuery = CoreChain | 'all';

export type UnifiedMarketSource = 'core' | 'ledger' | 'jupiter' | 'pnp' | 'limitless' | 'all';

export interface ParsedMarketRef {
  raw: string;
  chain: CoreChain | null;
  coreRef: string;
  legacyMarketId: string | null;
  namespaced: boolean;
}

const LEGACY_MARKET_ID_PATTERN = /^mkt-[a-z0-9]+(?:[a-z0-9-]*)$/i;
const BASE_ADDRESS_PATTERN = /^0x[a-fA-F0-9]{40}$/;

export function parseChainQuery(raw: string | undefined): CoreChainQuery {
  if (raw === 'solana' || raw === 'base' || raw === 'all') return raw;
  return 'all';
}

export function parseUnifiedSource(raw: string | undefined): UnifiedMarketSource {
  if (raw === 'core' || raw === 'ledger' || raw === 'jupiter' || raw === 'pnp' || raw === 'limitless' || raw === 'all') {
    return raw;
  }
  return 'all';
}

export function parseMarketRef(rawMarketId: string): ParsedMarketRef {
  const raw = rawMarketId.trim();
  if (raw.startsWith('sol:')) {
    return {
      raw,
      chain: 'solana',
      coreRef: raw.slice(4),
      legacyMarketId: LEGACY_MARKET_ID_PATTERN.test(raw.slice(4)) ? raw.slice(4) : null,
      namespaced: true,
    };
  }
  if (raw.startsWith('base:')) {
    return {
      raw,
      chain: 'base',
      coreRef: raw.slice(5),
      legacyMarketId: null,
      namespaced: true,
    };
  }
  if (LEGACY_MARKET_ID_PATTERN.test(raw)) {
    return {
      raw,
      chain: 'solana',
      coreRef: raw,
      legacyMarketId: raw,
      namespaced: false,
    };
  }
  if (BASE_ADDRESS_PATTERN.test(raw)) {
    return {
      raw,
      chain: 'base',
      coreRef: raw,
      legacyMarketId: null,
      namespaced: false,
    };
  }

  return {
    raw,
    chain: null,
    coreRef: raw,
    legacyMarketId: null,
    namespaced: false,
  };
}

export function toNamespacedMarketId(chain: CoreChain, marketRef: string): string {
  if (chain === 'solana') return `sol:${marketRef}`;
  return `base:${marketRef}`;
}

export function isLegacyLedgerAlias(source: UnifiedMarketSource): boolean {
  return source === 'ledger';
}

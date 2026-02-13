function parseBoolean(raw: string | undefined, fallback: boolean): boolean {
  if (!raw) return fallback;
  const normalized = raw.trim().toLowerCase();
  if (normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on') return true;
  if (normalized === '0' || normalized === 'false' || normalized === 'no' || normalized === 'off') return false;
  return fallback;
}

const productionMode = process.env.NODE_ENV === 'production';

export function isSyntheticLedgerWriteEnabled(): boolean {
  return parseBoolean(process.env.SYNTHETIC_LEDGER_WRITES_ENABLED, !productionMode);
}

export function syntheticLedgerWriteBlockReason(action: string): string {
  return `synthetic ledger writes disabled for ${action}; use core adapter routes`;
}

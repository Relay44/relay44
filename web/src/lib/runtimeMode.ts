const TRUTHY_VALUES = new Set(['1', 'true', 'yes', 'on']);

interface RuntimeFlags {
  evm_writes_enabled: boolean;
  solana_writes_enabled: boolean;
  external_trading_enabled: boolean;
  external_agents_enabled: boolean;
}

export interface RuntimeCapabilities {
  runtime: RuntimeFlags;
  wallet?: {
    read_enabled: boolean;
    deposit_enabled: boolean;
    withdraw_enabled: boolean;
    claim_enabled: boolean;
    deposit_mode: 'chain' | 'disabled';
    withdraw_mode: 'chain' | 'disabled';
  };
}

function isEnabled(value: string | undefined): boolean {
  return TRUTHY_VALUES.has(String(value || '').trim().toLowerCase());
}

export const readOnlyPreviewEnabled = isEnabled(
  process.env.NEXT_PUBLIC_READ_ONLY_MODE
);

let currentCapabilities: RuntimeCapabilities | null = null;

export function setRuntimeCapabilities(
  capabilities?: RuntimeCapabilities | null
) {
  currentCapabilities = capabilities ?? null;
}

export function getRuntimeCapabilities(): RuntimeCapabilities | null {
  return currentCapabilities;
}

export function capabilitiesAreReadOnly(
  capabilities?: RuntimeCapabilities | null
): boolean {
  if (!capabilities) {
    return false;
  }

  return (
    !capabilities.runtime.evm_writes_enabled &&
    !capabilities.runtime.solana_writes_enabled &&
    !capabilities.runtime.external_trading_enabled &&
    !capabilities.runtime.external_agents_enabled
  );
}

export function isReadOnlyMode(
  capabilities?: RuntimeCapabilities | null
): boolean {
  return readOnlyPreviewEnabled || capabilitiesAreReadOnly(capabilities);
}

export function currentRuntimeIsReadOnly(): boolean {
  return isReadOnlyMode(currentCapabilities);
}

export function assertWritesEnabled(
  action: string,
  capabilities?: RuntimeCapabilities | null
) {
  if (!isReadOnlyMode(capabilities ?? currentCapabilities)) {
    return;
  }

  throw new Error(`${action} is disabled in read-only mode`);
}

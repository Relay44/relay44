export enum ValidationTier {
  Unverified = 0,
  Bronze = 1,
  Silver = 2,
  Gold = 3,
  Platinum = 4,
}

export const TIER_TO_RESPONSE: Record<ValidationTier, number> = {
  [ValidationTier.Unverified]: 20,
  [ValidationTier.Bronze]: 40,
  [ValidationTier.Silver]: 60,
  [ValidationTier.Gold]: 80,
  [ValidationTier.Platinum]: 95,
};

export const responseToTier = (response: number): ValidationTier => {
  if (response >= 90) return ValidationTier.Platinum;
  if (response >= 75) return ValidationTier.Gold;
  if (response >= 50) return ValidationTier.Silver;
  if (response >= 25) return ValidationTier.Bronze;
  return ValidationTier.Unverified;
};

export interface GlobalAgentId {
  namespace: 'eip155';
  chainId: number;
  registry: string;
  agentId: bigint;
  raw: string;
}

export interface MetadataEntry {
  key: string;
  value: Uint8Array;
}

export interface ValidationStatus {
  validatorAddress: string;
  agentId: bigint;
  response: number;
  responseHash: string;
  tag: string;
  lastUpdate: number;
}

export interface ValidationSummary {
  count: number;
  averageResponse: number;
}

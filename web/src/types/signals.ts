/** Signal marketplace types matching the backend API responses. */

export interface SignalProvider {
  id: string;
  owner: string;
  name: string;
  description: string | null;
  category: string;
  updateFrequencySecs: number;
  active: boolean;
  avgBrierScore: number | null;
  scoredSignals: number | null;
  createdAt: string;
}

export interface SignalProviderStats {
  providerId: string;
  totalSignals: number;
  scoredSignals: number | null;
  avgBrierScore: number | null;
  updatedAt: string;
}

export interface SignalEmission {
  providerName: string;
  providerId: string;
  outcome: string;
  signalValue: number;
  confidence: number;
  metadata: Record<string, unknown>;
  createdAt: string;
}

export interface SignalProviderFilters {
  category?: string;
  minBrier?: number;
  limit?: number;
}

export interface CreateSignalProviderRequest {
  name: string;
  description?: string;
  sourceUrl?: string;
  category?: string;
  updateFrequencySecs?: number;
}

import type { ScoringWeights } from '../types/scoring.js';

export const DEFAULT_WEIGHTS: ScoringWeights = {
  sourceCredibility: 0.25,
  claimConsistency: 0.20,
  narrativeVelocity: 0.15,
  coordinationSignal: 0.15,
  oddsDivergence: 0.15,
  temporalAnomaly: 0.10,
};

export function getRiskLevel(score: number): 'low' | 'medium' | 'high' | 'critical' {
  if (score >= 80) return 'critical';
  if (score >= 60) return 'high';
  if (score >= 35) return 'medium';
  return 'low';
}

export interface ScoreComponent {
  name: string;
  value: number;
  weight: number;
  weighted: number;
  details?: string;
}

export interface MisinformationScore {
  overall: number;
  components: ScoreComponent[];
  confidence: number;
  riskLevel: 'low' | 'medium' | 'high' | 'critical';
  summary: string;
}

export interface ScoringWeights {
  sourceCredibility: number;
  claimConsistency: number;
  narrativeVelocity: number;
  coordinationSignal: number;
  oddsDivergence: number;
  temporalAnomaly: number;
}

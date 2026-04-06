export type GraphNodeType = 'market' | 'claim' | 'source' | 'snapshot';
export type RelationshipType = 'supports' | 'contradicts' | 'amplifies' | 'originates';
export type RiskLevel = 'low' | 'medium' | 'high' | 'critical';
export type VerificationStatus = 'unverified' | 'supported' | 'disputed' | 'debunked';

export interface GraphNode {
  id: string;
  type: GraphNodeType;
  label: string;
  data: Record<string, unknown>;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  type: RelationshipType;
  weight: number;
}

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
  riskLevel: RiskLevel;
  summary: string;
}

export interface ContextGraphResult {
  cached: boolean;
  conditionId: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  score: MisinformationScore;
  metadata: {
    analyzedAt: string;
    snapshotUAL?: string;
    claimCount: number;
    sourceCount: number;
    edgeCount: number;
  };
}

export interface NarrativeSnapshot {
  id: string;
  condition_id: string;
  snapshot_at: number;
  misinfo_score: number;
  claim_count: number;
  source_count: number;
  dominant_narrative: string;
}

export const NODE_COLORS: Record<GraphNodeType, string> = {
  market: '#3b82f6',
  claim: '#10b981',
  source: '#6b7280',
  snapshot: '#8b5cf6',
};

export const EDGE_COLORS: Record<RelationshipType, string> = {
  supports: '#10b981',
  contradicts: '#ef4444',
  amplifies: '#f59e0b',
  originates: '#6b7280',
};

export const RISK_COLORS: Record<RiskLevel, string> = {
  low: '#10b981',
  medium: '#f59e0b',
  high: '#f97316',
  critical: '#ef4444',
};

export const VERIFICATION_COLORS: Record<VerificationStatus, string> = {
  unverified: '#6b7280',
  supported: '#10b981',
  disputed: '#f59e0b',
  debunked: '#ef4444',
};

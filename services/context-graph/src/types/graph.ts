import type { UAL } from './index.js';

export type VerificationStatus = 'unverified' | 'supported' | 'disputed' | 'debunked';
export type RelationshipType = 'supports' | 'contradicts' | 'amplifies' | 'originates';
export type SourcePlatform = 'x' | 'news' | 'polymarket' | 'reddit' | 'telegram' | 'other';

export interface Claim {
  id: string;
  text: string;
  claimHash: string;
  sourceId: string;
  confidence: number;
  sentiment: number;
  verificationStatus: VerificationStatus;
  evidenceUALs: string[];
  extractedAt: string;
}

export interface Source {
  id: string;
  url: string;
  platform: SourcePlatform;
  author: string;
  publishedAt: string;
  title?: string;
  snippet?: string;
  engagementMetrics: {
    likes?: number;
    shares?: number;
    replies?: number;
    views?: number;
  };
  credibilityScore: number;
  biasIndicators: string[];
}

export interface NarrativeEdge {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  relationshipType: RelationshipType;
  weight: number;
  firstObservedAt: string;
  spreadVelocity: number;
}

export interface ContextGraphSnapshot {
  id: string;
  conditionId: string;
  marketQuestion: string;
  analysisTimestamp: string;
  misinfoScore: number;
  claims: Claim[];
  sources: Source[];
  edges: NarrativeEdge[];
  narrativePattern: string;
  anomalyFlags: string[];
  snapshotUAL?: string;
}

export type GraphNodeType = 'market' | 'claim' | 'source' | 'snapshot';

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

export interface ContextGraphResult {
  nodes: GraphNode[];
  edges: GraphEdge[];
  score: import('./scoring.js').MisinformationScore;
  metadata: {
    analyzedAt: string;
    snapshotUAL?: string;
    claimCount: number;
    sourceCount: number;
    edgeCount: number;
  };
}

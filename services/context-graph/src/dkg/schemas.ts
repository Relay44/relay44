import { z } from 'zod';
import type { MarketContext } from '../types/polymarket.js';
import type { Claim, Source, NarrativeEdge, ContextGraphSnapshot } from '../types/graph.js';

const SCHEMA_ORG = 'https://schema.org/';

const RELAY44_CONTEXT = {
  '@version': 1.1,
  '@vocab': 'https://schema.org/',
  relay44: 'https://relay44.com/ontology/',
  misinfo: 'https://relay44.com/ontology/misinfo/',
  narrativeId: 'relay44:narrativeId',
  misinfoScore: 'misinfo:confidenceScore',
  sourceCredibility: 'misinfo:sourceCredibility',
  claimHash: 'relay44:claimHash',
  spreadVelocity: 'misinfo:spreadVelocity',
  coordinationScore: 'misinfo:coordinationScore',
  verificationStatus: 'misinfo:verificationStatus',
  conditionId: 'relay44:conditionId',
  platform: 'relay44:platform',
  sentiment: 'relay44:sentiment',
  biasIndicators: 'relay44:biasIndicators',
  anomalyFlags: 'misinfo:anomalyFlags',
  narrativePattern: 'misinfo:narrativePattern',
} as const;

export const SCHEMA_VERSION = '1.0.0';

// --- Zod validation schemas ---

export const MarketContextSchema = z.object({
  conditionId: z.string().min(1).max(128),
  question: z.string().min(1).max(500),
  description: z.string().max(2000).default(''),
  outcomes: z.array(z.string()).min(1),
  currentOdds: z.record(z.string(), z.number().min(0).max(1)),
  volume24h: z.number().min(0),
  totalLiquidity: z.number().min(0),
  category: z.string().max(64).default(''),
  createdAt: z.string(),
  endDate: z.string(),
  slug: z.string().max(256).default(''),
  active: z.boolean(),
});

export const ClaimSchema = z.object({
  id: z.string().min(1).max(128),
  text: z.string().min(1).max(1000),
  claimHash: z.string().min(8).max(128),
  sourceId: z.string().min(1).max(128),
  confidence: z.number().min(0).max(100),
  sentiment: z.number().min(-1).max(1),
  verificationStatus: z.enum(['unverified', 'supported', 'disputed', 'debunked']),
  evidenceUALs: z.array(z.string()).default([]),
  extractedAt: z.string(),
});

export const SourceSchema = z.object({
  id: z.string().min(1).max(128),
  url: z.string().max(2000),
  platform: z.enum(['x', 'news', 'polymarket', 'reddit', 'telegram', 'other']),
  author: z.string().max(256),
  publishedAt: z.string(),
  title: z.string().max(500).optional(),
  snippet: z.string().max(1000).optional(),
  credibilityScore: z.number().min(0).max(100),
  biasIndicators: z.array(z.string()).default([]),
});

export const NarrativeEdgeSchema = z.object({
  id: z.string().min(1).max(128),
  sourceNodeId: z.string().min(1).max(128),
  targetNodeId: z.string().min(1).max(128),
  relationshipType: z.enum(['supports', 'contradicts', 'amplifies', 'originates']),
  weight: z.number().min(0).max(1),
  firstObservedAt: z.string(),
  spreadVelocity: z.number().min(0),
});

export const ContextGraphSnapshotSchema = z.object({
  id: z.string().min(1).max(128),
  conditionId: z.string().min(1).max(128),
  marketQuestion: z.string().min(1).max(500),
  analysisTimestamp: z.string(),
  misinfoScore: z.number().min(0).max(100),
  narrativePattern: z.string().max(500).default(''),
  anomalyFlags: z.array(z.string()).default([]),
});

// --- JSON-LD asset builders ---

export function buildMarketContextAsset(market: MarketContext): object {
  return {
    '@context': [SCHEMA_ORG, RELAY44_CONTEXT],
    '@type': 'FinancialProduct',
    '@id': `urn:relay44:market:${market.conditionId}`,
    name: market.question,
    version: SCHEMA_VERSION,
    description: market.description,
    category: market.category,
    dateCreated: market.createdAt,
    expires: market.endDate,
    additionalProperty: [
      { '@type': 'PropertyValue', name: 'schemaVersion', value: SCHEMA_VERSION },
      { '@type': 'PropertyValue', name: 'conditionId', value: market.conditionId },
      { '@type': 'PropertyValue', name: 'outcomes', value: market.outcomes.join(',') },
      { '@type': 'PropertyValue', name: 'currentOdds', value: JSON.stringify(market.currentOdds) },
      { '@type': 'PropertyValue', name: 'volume24h', value: market.volume24h },
      { '@type': 'PropertyValue', name: 'totalLiquidity', value: market.totalLiquidity },
      { '@type': 'PropertyValue', name: 'slug', value: market.slug },
      { '@type': 'PropertyValue', name: 'active', value: market.active },
      {
        '@type': 'PropertyValue',
        name: 'movementHistory',
        value: JSON.stringify(market.movementHistory.slice(-24)),
      },
    ],
  };
}

export function buildClaimAsset(claim: Claim): object {
  return {
    '@context': [SCHEMA_ORG, RELAY44_CONTEXT],
    '@type': 'Claim',
    '@id': `urn:relay44:claim:${claim.claimHash}`,
    name: claim.text.slice(0, 100),
    version: SCHEMA_VERSION,
    text: claim.text,
    dateCreated: claim.extractedAt,
    additionalProperty: [
      { '@type': 'PropertyValue', name: 'schemaVersion', value: SCHEMA_VERSION },
      { '@type': 'PropertyValue', name: 'claimHash', value: claim.claimHash },
      { '@type': 'PropertyValue', name: 'sourceId', value: claim.sourceId },
      { '@type': 'PropertyValue', name: 'confidence', value: claim.confidence },
      { '@type': 'PropertyValue', name: 'sentiment', value: claim.sentiment },
      { '@type': 'PropertyValue', name: 'verificationStatus', value: claim.verificationStatus },
    ],
    ...(claim.evidenceUALs.length && {
      instrument: claim.evidenceUALs.map((ual) => ({ '@id': ual })),
    }),
  };
}

export function buildSourceAsset(source: Source): object {
  const sourceType =
    source.platform === 'news' ? 'NewsArticle' : 'SocialMediaPosting';

  return {
    '@context': [SCHEMA_ORG, RELAY44_CONTEXT],
    '@type': sourceType,
    '@id': `urn:relay44:source:${source.id}`,
    name: source.title || source.url,
    version: SCHEMA_VERSION,
    url: source.url,
    datePublished: source.publishedAt,
    author: { '@type': 'Person', name: source.author },
    ...(source.snippet && { description: source.snippet }),
    additionalProperty: [
      { '@type': 'PropertyValue', name: 'schemaVersion', value: SCHEMA_VERSION },
      { '@type': 'PropertyValue', name: 'platform', value: source.platform },
      { '@type': 'PropertyValue', name: 'sourceCredibility', value: source.credibilityScore },
      {
        '@type': 'PropertyValue',
        name: 'engagementMetrics',
        value: JSON.stringify(source.engagementMetrics),
      },
      ...(source.biasIndicators.length
        ? [{ '@type': 'PropertyValue', name: 'biasIndicators', value: source.biasIndicators.join(',') }]
        : []),
    ],
  };
}

export function buildNarrativeEdgeAsset(edge: NarrativeEdge): object {
  return {
    '@context': [SCHEMA_ORG, RELAY44_CONTEXT],
    '@type': 'Action',
    '@id': `urn:relay44:edge:${edge.id}`,
    name: 'NarrativeSpread',
    version: SCHEMA_VERSION,
    agent: { '@id': `urn:relay44:node:${edge.sourceNodeId}` },
    object: { '@id': `urn:relay44:node:${edge.targetNodeId}` },
    startTime: edge.firstObservedAt,
    additionalProperty: [
      { '@type': 'PropertyValue', name: 'schemaVersion', value: SCHEMA_VERSION },
      { '@type': 'PropertyValue', name: 'relationshipType', value: edge.relationshipType },
      { '@type': 'PropertyValue', name: 'weight', value: edge.weight },
      { '@type': 'PropertyValue', name: 'spreadVelocity', value: edge.spreadVelocity },
    ],
  };
}

export function buildContextGraphSnapshotAsset(
  snapshot: ContextGraphSnapshot,
  claimUALs: string[],
  sourceUALs: string[],
  edgeUALs: string[],
): object {
  return {
    '@context': [SCHEMA_ORG, RELAY44_CONTEXT],
    '@type': 'Dataset',
    '@id': `urn:relay44:snapshot:${snapshot.id}`,
    name: `Context Graph: ${snapshot.marketQuestion.slice(0, 80)}`,
    version: SCHEMA_VERSION,
    description: `Misinformation analysis for Polymarket market ${snapshot.conditionId}`,
    dateCreated: snapshot.analysisTimestamp,
    additionalProperty: [
      { '@type': 'PropertyValue', name: 'schemaVersion', value: SCHEMA_VERSION },
      { '@type': 'PropertyValue', name: 'conditionId', value: snapshot.conditionId },
      { '@type': 'PropertyValue', name: 'misinfoScore', value: snapshot.misinfoScore },
      { '@type': 'PropertyValue', name: 'narrativePattern', value: snapshot.narrativePattern },
      { '@type': 'PropertyValue', name: 'anomalyFlags', value: snapshot.anomalyFlags.join(',') },
      { '@type': 'PropertyValue', name: 'claimCount', value: claimUALs.length },
      { '@type': 'PropertyValue', name: 'sourceCount', value: sourceUALs.length },
      { '@type': 'PropertyValue', name: 'edgeCount', value: edgeUALs.length },
    ],
    hasPart: [
      ...claimUALs.map((ual) => ({ '@id': ual })),
      ...sourceUALs.map((ual) => ({ '@id': ual })),
      ...edgeUALs.map((ual) => ({ '@id': ual })),
    ],
  };
}

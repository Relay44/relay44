import type { NarrativeEdge, Source } from '../types/graph.js';
import type { SocialSignals } from '../pipeline/social-enricher.js';

export interface AnomalyResult {
  coordinationScore: number;
  temporalAnomalyScore: number;
  narrativeVelocityScore: number;
  flags: string[];
}

export function detectAnomalies(
  edges: NarrativeEdge[],
  social: SocialSignals,
  sources: Source[],
): AnomalyResult {
  const flags: string[] = [];

  // --- Coordination detection ---
  let coordinationScore = 0;

  const { duplicateTextRatio, burstiness, lowFollowerRatio } =
    social.coordinationIndicators;

  // High duplicate text ratio suggests coordinated posting
  if (duplicateTextRatio > 0.3) {
    coordinationScore += 40;
    flags.push(`High duplicate text ratio: ${(duplicateTextRatio * 100).toFixed(0)}%`);
  } else if (duplicateTextRatio > 0.1) {
    coordinationScore += 20;
  }

  // High burstiness suggests coordinated timing
  if (burstiness > 5) {
    coordinationScore += 30;
    flags.push(`Burst posting detected: ${burstiness.toFixed(1)}x average`);
  } else if (burstiness > 3) {
    coordinationScore += 15;
  }

  // High low-follower ratio suggests bot activity
  if (lowFollowerRatio > 0.5) {
    coordinationScore += 30;
    flags.push(`${(lowFollowerRatio * 100).toFixed(0)}% of posts from low-follower accounts`);
  } else if (lowFollowerRatio > 0.3) {
    coordinationScore += 15;
  }

  coordinationScore = Math.min(100, coordinationScore);

  // --- Temporal anomaly detection ---
  let temporalAnomalyScore = 0;

  // Check for sudden narrative shifts
  const amplificationEdges = edges.filter((e) => e.relationshipType === 'amplifies');
  const highVelocity = amplificationEdges.filter((e) => e.spreadVelocity > 5);

  if (highVelocity.length > 3) {
    temporalAnomalyScore += 40;
    flags.push(`${highVelocity.length} high-velocity narrative spreads detected`);
  }

  // Check if engagement spikes without credible source
  const credibleSources = sources.filter((s) => s.credibilityScore >= 70);
  if (social.tweetCount > 50 && credibleSources.length === 0) {
    temporalAnomalyScore += 30;
    flags.push('High social activity with no credible sources');
  }

  // One-sided narrative
  const { positive, negative, neutral } = social.sentimentDistribution;
  const total = positive + negative + neutral;
  if (total > 10) {
    const dominance = Math.max(positive, negative) / total;
    if (dominance > 0.8) {
      temporalAnomalyScore += 25;
      flags.push(`One-sided narrative: ${(dominance * 100).toFixed(0)}% ${positive > negative ? 'positive' : 'negative'}`);
    }
  }

  temporalAnomalyScore = Math.min(100, temporalAnomalyScore);

  // --- Narrative velocity ---
  let narrativeVelocityScore = 0;

  const avgVelocity = amplificationEdges.length > 0
    ? amplificationEdges.reduce((sum, e) => sum + e.spreadVelocity, 0) / amplificationEdges.length
    : 0;

  if (avgVelocity > 8) {
    narrativeVelocityScore = 90;
    flags.push(`Extreme spread velocity: ${avgVelocity.toFixed(1)}/hr`);
  } else if (avgVelocity > 4) {
    narrativeVelocityScore = 60;
  } else if (avgVelocity > 2) {
    narrativeVelocityScore = 30;
  }

  // Amplification concentration — few sources amplifying many
  const amplifiers = new Map<string, number>();
  for (const e of amplificationEdges) {
    amplifiers.set(e.sourceNodeId, (amplifiers.get(e.sourceNodeId) || 0) + 1);
  }
  const topAmplifier = Math.max(...amplifiers.values(), 0);
  if (topAmplifier > 5) {
    narrativeVelocityScore = Math.min(100, narrativeVelocityScore + 20);
    flags.push(`Single source amplifying ${topAmplifier} others`);
  }

  return {
    coordinationScore,
    temporalAnomalyScore,
    narrativeVelocityScore,
    flags,
  };
}

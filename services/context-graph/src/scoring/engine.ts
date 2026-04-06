import type { Claim, NarrativeEdge } from '../types/graph.js';
import type { MisinformationScore, ScoreComponent } from '../types/scoring.js';
import type { MarketContext } from '../types/polymarket.js';
import type { SocialSignals } from '../pipeline/social-enricher.js';
import type { NewsSignals } from '../pipeline/news-enricher.js';
import { DEFAULT_WEIGHTS, getRiskLevel } from './weights.js';
import { scoreSourceCredibility } from './source-credibility.js';
import { detectAnomalies } from './anomaly-detector.js';

export class ScoringEngine {
  score(
    claims: Claim[],
    edges: NarrativeEdge[],
    social: SocialSignals,
    news: NewsSignals,
    market: MarketContext,
  ): MisinformationScore {
    const allSources = [...social.sources, ...news.sources];
    const anomalies = detectAnomalies(edges, social, allSources);

    const components: ScoreComponent[] = [];

    // 1. Source credibility (inversed: low credibility → high misinfo score)
    const credResult = scoreSourceCredibility(allSources);
    components.push({
      name: 'sourceCredibility',
      value: credResult.score,
      weight: DEFAULT_WEIGHTS.sourceCredibility,
      weighted: credResult.score * DEFAULT_WEIGHTS.sourceCredibility,
      details: credResult.details,
    });

    // 2. Claim consistency
    const consistency = this.scoreClaimConsistency(claims, edges);
    components.push({
      name: 'claimConsistency',
      value: consistency.score,
      weight: DEFAULT_WEIGHTS.claimConsistency,
      weighted: consistency.score * DEFAULT_WEIGHTS.claimConsistency,
      details: consistency.details,
    });

    // 3. Narrative velocity
    components.push({
      name: 'narrativeVelocity',
      value: anomalies.narrativeVelocityScore,
      weight: DEFAULT_WEIGHTS.narrativeVelocity,
      weighted: anomalies.narrativeVelocityScore * DEFAULT_WEIGHTS.narrativeVelocity,
      details: `Avg spread velocity across ${edges.filter((e) => e.relationshipType === 'amplifies').length} amplification edges`,
    });

    // 4. Coordination signal
    components.push({
      name: 'coordinationSignal',
      value: anomalies.coordinationScore,
      weight: DEFAULT_WEIGHTS.coordinationSignal,
      weighted: anomalies.coordinationScore * DEFAULT_WEIGHTS.coordinationSignal,
      details: `Dup ratio: ${(social.coordinationIndicators.duplicateTextRatio * 100).toFixed(0)}%, Burstiness: ${social.coordinationIndicators.burstiness.toFixed(1)}x`,
    });

    // 5. Odds-reality divergence
    const divergence = this.scoreOddsDivergence(claims, market);
    components.push({
      name: 'oddsDivergence',
      value: divergence.score,
      weight: DEFAULT_WEIGHTS.oddsDivergence,
      weighted: divergence.score * DEFAULT_WEIGHTS.oddsDivergence,
      details: divergence.details,
    });

    // 6. Temporal anomaly
    components.push({
      name: 'temporalAnomaly',
      value: anomalies.temporalAnomalyScore,
      weight: DEFAULT_WEIGHTS.temporalAnomaly,
      weighted: anomalies.temporalAnomalyScore * DEFAULT_WEIGHTS.temporalAnomaly,
      details: anomalies.flags.join('; ') || 'No temporal anomalies detected',
    });

    // Calculate overall score
    const overall = Math.round(
      components.reduce((sum, c) => sum + c.weighted, 0),
    );
    const clamped = Math.max(0, Math.min(100, overall));

    // Confidence based on data availability
    const confidence = this.calculateConfidence(claims, allSources, social);

    const riskLevel = getRiskLevel(clamped);
    const summary = this.generateSummary(clamped, riskLevel, components, anomalies.flags);

    return {
      overall: clamped,
      components,
      confidence,
      riskLevel,
      summary,
    };
  }

  private scoreClaimConsistency(
    claims: Claim[],
    edges: NarrativeEdge[],
  ): { score: number; details: string } {
    if (claims.length < 2) {
      return { score: 30, details: 'Insufficient claims for consistency analysis' };
    }

    const supportEdges = edges.filter((e) => e.relationshipType === 'supports');
    const contradictEdges = edges.filter((e) => e.relationshipType === 'contradicts');

    const totalRelations = supportEdges.length + contradictEdges.length;
    if (totalRelations === 0) {
      return { score: 40, details: 'No claim relationships detected' };
    }

    const contradictionRatio = contradictEdges.length / totalRelations;

    // More contradictions → higher misinfo score
    let score = contradictionRatio * 100;

    // Debunked claims are strong signals
    const debunkedCount = claims.filter((c) => c.verificationStatus === 'debunked').length;
    if (debunkedCount > 0) {
      score = Math.min(100, score + debunkedCount * 20);
    }

    // Disputed claims add moderate signal
    const disputedCount = claims.filter((c) => c.verificationStatus === 'disputed').length;
    if (disputedCount > 0) {
      score = Math.min(100, score + disputedCount * 10);
    }

    const details = `${supportEdges.length} supporting, ${contradictEdges.length} contradicting, ${debunkedCount} debunked`;
    return { score: Math.round(Math.min(100, score)), details };
  }

  private scoreOddsDivergence(
    claims: Claim[],
    market: MarketContext,
  ): { score: number; details: string } {
    const yesOdds = market.currentOdds['Yes'] || 0;

    // Check if evidence contradicts market odds
    const supportedClaims = claims.filter((c) => c.verificationStatus === 'supported');
    const debunkedClaims = claims.filter((c) => c.verificationStatus === 'debunked');

    // Average sentiment of verified claims
    const verifiedSentiment = supportedClaims.length > 0
      ? supportedClaims.reduce((sum, c) => sum + c.sentiment, 0) / supportedClaims.length
      : 0;

    // If market is very bullish (>80%) but evidence is negative, flag divergence
    // If market is very bearish (<20%) but evidence is positive, also flag
    let divergenceScore = 0;

    if (yesOdds > 0.8 && verifiedSentiment < -0.3) {
      divergenceScore = 70;
    } else if (yesOdds < 0.2 && verifiedSentiment > 0.3) {
      divergenceScore = 70;
    } else if (debunkedClaims.length > 0 && yesOdds > 0.5) {
      divergenceScore = 50;
    }

    // Check for sudden movement without evidence
    const history = market.movementHistory;
    if (history.length >= 2) {
      const recent = history[history.length - 1];
      const prior = history[history.length - 2];
      const recentYes = recent.outcomes['Yes'] || 0;
      const priorYes = prior.outcomes['Yes'] || 0;
      const swing = Math.abs(recentYes - priorYes);

      if (swing > 0.15 && supportedClaims.length === 0) {
        divergenceScore = Math.min(100, divergenceScore + 30);
      }
    }

    const details = `Market ${(yesOdds * 100).toFixed(0)}% Yes, evidence sentiment: ${verifiedSentiment.toFixed(2)}, ${debunkedClaims.length} debunked claims`;
    return { score: Math.min(100, divergenceScore), details };
  }

  private calculateConfidence(
    claims: Claim[],
    sources: { credibilityScore: number }[],
    social: SocialSignals,
  ): number {
    let confidence = 0;

    // More data → higher confidence
    if (claims.length >= 10) confidence += 25;
    else if (claims.length >= 5) confidence += 15;
    else if (claims.length >= 2) confidence += 10;

    if (sources.length >= 10) confidence += 25;
    else if (sources.length >= 5) confidence += 15;
    else if (sources.length >= 1) confidence += 10;

    if (social.tweetCount >= 50) confidence += 20;
    else if (social.tweetCount >= 10) confidence += 10;

    // Verified claims boost confidence
    const verifiedCount = claims.filter(
      (c) => c.verificationStatus !== 'unverified',
    ).length;
    if (verifiedCount > 0) confidence += Math.min(30, verifiedCount * 10);

    return Math.min(100, confidence);
  }

  private generateSummary(
    score: number,
    riskLevel: string,
    components: ScoreComponent[],
    flags: string[],
  ): string {
    const topComponents = [...components]
      .sort((a, b) => b.weighted - a.weighted)
      .slice(0, 2)
      .map((c) => c.name.replace(/([A-Z])/g, ' $1').toLowerCase().trim());

    let summary = `Misinformation risk: ${riskLevel} (${score}/100). `;
    summary += `Top signals: ${topComponents.join(', ')}. `;

    if (flags.length > 0) {
      summary += `Anomalies: ${flags.slice(0, 3).join('; ')}.`;
    } else {
      summary += 'No significant anomalies detected.';
    }

    return summary;
  }
}

import type { Source } from '../types/graph.js';

export function scoreSourceCredibility(sources: Source[]): {
  score: number;
  details: string;
} {
  if (sources.length === 0) {
    return { score: 50, details: 'No sources to evaluate' };
  }

  const avgCredibility =
    sources.reduce((sum, s) => sum + s.credibilityScore, 0) / sources.length;

  // Credibility diversity — are sources all from one platform?
  const platforms = new Set(sources.map((s) => s.platform));
  const diversityBonus = Math.min(platforms.size * 5, 20);

  // High-credibility source count
  const reliableCount = sources.filter((s) => s.credibilityScore >= 80).length;
  const reliableRatio = reliableCount / sources.length;

  // Low-credibility dominance penalty
  const lowCredCount = sources.filter((s) => s.credibilityScore < 30).length;
  const lowCredRatio = lowCredCount / sources.length;

  // Inverse: high credibility = low misinfo score
  let misinfoScore = 100 - avgCredibility;
  misinfoScore -= diversityBonus;
  misinfoScore += lowCredRatio * 30;
  misinfoScore -= reliableRatio * 20;

  const clamped = Math.max(0, Math.min(100, misinfoScore));
  const details = `Avg credibility: ${Math.round(avgCredibility)}, ${reliableCount}/${sources.length} reliable, ${platforms.size} platforms`;

  return { score: clamped, details };
}

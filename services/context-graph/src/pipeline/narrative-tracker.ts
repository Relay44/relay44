import type { Claim, Source, NarrativeEdge, RelationshipType } from '../types/graph.js';
import type { SocialSignals } from './social-enricher.js';
import type { NewsSignals } from './news-enricher.js';

export class NarrativeTracker {
  buildEdges(
    claims: Claim[],
    social: SocialSignals,
    news: NewsSignals,
  ): NarrativeEdge[] {
    const edges: NarrativeEdge[] = [];
    const allSources = [...social.sources, ...news.sources];

    // 1. Source → Claim edges (originates)
    for (const claim of claims) {
      const source = allSources.find((s) => s.id === claim.sourceId);
      if (source) {
        edges.push({
          id: `edge:${source.id}→${claim.id}`,
          sourceNodeId: source.id,
          targetNodeId: claim.id,
          relationshipType: 'originates',
          weight: claim.confidence / 100,
          firstObservedAt: source.publishedAt,
          spreadVelocity: 0,
        });
      }
    }

    // 2. Claim ↔ Claim edges (supports/contradicts)
    for (let i = 0; i < claims.length; i++) {
      for (let j = i + 1; j < claims.length; j++) {
        const relation = this.compareClaims(claims[i], claims[j]);
        if (relation) {
          edges.push({
            id: `edge:${claims[i].id}↔${claims[j].id}`,
            sourceNodeId: claims[i].id,
            targetNodeId: claims[j].id,
            relationshipType: relation.type,
            weight: relation.weight,
            firstObservedAt: claims[i].extractedAt < claims[j].extractedAt
              ? claims[i].extractedAt
              : claims[j].extractedAt,
            spreadVelocity: 0,
          });
        }
      }
    }

    // 3. Source → Source edges (amplifies) — detect re-sharing patterns
    const sourcesByTime = [...allSources].sort(
      (a, b) => new Date(a.publishedAt).getTime() - new Date(b.publishedAt).getTime(),
    );

    for (let i = 0; i < sourcesByTime.length; i++) {
      for (let j = i + 1; j < Math.min(i + 10, sourcesByTime.length); j++) {
        const similarity = this.textSimilarity(
          sourcesByTime[i].snippet || '',
          sourcesByTime[j].snippet || '',
        );

        if (similarity > 0.6) {
          const timeDiffMs =
            new Date(sourcesByTime[j].publishedAt).getTime() -
            new Date(sourcesByTime[i].publishedAt).getTime();
          const hoursGap = timeDiffMs / 3600000;

          edges.push({
            id: `edge:${sourcesByTime[i].id}→${sourcesByTime[j].id}`,
            sourceNodeId: sourcesByTime[i].id,
            targetNodeId: sourcesByTime[j].id,
            relationshipType: 'amplifies',
            weight: similarity,
            firstObservedAt: sourcesByTime[i].publishedAt,
            spreadVelocity: hoursGap > 0 ? 1 / hoursGap : 10,
          });
        }
      }
    }

    // 4. Compute spread velocities for origination edges
    this.computeSpreadVelocity(edges, allSources);

    return edges;
  }

  private compareClaims(
    a: Claim,
    b: Claim,
  ): { type: RelationshipType; weight: number } | null {
    const similarity = this.textSimilarity(a.text, b.text);

    // Very similar text — supporting each other
    if (similarity > 0.7) {
      return { type: 'supports', weight: similarity };
    }

    // Check for contradiction signals
    if (similarity > 0.3) {
      const aText = a.text.toLowerCase();
      const bText = b.text.toLowerCase();

      const contradictionPairs = [
        ['true', 'false'], ['confirmed', 'denied'], ['will', 'won\'t'],
        ['yes', 'no'], ['correct', 'incorrect'], ['real', 'fake'],
        ['likely', 'unlikely'], ['approved', 'rejected'],
      ];

      for (const [pos, neg] of contradictionPairs) {
        if (
          (aText.includes(pos) && bText.includes(neg)) ||
          (aText.includes(neg) && bText.includes(pos))
        ) {
          return { type: 'contradicts', weight: 0.8 };
        }
      }

      // Same topic, moderate similarity
      return { type: 'supports', weight: similarity * 0.5 };
    }

    return null;
  }

  private textSimilarity(a: string, b: string): number {
    if (!a || !b) return 0;

    const wordsA = new Set(a.toLowerCase().split(/\s+/).filter((w) => w.length > 3));
    const wordsB = new Set(b.toLowerCase().split(/\s+/).filter((w) => w.length > 3));

    if (wordsA.size === 0 || wordsB.size === 0) return 0;

    let intersection = 0;
    for (const word of wordsA) {
      if (wordsB.has(word)) intersection++;
    }

    // Jaccard similarity
    const union = wordsA.size + wordsB.size - intersection;
    return union > 0 ? intersection / union : 0;
  }

  private computeSpreadVelocity(edges: NarrativeEdge[], sources: Source[]): void {
    const sourceMap = new Map(sources.map((s) => [s.id, s]));

    for (const edge of edges) {
      if (edge.relationshipType !== 'originates') continue;

      const source = sourceMap.get(edge.sourceNodeId);
      if (!source) continue;

      // Count how many amplification edges lead from this source
      const amplifications = edges.filter(
        (e) =>
          e.relationshipType === 'amplifies' &&
          e.sourceNodeId === source.id,
      );

      if (amplifications.length > 0) {
        // Average spread velocity based on time gaps
        const avgVelocity =
          amplifications.reduce((sum, e) => sum + e.spreadVelocity, 0) /
          amplifications.length;
        edge.spreadVelocity = avgVelocity;
      }
    }
  }
}

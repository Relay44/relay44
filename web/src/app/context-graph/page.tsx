'use client';

import { useState, useEffect } from 'react';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Badge } from '@/components/ui/Badge';
import { Spinner } from '@/components/ui/Spinner';
import {
  ContextGraphViewer,
  NarrativeTimeline,
  useContextGraphAnalysis,
  useNarrativeTimeline,
} from '@/components/context-graph';

export default function ContextGraphPage() {
  const [input, setInput] = useState('');
  const { result, loading, error, analyze } = useContextGraphAnalysis();
  const { data: timeline } = useNarrativeTimeline(result?.conditionId ?? null);

  const handleAnalyze = async () => {
    if (!input.trim()) return;

    const isUrl = input.includes('polymarket.com');
    const isConditionId = /^0x[a-fA-F0-9]+$/.test(input);

    await analyze({
      marketUrl: isUrl ? input : undefined,
      conditionId: isConditionId ? input : undefined,
      slug: !isUrl && !isConditionId ? input : undefined,
      depth: 'full',
    });
  };

  return (
    <div className="min-h-screen bg-bg-base text-text-primary p-4 md:p-8">
      <div className="max-w-7xl mx-auto">
        <div className="mb-8">
          <h1 className="text-2xl font-bold mb-1">Context Graph</h1>
          <p className="text-sm text-text-muted">
            Analyze Polymarket markets for misinformation using decentralized context graphs on OriginTrail DKG
          </p>
        </div>

        <div className="flex gap-2 mb-6">
          <div className="flex-1">
            <Input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAnalyze()}
              placeholder="Polymarket URL, condition ID, or market slug..."
            />
          </div>
          <Button
            onClick={handleAnalyze}
            loading={loading}
            disabled={!input.trim()}
          >
            Analyze
          </Button>
        </div>

        {error && (
          <Card className="mb-4 p-3 border-ask/20">
            <p className="text-sm text-ask">{error}</p>
          </Card>
        )}

        {loading && (
          <div className="flex items-center justify-center py-20">
            <div className="flex items-center gap-3 text-text-muted">
              <Spinner size="md" />
              <span className="text-sm">
                Building context graph... Fetching market data, social signals, and news.
              </span>
            </div>
          </div>
        )}

        {result && !loading && (
          <div className="space-y-6">
            {result.cached && (
              <Badge variant="secondary">
                Cached result. Use depth=full for fresh analysis.
              </Badge>
            )}

            <ContextGraphViewer result={result} />

            {timeline && timeline.length > 0 && (
              <NarrativeTimeline timeline={timeline} />
            )}

            {result.metadata.snapshotUAL && (
              <Card className="p-4">
                <h3 className="text-sm font-medium text-text-secondary mb-2">
                  DKG Provenance
                </h3>
                <p className="text-xs text-text-muted mb-1">
                  This analysis is published as a verifiable knowledge asset on OriginTrail DKG.
                </p>
                <code className="text-xs text-accent break-all">
                  {result.metadata.snapshotUAL}
                </code>
              </Card>
            )}
          </div>
        )}

        {!result && !loading && !error && (
          <div className="text-center py-20 text-text-muted">
            <p className="text-sm">
              Enter a Polymarket market URL or slug to build a context graph
            </p>
            <p className="text-xs mt-2 text-text-muted">
              The system will analyze social signals, news cross-references, and market data
              to detect potential misinformation patterns.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

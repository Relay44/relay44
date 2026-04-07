'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout';
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

  async function handleAnalyze(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed) return;

    const payload = trimmed.startsWith('http')
      ? { marketUrl: trimmed }
      : trimmed.startsWith('0x')
        ? { conditionId: trimmed }
        : { slug: trimmed };

    await analyze(payload);
  }

  return (
    <PageShell>
      <div className="container mx-auto max-w-6xl px-4 py-8 space-y-6">
        <div className="space-y-2">
          <h1 className="text-3xl font-bold">Context Graph</h1>
          <p className="text-text-secondary">
            Misinformation detection for prediction markets — verifiable knowledge graphs on OriginTrail DKG.
          </p>
        </div>

        <form onSubmit={handleAnalyze} className="flex gap-3">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Polymarket URL, condition ID, or market slug"
            className="flex-1 h-10 border border-border bg-bg-primary px-3 text-sm text-text-primary placeholder:text-text-muted focus:border-border-hover focus:outline-none"
          />
          <button
            type="submit"
            disabled={loading || !input.trim()}
            className="h-10 border border-border px-6 text-[0.7rem] uppercase tracking-[0.12em] text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {loading ? 'Analyzing...' : 'Analyze'}
          </button>
        </form>

        {error && (
          <div className="border border-red-500/30 bg-red-500/5 p-3 text-sm text-red-400">
            {error}
          </div>
        )}

        {result && (
          <>
            <ContextGraphViewer result={result} />
            {timeline && timeline.length > 0 && (
              <NarrativeTimeline timeline={timeline} />
            )}
          </>
        )}
      </div>
    </PageShell>
  );
}

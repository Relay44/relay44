import { Router, type Request, type Response, type NextFunction } from 'express';
import type { DKGClientInterface } from '../dkg/client.js';
import type { ServiceConfig } from '../types/index.js';
import type { MarketInput } from '../types/polymarket.js';
import { ContextGraphPipeline } from '../pipeline/orchestrator.js';
import { AnalysisStore } from '../db/queries.js';
import type { TickAnalyzer } from '../workers/tick-analyzer.js';
import type Database from 'better-sqlite3';

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise
      .then((val) => { clearTimeout(timer); resolve(val); })
      .catch((err) => { clearTimeout(timer); reject(err); });
  });
}

function requireAuth(secret: string | undefined) {
  return (req: Request, res: Response, next: NextFunction) => {
    if (!secret) { next(); return; }
    const auth = req.headers.authorization;
    if (auth === `Bearer ${secret}`) { next(); return; }
    res.status(401).json({ error: 'Unauthorized' });
  };
}

function sanitizeId(raw: unknown): string {
  const id = String(raw || '').trim().slice(0, 256);
  if (!id) return '';
  // Allow alphanumeric, hyphens, underscores, colons (for UALs), dots
  if (!/^[\w.:-]+$/.test(id)) return '';
  return id;
}

export function createRouter(
  dkg: DKGClientInterface,
  db: Database.Database,
  config: ServiceConfig,
  tickAnalyzer?: TickAnalyzer,
): Router {
  const router = Router();
  const pipeline = new ContextGraphPipeline(dkg, config);
  const store = new AnalysisStore(db);
  const auth = requireAuth(config.sharedSecret);

  let activeAnalyses = 0;

  // Health check
  router.get('/health', async (_req: Request, res: Response) => {
    const clientType = dkg.constructor.name;
    const isMock = clientType === 'MockDKGClient';
    const diag = {
      clientType,
      dkgEnabled: config.features.dkgEnabled,
      hasEndpoint: !!config.dkg.endpoint,
      endpoint: config.dkg.endpoint ? config.dkg.endpoint.slice(0, 30) : null,
    };

    if (isMock) {
      res.json({ status: 'ok', dkg: 'mock', version: '0.1.0', diag });
      return;
    }
    try {
      const reachable = await withTimeout(dkg.healthCheck(), 5_000, 'DKG health check');
      res.json({ status: 'ok', dkg: reachable ? 'connected' : 'live', version: '0.1.0', diag });
    } catch {
      res.json({ status: 'ok', dkg: 'live', version: '0.1.0', diag });
    }
  });

  // Submit market for analysis (auth-protected)
  router.post('/analyze', auth, async (req: Request, res: Response) => {
    try {
      const { marketUrl, conditionId, slug, depth } = req.body as MarketInput;

      if (!marketUrl && !conditionId && !slug) {
        res.status(400).json({ error: 'Provide marketUrl, conditionId, or slug' });
        return;
      }

      // Check cache — try conditionId first, then slug, then URL
      const cacheKeys = [conditionId, slug, marketUrl].filter(Boolean) as string[];
      if (depth !== 'full') {
        for (const key of cacheKeys) {
          const cached = store.getCached(key);
          if (cached) {
            res.json({ cached: true, ...cached });
            return;
          }
        }
      }

      // Concurrency limit
      if (activeAnalyses >= config.maxConcurrentAnalyses) {
        res.status(429).json({ error: 'Too many concurrent analyses. Try again shortly.' });
        return;
      }

      activeAnalyses++;
      try {
        const result = await withTimeout(
          pipeline.analyze({ marketUrl, conditionId, slug, depth }),
          config.analysisTimeoutMs,
          'Market analysis',
        );

        const resolvedConditionId = result.nodes.find((n) => n.type === 'market')?.data?.conditionId as string || '';
        const marketQuestion = result.nodes.find((n) => n.type === 'market')?.label || 'Unknown';
        store.save(resolvedConditionId, marketQuestion, result);

        // Also cache under the original input key (slug/URL) for lookup consistency
        const inputKey = slug || marketUrl || '';
        if (inputKey && inputKey !== resolvedConditionId) {
          store.save(inputKey, marketQuestion, result);
        }

        store.saveNarrativeSnapshot(
          resolvedConditionId,
          result.score.overall,
          result.metadata.claimCount,
          result.metadata.sourceCount,
          result.score.summary,
        );

        res.json({ cached: false, conditionId: resolvedConditionId, ...result });
      } finally {
        activeAnalyses--;
      }
    } catch (error) {
      const msg = error instanceof Error ? error.message : 'Analysis failed';
      console.error('[api] Analysis error:', error);
      res.status(500).json({ error: msg });
    }
  });

  // Get analysis result by condition ID
  router.get('/analysis/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const cached = store.getCached(id);
    if (!cached) {
      res.status(404).json({ error: 'Analysis not found or expired' });
      return;
    }
    res.json(cached);
  });

  // Get full graph for visualization
  router.get('/graph/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const cached = store.getCached(id);
    if (!cached?.graph) {
      res.status(404).json({ error: 'Graph not found or expired' });
      return;
    }
    res.json(cached.graph);
  });

  // Get score only
  router.get('/score/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const cached = store.getCached(id);
    if (!cached) {
      res.status(404).json({ error: 'Score not found or expired' });
      return;
    }
    res.json({
      conditionId: cached.conditionId,
      score: cached.score,
      snapshotUAL: cached.snapshotUAL,
    });
  });

  // Get extracted claims
  router.get('/claims/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const cached = store.getCached(id);
    if (!cached?.graph) {
      res.status(404).json({ error: 'Claims not found or expired' });
      return;
    }
    const claims = cached.graph.nodes.filter((n) => n.type === 'claim');
    res.json({ conditionId: cached.conditionId, claims });
  });

  // Get narrative spread pattern
  router.get('/narrative/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const timeline = store.getNarrativeTimeline(id);
    if (timeline.length === 0) {
      res.status(404).json({ error: 'No narrative data found' });
      return;
    }
    res.json({ conditionId: id, timeline });
  });

  // Get analysis history
  router.get('/history/:id', (req: Request, res: Response) => {
    const id = sanitizeId(req.params.id);
    if (!id) { res.status(400).json({ error: 'Invalid ID' }); return; }
    const limit = Math.min(Math.max(parseInt(req.query.limit as string) || 20, 1), 50);
    const history = store.getHistory(id, limit);
    res.json({ conditionId: id, history });
  });

  // Verify a specific claim (auth-protected)
  router.post('/verify-claim', auth, async (req: Request, res: Response) => {
    try {
      const { claimText, conditionId } = req.body;
      if (!claimText || typeof claimText !== 'string') {
        res.status(400).json({ error: 'claimText is required and must be a string' });
        return;
      }
      if (claimText.length > 2000) {
        res.status(400).json({ error: 'claimText must be under 2000 characters' });
        return;
      }

      const { createHash } = await import('crypto');
      const normalized = claimText.toLowerCase().replace(/\s+/g, ' ').trim();
      const claimHash = createHash('sha256').update(normalized).digest('hex');

      const sanitizedConditionId = conditionId ? sanitizeId(conditionId) : null;
      const cached = sanitizedConditionId ? store.getCached(sanitizedConditionId) : null;
      if (cached?.graph) {
        const matchingClaim = cached.graph.nodes.find(
          (n) => n.type === 'claim' && n.data?.claimHash === claimHash,
        );

        if (matchingClaim) {
          res.json({ found: true, claimHash, claim: matchingClaim });
          return;
        }
      }

      res.json({
        found: false,
        claimHash,
        message: 'Claim not found in analyzed context graphs. Run /analyze first.',
      });
    } catch (error) {
      const msg = error instanceof Error ? error.message : 'Verification failed';
      res.status(500).json({ error: msg });
    }
  });

  // Track a market for automated re-analysis (auth-protected)
  router.post('/track', auth, (req: Request, res: Response) => {
    if (!tickAnalyzer) {
      res.status(503).json({ error: 'Tick analyzer not available' });
      return;
    }
    const { conditionId, odds } = req.body;
    if (!conditionId || typeof conditionId !== 'string') {
      res.status(400).json({ error: 'conditionId is required' });
      return;
    }
    const sanitized = sanitizeId(conditionId);
    if (!sanitized) { res.status(400).json({ error: 'Invalid conditionId' }); return; }
    tickAnalyzer.trackMarket(sanitized, odds || {});
    res.json({ tracked: true, conditionId: sanitized });
  });

  router.post('/untrack', auth, (req: Request, res: Response) => {
    if (!tickAnalyzer) {
      res.status(503).json({ error: 'Tick analyzer not available' });
      return;
    }
    const { conditionId } = req.body;
    const sanitized = sanitizeId(conditionId);
    if (!sanitized) { res.status(400).json({ error: 'Invalid conditionId' }); return; }
    tickAnalyzer.untrackMarket(sanitized);
    res.json({ untracked: true, conditionId: sanitized });
  });

  return router;
}

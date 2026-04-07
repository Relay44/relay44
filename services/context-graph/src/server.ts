import express, { type Request, type Response, type NextFunction } from 'express';
import type { DKGClientInterface } from './dkg/client.js';
import type { ServiceConfig } from './types/index.js';
import { createRouter } from './api/routes.js';
import type { TickAnalyzer } from './workers/tick-analyzer.js';
import type Database from 'better-sqlite3';

export function createServer(
  dkg: DKGClientInterface,
  db: Database.Database,
  config: ServiceConfig,
  tickAnalyzer?: TickAnalyzer,
): express.Application {
  const app = express();

  // Trust proxy (Render, Cloudflare, etc.)
  app.set('trust proxy', 1);

  // Request size limit
  app.use(express.json({ limit: '512kb' }));

  // Request ID for tracing
  app.use((req: Request, _res: Response, next: NextFunction) => {
    (req as any).requestId = Math.random().toString(36).slice(2, 10);
    next();
  });

  // CORS
  const allowedOrigins = process.env.CORS_ALLOWED_ORIGINS?.split(',').map(s => s.trim()) || ['*'];
  app.use((req: Request, res: Response, next: NextFunction) => {
    const origin = req.headers.origin || '*';
    if (allowedOrigins.includes('*') || allowedOrigins.includes(origin)) {
      res.header('Access-Control-Allow-Origin', origin);
    }
    res.header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');
    res.header('Access-Control-Max-Age', '86400');
    if (req.method === 'OPTIONS') {
      res.sendStatus(204);
      return;
    }
    next();
  });

  // Access logging
  app.use((req: Request, res: Response, next: NextFunction) => {
    const start = Date.now();
    const id = (req as any).requestId;
    res.on('finish', () => {
      const duration = Date.now() - start;
      if (req.path !== '/api/context-graph/health') {
        console.log(`[http] ${req.method} ${req.path} ${res.statusCode} ${duration}ms [${id}]`);
      }
    });
    next();
  });

  // Mount API routes
  app.use('/api/context-graph', createRouter(dkg, db, config, tickAnalyzer));

  // 404 handler
  app.use((_req: Request, res: Response) => {
    res.status(404).json({ error: 'Not found' });
  });

  // Global error handler
  app.use((err: Error, _req: Request, res: Response, _next: NextFunction) => {
    console.error('[http] Unhandled error:', err.message);
    res.status(500).json({ error: 'Internal server error' });
  });

  return app;
}

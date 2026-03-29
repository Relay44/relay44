#!/usr/bin/env node

import express from 'express';

const HOST = process.env.POLYMARKET_FORWARDER_HOST || '0.0.0.0';
const PORT = Number(process.env.PORT || process.env.POLYMARKET_FORWARDER_PORT || 8099);
const SHARED_SECRET = String(process.env.POLYMARKET_FORWARDER_SHARED_SECRET || '').trim();
const TARGET = String(process.env.POLYMARKET_FORWARDER_TARGET || 'https://clob.polymarket.com')
  .trim()
  .replace(/\/$/, '');
const REQUEST_TIMEOUT_MS = Number(process.env.POLYMARKET_FORWARDER_TIMEOUT_MS || 20000);
const ALLOWED_METHODS = new Set(['POST', 'DELETE']);
const ALLOWED_PATHS = new Set(['/order']);
const ALLOWED_HEADERS = new Set([
  'content-type',
  'poly_address',
  'poly_api_key',
  'poly_passphrase',
  'poly_signature',
  'poly_timestamp',
  'poly_builder_api_key',
  'poly_builder_passphrase',
  'poly_builder_signature',
  'poly_builder_timestamp',
]);

function normalizeError(error) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function requireSharedSecret(req, res, next) {
  if (!SHARED_SECRET) {
    res.status(500).json({ error: 'forwarder secret is not configured' });
    return;
  }

  if (req.headers.authorization === `Bearer ${SHARED_SECRET}`) {
    next();
    return;
  }

  res.status(401).json({ error: 'unauthorized' });
}

function normalizeMethod(raw) {
  return String(raw || '').trim().toUpperCase();
}

function normalizePath(raw) {
  const path = String(raw || '').trim();
  return path.startsWith('/') ? path : `/${path}`;
}

function sanitizeHeaders(raw) {
  const input = raw && typeof raw === 'object' ? raw : {};
  const output = {};
  for (const [name, value] of Object.entries(input)) {
    const header = String(name || '').trim();
    const normalized = header.toLowerCase();
    if (!header || !ALLOWED_HEADERS.has(normalized)) {
      continue;
    }
    output[header] = String(value ?? '').trim();
  }
  return output;
}

const app = express();
app.use(express.json({ limit: '512kb' }));

app.get('/health', (_req, res) => {
  res.json({
    ok: true,
    target: TARGET,
    allowedMethods: Array.from(ALLOWED_METHODS),
    allowedPaths: Array.from(ALLOWED_PATHS),
  });
});

app.post('/forward', requireSharedSecret, async (req, res) => {
  const method = normalizeMethod(req.body?.method);
  const path = normalizePath(req.body?.path);
  const body = typeof req.body?.body === 'string' ? req.body.body : '';
  const headers = sanitizeHeaders(req.body?.headers);

  if (!ALLOWED_METHODS.has(method)) {
    res.status(400).json({ error: 'method not allowed' });
    return;
  }
  if (!ALLOWED_PATHS.has(path)) {
    res.status(400).json({ error: 'path not allowed' });
    return;
  }

  try {
    const response = await fetch(`${TARGET}${path}`, {
      method,
      headers,
      body,
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });
    const text = await response.text();
    const contentType = response.headers.get('content-type');
    if (contentType) {
      res.setHeader('content-type', contentType);
    }
    res.status(response.status).send(text);
  } catch (error) {
    res.status(502).json({ error: normalizeError(error) });
  }
});

app.listen(PORT, HOST, () => {
  // eslint-disable-next-line no-console
  console.log(`polymarket forwarder listening on http://${HOST}:${PORT}`);
});

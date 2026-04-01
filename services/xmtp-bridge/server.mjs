#!/usr/bin/env node

import express from 'express';
import { Agent } from '@xmtp/agent-sdk';

const HOST = process.env.XMTP_BRIDGE_HOST || '127.0.0.1';
const PORT = Number(process.env.PORT || process.env.XMTP_BRIDGE_PORT || 8090);

const AGENT_MAX_AGE_MS = 10 * 60 * 1000; // 10 minutes
const SYNC_TIMEOUT_MS = 10_000; // 10 seconds

let agentPromise;
let agentCreatedAt = 0;

function assertNodeRuntime() {
  const major = Number((process.versions.node || '0').split('.')[0] || 0);
  if (major >= 22) return;

  // eslint-disable-next-line no-console
  console.error(
    `xmtp bridge requires Node.js >=22 (current ${process.versions.node}).`,
  );
  process.exit(1);
}

assertNodeRuntime();

function isEthAddress(value) {
  return typeof value === 'string' && /^0x[a-fA-F0-9]{40}$/.test(value.trim());
}

function badRequest(res, error) {
  return res.status(400).json({ error });
}

function invalidateAgent() {
  agentPromise = null;
  agentCreatedAt = 0;
}

async function getAgent() {
  if (agentPromise && Date.now() - agentCreatedAt > AGENT_MAX_AGE_MS) {
    // eslint-disable-next-line no-console
    console.warn('xmtp agent stale, recreating');
    invalidateAgent();
  }
  if (!agentPromise) {
    agentCreatedAt = Date.now();
    agentPromise = Agent.createFromEnv();
  }
  return agentPromise;
}

function withTimeout(promise, ms, label = 'operation') {
  return Promise.race([
    promise,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms),
    ),
  ]);
}

async function syncConversations(agent) {
  try {
    await withTimeout(agent.client.conversations.sync(), SYNC_TIMEOUT_MS, 'conversations.sync()');
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error('conversations.sync() failed, invalidating agent:', err.message);
    invalidateAgent();
    throw err;
  }
}

async function resolveConversation(agent, swarmId) {
  const normalized = String(swarmId || '').trim();
  if (!normalized) {
    throw new Error('swarm_id is required');
  }

  if (isEthAddress(normalized)) {
    return agent.createDmWithAddress(normalized);
  }

  const context = await agent.getConversationContext(normalized);
  if (!context) {
    const err = new Error(`Unknown XMTP conversation id: ${normalized}`);
    err.statusCode = 400;
    throw err;
  }
  return context.conversation;
}

function parseEnvelope(rawMessage) {
  const fallback = typeof rawMessage?.fallback === 'string' ? rawMessage.fallback : '';
  const content = typeof rawMessage?.content === 'string' ? rawMessage.content : fallback;
  if (!content) {
    return {
      sender: rawMessage.senderInboxId || '',
      message: '',
      metadata: null,
    };
  }

  try {
    const parsed = JSON.parse(content);
    if (parsed && typeof parsed === 'object') {
      return {
        sender: typeof parsed.sender === 'string' ? parsed.sender : rawMessage.senderInboxId || '',
        message: typeof parsed.message === 'string' ? parsed.message : content,
        metadata: parsed.metadata ?? null,
      };
    }
  } catch {
    // fall through
  }

  return {
    sender: rawMessage.senderInboxId || '',
    message: content,
    metadata: null,
  };
}

function normalizePagination(query) {
  const parsedLimit = Number(query.limit ?? 50);
  const limit = Number.isFinite(parsedLimit) ? Math.min(Math.max(parsedLimit, 1), 200) : 50;
  const cursor = typeof query.cursor === 'string' && query.cursor ? query.cursor : null;
  // Keep offset for backward compat but only within a single page
  const parsedOffset = Number(query.offset ?? 0);
  const offset = Number.isFinite(parsedOffset) ? Math.max(parsedOffset, 0) : 0;
  return { limit, offset, cursor };
}

const app = express();
app.use(express.json({ limit: '256kb' }));

app.get('/health', async (_req, res) => {
  try {
    const agent = await getAgent();
    res.json({
      ok: true,
      env: process.env.XMTP_ENV || 'production',
      address: agent.address || null,
      mode: 'xmtp_http_bridge',
    });
  } catch (error) {
    res.status(500).json({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
});

app.post('/swarm/send', async (req, res) => {
  const { swarm_id, sender, message, signature, metadata } = req.body || {};
  if (!swarm_id || typeof swarm_id !== 'string') {
    return badRequest(res, 'swarm_id must be a string');
  }
  if (!sender || typeof sender !== 'string') {
    return badRequest(res, 'sender must be a string');
  }
  if (!message || typeof message !== 'string') {
    return badRequest(res, 'message must be a string');
  }

  try {
    const agent = await getAgent();
    await syncConversations(agent);
    const conversation = await resolveConversation(agent, swarm_id);
    const unixMs = Date.now();
    const createdAt = new Date(unixMs).toISOString();
    const payload = JSON.stringify({
      sender,
      message,
      metadata: metadata ?? null,
      swarm_id,
      created_at: createdAt,
      unix_ms: unixMs,
    });
    const messageId = await conversation.sendText(payload);

    return res.status(201).json({
      id: messageId,
      swarm_id,
      topic: conversation.id,
      sender,
      message,
      signature: typeof signature === 'string' ? signature : '',
      metadata: metadata ?? null,
      created_at: createdAt,
      unix_ms: unixMs,
    });
  } catch (error) {
    const status = error?.statusCode || 500;
    return res.status(status).json({
      error: error instanceof Error ? error.message : String(error),
    });
  }
});

app.get('/swarm/:swarmId/messages', async (req, res) => {
  const { limit, offset, cursor } = normalizePagination(req.query);
  const swarmId = req.params.swarmId;

  try {
    const agent = await getAgent();
    await syncConversations(agent);
    const conversation = await resolveConversation(agent, swarmId);

    // Cursor-based pagination: fetch one extra to detect next page
    const fetchOpts = { limit: limit + offset + 1 };
    if (cursor) {
      fetchOpts.afterNs = cursor;
    }
    const messages = await conversation.messages(fetchOpts);

    messages.sort((a, b) => a.sentAt.getTime() - b.sentAt.getTime());

    const sliced = messages.slice(offset, offset + limit);
    const hasMore = messages.length > offset + limit;

    const page = sliced.map((entry) => {
      const envelope = parseEnvelope(entry);
      return {
        id: entry.id,
        swarm_id: swarmId,
        topic: conversation.id,
        sender: envelope.sender,
        message: envelope.message,
        signature: '',
        metadata: envelope.metadata,
        created_at: entry.sentAt.toISOString(),
        unix_ms: entry.sentAt.getTime(),
      };
    });

    const nextCursor = hasMore && page.length > 0
      ? String(page[page.length - 1].unix_ms * 1_000_000)
      : null;

    return res.json({
      data: page,
      total_returned: page.length,
      limit,
      offset,
      cursor: nextCursor,
      has_more: hasMore,
      topic: conversation.id,
    });
  } catch (error) {
    const status = error?.statusCode || 500;
    return res.status(status).json({
      error: error instanceof Error ? error.message : String(error),
    });
  }
});

app.listen(PORT, HOST, () => {
  // eslint-disable-next-line no-console
  console.log(`xmtp bridge listening on http://${HOST}:${PORT}`);
});

process.on('unhandledRejection', (reason) => {
  // eslint-disable-next-line no-console
  console.error('unhandledRejection:', reason);
});

process.on('uncaughtException', (err) => {
  // eslint-disable-next-line no-console
  console.error('uncaughtException:', err);
  process.exit(1);
});

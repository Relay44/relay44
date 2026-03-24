#!/usr/bin/env node

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';

const DEFAULT_API_BASE = 'http://127.0.0.1:8080/v1';
const JSON_HEADERS = {
  'content-type': 'application/json',
  accept: 'application/json',
};

const apiBase = resolveApiBase();
const transportEndpoint = `${apiBase}/web4/mcp`;
let rpcId = 0;

function resolveApiBase() {
  const candidate =
    process.env.RELAY44_API_BASE_URL ||
    process.env.PUBLIC_API_URL ||
    process.env.NEXT_PUBLIC_API_URL ||
    DEFAULT_API_BASE;

  const trimmed = String(candidate || '').trim();
  if (!trimmed) return DEFAULT_API_BASE;

  let normalized = trimmed.replace(//+$/, '');
  if (!normalized.endsWith('/v1')) {
    normalized = `${normalized}/v1`;
  }
  return normalized;
}

function toPrettyText(payload) {
  try {
    return JSON.stringify(payload, null, 2);
  } catch {
    return String(payload);
  }
}

function ensureToolResultShape(payload) {
  if (payload && typeof payload === 'object' && Array.isArray(payload.content)) {
    return {
      content: payload.content,
      structuredContent: payload.structuredContent ?? payload,
      isError: Boolean(payload.isError),
    };
  }

  return {
    content: [{ type: 'text', text: toPrettyText(payload) }],
    structuredContent: payload,
    isError: false,
  };
}

function ensureResourceResultShape(uri, payload) {
  if (payload && typeof payload === 'object' && Array.isArray(payload.contents)) {
    return payload;
  }

  return {
    contents: [
      {
        uri,
        mimeType: 'application/json',
        text: toPrettyText(payload),
      },
    ],
  };
}

function ensurePromptResultShape(payload) {
  if (payload && typeof payload === 'object' && Array.isArray(payload.messages)) {
    return payload;
  }

  return {
    description: 'Generated prompt',
    messages: [
      {
        role: 'user',
        content: {
          type: 'text',
          text: toPrettyText(payload),
        },
      },
    ],
  };
}

async function callBackend(method, params = {}) {
  const request = {
    jsonrpc: '2.0',
    id: ++rpcId,
    method,
    params,
  };

  let response;
  try {
    response = await fetch(transportEndpoint, {
      method: 'POST',
      headers: JSON_HEADERS,
      body: JSON.stringify(request),
    });
  } catch (error) {
    throw new Error(`Failed to reach backend MCP endpoint: ${error.message}`);
  }

  let body = null;
  try {
    body = await response.json();
  } catch {
    body = null;
  }

  if (!response.ok) {
    throw new Error(`Backend MCP endpoint returned ${response.status}`);
  }

  if (!body || typeof body !== 'object') {
    throw new Error('Backend MCP endpoint returned invalid JSON-RPC payload');
  }

  if (body.error) {
    throw new Error(body.error.message || 'Backend MCP method failed');
  }

  return body.result;
}

async function callTool(name, args = {}) {
  const payload = await callBackend('tools/call', { name, arguments: args });
  return ensureToolResultShape(payload);
}

async function readResource(uri) {
  const payload = await callBackend('resources/read', { uri });
  return ensureResourceResultShape(uri, payload);
}

async function getPrompt(name, args = {}) {
  const payload = await callBackend('prompts/get', { name, arguments: args });
  return ensurePromptResultShape(payload);
}

function withRuntimeError(error) {
  return {
    content: [{ type: 'text', text: `Runtime error: ${error.message}` }],
    structuredContent: { ok: false, error: error.message },
    isError: true,
  };
}

async function main() {
  const server = new McpServer({
    name: 'relay44-mcp',
    version: '1.0.0',
  });

  server.registerTool(
    'getMarkets',
    {
      description: 'List unified markets with source/tradable filters.',
      inputSchema: z.object({
        limit: z.number().int().min(1).max(200).optional(),
        offset: z.number().int().min(0).optional(),
        source: z.enum(['all', 'internal', 'limitless', 'polymarket']).optional(),
        tradable: z.enum(['all', 'user', 'agent']).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('getMarkets', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'getOrderBook',
    {
      description: 'Fetch market orderbook (x402 payment required when enabled).',
      inputSchema: z.object({
        market_id: z.union([z.string(), z.number().int().min(1)]),
        outcome: z.enum(['yes', 'no']),
        depth: z.number().int().min(1).max(100).optional(),
        payment: z
          .object({
            resource: z.string(),
            amount_microusdc: z.number().int().nonnegative(),
            nonce: z.string(),
            expires_at: z.number().int().nonnegative(),
            tx_hash: z.string(),
            signature: z.string(),
          })
          .optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('getOrderBook', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'getTrades',
    {
      description: 'Fetch market trades (x402 payment required when enabled).',
      inputSchema: z.object({
        market_id: z.union([z.string(), z.number().int().min(1)]),
        outcome: z.enum(['yes', 'no']).optional(),
        limit: z.number().int().min(1).max(200).optional(),
        offset: z.number().int().min(0).optional(),
        payment: z
          .object({
            resource: z.string(),
            amount_microusdc: z.number().int().nonnegative(),
            nonce: z.string(),
            expires_at: z.number().int().nonnegative(),
            tx_hash: z.string(),
            signature: z.string(),
          })
          .optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('getTrades', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'getAgents',
    {
      description: 'List autonomous agents.',
      inputSchema: z.object({
        limit: z.number().int().min(1).max(200).optional(),
        offset: z.number().int().min(0).optional(),
        owner: z.string().optional(),
        market_id: z.number().int().min(1).optional(),
        active: z.boolean().optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('getAgents', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'prepareExternalOrder',
    {
      description: 'Create external order intent and return preflight + typed data.',
      inputSchema: z.object({
        provider: z.enum(['limitless', 'polymarket']),
        market_id: z.string().min(1),
        outcome: z.enum(['yes', 'no']),
        side: z.enum(['buy', 'sell']),
        price: z.number().gt(0).lt(1),
        quantity: z.number().gt(0),
        credential_id: z.string().optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('prepareExternalOrder', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'submitExternalOrder',
    {
      description: 'Submit signed external order intent.',
      inputSchema: z.object({
        intent_id: z.string().min(1),
        signed_order: z.record(z.any()),
        credential_id: z.string().optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('submitExternalOrder', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'cancelExternalOrder',
    {
      description: 'Cancel external venue order.',
      inputSchema: z.object({
        provider: z.enum(['limitless', 'polymarket']),
        provider_order_id: z.string().min(1),
        credential_id: z.string().optional(),
        payload: z.record(z.any()).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('cancelExternalOrder', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'listExternalAgents',
    {
      description: 'List external agents.',
      inputSchema: z.object({
        provider: z.enum(['limitless', 'polymarket']).optional(),
        active: z.boolean().optional(),
        limit: z.number().int().min(1).max(200).optional(),
        offset: z.number().int().min(0).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('listExternalAgents', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'executeExternalAgent',
    {
      description: 'Execute external agent cycle.',
      inputSchema: z.object({
        agent_id: z.string().min(1),
        force: z.boolean().optional(),
        signed_order: z.record(z.any()).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('executeExternalAgent', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'prepareCreateAgentTx',
    {
      description: 'Prepare calldata for createAgent wallet execution.',
      inputSchema: z.object({
        from: z.string().optional(),
        marketId: z.number().int().min(1),
        isYes: z.boolean(),
        priceBps: z.number().int().min(1).max(9999),
        size: z.string().min(1),
        cadence: z.number().int().min(1),
        expiryWindow: z.number().int().min(1),
        strategy: z.string().min(1),
      }),
    },
    async (args) => {
      try {
        return await callTool('prepareCreateAgentTx', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'prepareExecuteAgentTx',
    {
      description: 'Prepare calldata for executeAgent wallet execution.',
      inputSchema: z.object({
        from: z.string().optional(),
        agentId: z.number().int().min(1),
      }),
    },
    async (args) => {
      try {
        return await callTool('prepareExecuteAgentTx', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'getX402Quote',
    {
      description: 'Get x402 quote for premium resources.',
      inputSchema: z.object({
        resource: z.enum(['orderbook', 'trades', 'mcp_tool_call']),
      }),
    },
    async (args) => {
      try {
        return await callTool('getX402Quote', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'sendSwarmMessage',
    {
      description: 'Send signed XMTP swarm message.',
      inputSchema: z.object({
        swarm_id: z.string().min(3),
        sender: z.string().min(3),
        message: z.string().min(1),
        signature: z.string().min(1),
        metadata: z.record(z.any()).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('sendSwarmMessage', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerTool(
    'listSwarmMessages',
    {
      description: 'List XMTP swarm messages.',
      inputSchema: z.object({
        swarm_id: z.string().min(3),
        limit: z.number().int().min(1).max(200).optional(),
        offset: z.number().int().min(0).optional(),
      }),
    },
    async (args) => {
      try {
        return await callTool('listSwarmMessages', args);
      } catch (error) {
        return withRuntimeError(error);
      }
    },
  );

  server.registerResource(
    'live-markets',
    'relay44://markets/live',
    {
      title: 'Live markets',
      description: 'Current market list from MarketCore.',
      mimeType: 'application/json',
    },
    async () => {
      return await readResource('relay44://markets/live');
    },
  );

  server.registerResource(
    'active-agents',
    'relay44://agents/active',
    {
      title: 'Active agents',
      description: 'Active AgentRuntime entries with execution readiness.',
      mimeType: 'application/json',
    },
    async () => {
      return await readResource('relay44://agents/active');
    },
  );

  server.registerResource(
    'runtime-health',
    'relay44://runtime/health',
    {
      title: 'Web4 runtime health',
      description: 'Current MCP/x402/XMTP runtime readiness state.',
      mimeType: 'application/json',
    },
    async () => {
      return await readResource('relay44://runtime/health');
    },
  );

  server.registerResource(
    'xmtp-health',
    'relay44://xmtp/health',
    {
      title: 'XMTP swarm health',
      description: 'XMTP swarm runtime configuration and limits.',
      mimeType: 'application/json',
    },
    async () => {
      return await readResource('relay44://xmtp/health');
    },
  );

  server.registerPrompt(
    'market-scan',
    {
      description: 'Scan active markets and return ranked opportunities.',
      argsSchema: {
        limit: z.coerce.number().int().min(1).max(50).optional(),
      },
    },
    async (args) => {
      return await getPrompt('market-scan', args);
    },
  );

  server.registerPrompt(
    'market-analysis',
    {
      description: 'Analyze market structure, liquidity and executable opportunities.',
      argsSchema: {
        market_id: z.coerce.number().int().min(1),
      },
    },
    async (args) => {
      return await getPrompt('market-analysis', args);
    },
  );

  server.registerPrompt(
    'agent-launch',
    {
      description: 'Generate agent launch params from risk budget and target outcome.',
      argsSchema: {
        market_id: z.coerce.number().int().min(1),
        outcome: z.enum(['yes', 'no']),
        budget_usdc: z.string().min(1),
      },
    },
    async (args) => {
      return await getPrompt('agent-launch', args);
    },
  );

  server.registerPrompt(
    'swarm-coordination',
    {
      description: 'Coordinate an XMTP swarm plan for executing market agents.',
      argsSchema: {
        swarm_id: z.string().min(3),
        objective: z.string().min(1),
      },
    },
    async (args) => {
      return await getPrompt('swarm-coordination', args);
    },
  );

  const transport = new StdioServerTransport();
  await server.connect(transport);

  process.stderr.write(
    `[relay44-mcp] connected via stdio, backend=${transportEndpoint}\n`,
  );
}

main().catch((error) => {
  process.stderr.write(`[relay44-mcp] fatal: ${error.message}\n`);
  process.exitCode = 1;
});

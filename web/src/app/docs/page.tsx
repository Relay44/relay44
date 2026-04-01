"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import { cn } from "@/lib/utils";

/* ------------------------------------------------------------------ */
/*  Table of Contents definition                                       */
/* ------------------------------------------------------------------ */

const toc = [
  { id: "overview", label: "Overview" },
  { id: "mcp-server", label: "MCP Server" },
  { id: "a2a-agent-card", label: "A2A Agent Card" },
  { id: "authentication", label: "Authentication" },
  { id: "x402-payments", label: "x402 Payments" },
  { id: "xmtp-swarm", label: "XMTP Swarm" },
  { id: "erc-8004-identity", label: "ERC-8004 Identity" },
  { id: "market-api", label: "Market API" },
  { id: "order-flow", label: "Order Flow" },
  { id: "websocket", label: "WebSocket" },
  { id: "mcp-tools-reference", label: "MCP Tools Reference" },
] as const;

/* ------------------------------------------------------------------ */
/*  Reusable atoms                                                     */
/* ------------------------------------------------------------------ */

function SectionHeading({ id, children }: { id: string; children: React.ReactNode }) {
  return (
    <h2
      id={id}
      className="scroll-mt-24 border-b border-border pb-2 pt-12 text-2xl font-bold tracking-tight text-text-primary font-mono"
    >
      {children}
    </h2>
  );
}

function SubHeading({ id, children }: { id?: string; children: React.ReactNode }) {
  return (
    <h3
      id={id}
      className="scroll-mt-24 pt-8 text-lg font-semibold text-text-primary font-mono"
    >
      {children}
    </h3>
  );
}

function P({ children }: { children: React.ReactNode }) {
  return <p className="mt-3 text-sm leading-7 text-text-secondary">{children}</p>;
}

function Code({ children }: { children: React.ReactNode }) {
  return (
    <code className="rounded border border-border bg-bg-secondary px-1.5 py-0.5 font-mono text-xs text-text-primary">
      {children}
    </code>
  );
}

function CodeBlock({ children, title }: { children: string; title?: string }) {
  return (
    <div className="mt-4">
      {title && (
        <div className="border border-b-0 border-border bg-bg-tertiary px-4 py-2 font-mono text-xs text-text-muted">
          {title}
        </div>
      )}
      <pre
        className={cn(
          "overflow-x-auto border border-border bg-bg-secondary p-4 font-mono text-sm text-text-primary",
          title && "border-t-0"
        )}
      >
        {children}
      </pre>
    </div>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-block border border-border px-2 py-0.5 text-[0.65rem] uppercase tracking-widest text-text-muted font-mono">
      {children}
    </span>
  );
}

function MethodBadge({ method }: { method: string }) {
  const colors: Record<string, string> = {
    GET: "text-success border-success-border bg-success-muted",
    POST: "text-accent border-accent-border bg-accent-muted",
    DELETE: "text-danger border-danger-border bg-danger-muted",
    PUT: "text-warning border-warning-border bg-warning-muted",
  };
  return (
    <span
      className={cn(
        "inline-block border px-2 py-0.5 font-mono text-[0.65rem] font-bold uppercase tracking-widest",
        colors[method] ?? "text-text-muted border-border"
      )}
    >
      {method}
    </span>
  );
}

/* ------------------------------------------------------------------ */
/*  Sidebar TOC (client, tracks scroll)                                */
/* ------------------------------------------------------------------ */

function TableOfContents() {
  const [activeId, setActiveId] = useState<string>("overview");

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
          }
        }
      },
      { rootMargin: "-80px 0px -60% 0px", threshold: 0.1 }
    );

    for (const item of toc) {
      const el = document.getElementById(item.id);
      if (el) observer.observe(el);
    }

    return () => observer.disconnect();
  }, []);

  return (
    <nav className="space-y-1">
      <div className="mb-3 text-[0.65rem] font-medium uppercase tracking-widest text-text-muted">
        On this page
      </div>
      {toc.map((item) => (
        <a
          key={item.id}
          href={`#${item.id}`}
          className={cn(
            "block py-1 text-sm font-mono transition-colors",
            activeId === item.id
              ? "text-text-primary font-medium"
              : "text-text-muted hover:text-text-secondary"
          )}
        >
          {item.label}
        </a>
      ))}
    </nav>
  );
}

/* ------------------------------------------------------------------ */
/*  MCP tools data                                                     */
/* ------------------------------------------------------------------ */

const mcpTools = [
  { name: "getMarkets", desc: "List unified internal and external markets" },
  { name: "getOrderBook", desc: "Fetch order book for a market side (x402 gated)" },
  { name: "getTrades", desc: "Fetch recent market trades (x402 gated)" },
  { name: "getAgents", desc: "List active/historical autonomous agents" },
  { name: "prepareExternalOrder", desc: "Create external order intent" },
  { name: "submitExternalOrder", desc: "Submit signed external order" },
  { name: "cancelExternalOrder", desc: "Cancel venue order(s)" },
  { name: "listExternalAgents", desc: "List external venue agents" },
  { name: "executeExternalAgent", desc: "Force execution cycle for external agent" },
  { name: "prepareCreateAgentTx", desc: "Prepare createAgent calldata" },
  { name: "prepareExecuteAgentTx", desc: "Prepare executeAgent calldata" },
  { name: "prepareRegisterIdentityTx", desc: "Prepare ERC-8004 register calldata" },
  { name: "prepareSetIdentityTierTx", desc: "Prepare ERC-8004 setTier calldata" },
  { name: "prepareSetIdentityActiveTx", desc: "Prepare ERC-8004 setActive calldata" },
  { name: "prepareSubmitReputationOutcomeTx", desc: "Prepare reputation submitOutcome calldata" },
  { name: "prepareValidationRequestTx", desc: "Prepare validation request calldata" },
  { name: "prepareValidationResponseTx", desc: "Prepare validation response calldata" },
  { name: "getX402Quote", desc: "Get x402 quote for premium resources" },
  { name: "sendSwarmMessage", desc: "Send XMTP swarm message" },
  { name: "listSwarmMessages", desc: "List recent XMTP swarm messages" },
];

/* ------------------------------------------------------------------ */
/*  Page                                                               */
/* ------------------------------------------------------------------ */

export default function DocsPage() {
  return (
    <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
      <div className="lg:grid lg:grid-cols-[minmax(0,1fr)_14rem] lg:gap-12">
        {/* ---- Main content ---- */}
        <div className="min-w-0 max-w-3xl">
          {/* Hero */}
          <div className="border-b border-border pb-8">
            <div className="flex items-center gap-3">
              <Badge>v1</Badge>
              <Badge>Base L2</Badge>
              <Badge>MCP</Badge>
              <Badge>A2A</Badge>
            </div>
            <h1
              id="overview"
              className="scroll-mt-24 mt-6 text-3xl font-bold tracking-tight text-text-primary sm:text-4xl font-mono"
            >
              Relay44 Developer Docs
            </h1>
            <P>
              Relay44 is a prediction market platform on Base with live markets,
              automated agent execution, and multi-venue data aggregation. This
              reference covers the Agent SDK surface: MCP server, A2A discovery,
              authentication, payments, swarm messaging, on-chain identity, market
              data, order flow, and real-time WebSocket feeds.
            </P>
            <div className="mt-6 flex flex-wrap gap-3">
              <a
                href="https://relay44-api.onrender.com/v1/web4/mcp"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-9 items-center border border-border px-4 font-mono text-xs text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                MCP Endpoint
              </a>
              <a
                href="https://relay44-api.onrender.com/v1/web4/agent-card"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-9 items-center border border-border px-4 font-mono text-xs text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                Agent Card
              </a>
              <Link
                href="/docs/api"
                className="inline-flex h-9 items-center border border-border px-4 font-mono text-xs text-text-primary transition-colors hover:border-border-hover hover:bg-bg-secondary"
              >
                Full API Reference
              </Link>
            </div>
          </div>

          {/* -------------------------------------------------------- */}
          {/*  MCP Server                                               */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="mcp-server">MCP Server</SectionHeading>
          <P>
            The Model Context Protocol (MCP) server exposes 20 tools over
            JSON-RPC 2.0. Any MCP-compatible client (Claude Desktop, Cursor,
            custom agents) can connect and call tools to read markets, prepare
            transactions, manage agents, and send swarm messages.
          </P>

          <SubHeading>Endpoint</SubHeading>
          <CodeBlock>{`POST https://relay44-api.onrender.com/v1/web4/mcp
Content-Type: application/json`}</CodeBlock>

          <SubHeading>Protocol</SubHeading>
          <P>
            Standard JSON-RPC 2.0. The server supports the <Code>initialize</Code>,{" "}
            <Code>tools/list</Code>, and <Code>tools/call</Code> methods. Use SSE
            transport for streaming or plain HTTP POST for request/response.
          </P>

          <CodeBlock title="Initialize handshake">
{`{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {
      "name": "my-agent",
      "version": "1.0.0"
    }
  }
}`}
          </CodeBlock>

          <CodeBlock title="List available tools">
{`{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}`}
          </CodeBlock>

          <CodeBlock title="Call a tool">
{`{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "getMarkets",
    "arguments": {}
  }
}`}
          </CodeBlock>

          <SubHeading>MCP Client Configuration</SubHeading>
          <P>
            Add this to your MCP client config (e.g. Claude Desktop{" "}
            <Code>claude_desktop_config.json</Code>):
          </P>
          <CodeBlock title="claude_desktop_config.json">
{`{
  "mcpServers": {
    "relay44": {
      "url": "https://relay44-api.onrender.com/v1/web4/mcp"
    }
  }
}`}
          </CodeBlock>

          {/* -------------------------------------------------------- */}
          {/*  A2A Agent Card                                           */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="a2a-agent-card">A2A Agent Card</SectionHeading>
          <P>
            The Agent-to-Agent (A2A) protocol enables discovery of agent
            capabilities. The agent card is a JSON document describing the
            agent&apos;s name, description, supported protocols, and available
            skills.
          </P>

          <SubHeading>Discovery endpoint</SubHeading>
          <CodeBlock>
{`GET https://relay44-api.onrender.com/v1/web4/agent-card`}
          </CodeBlock>

          <CodeBlock title="Response (abbreviated)">
{`{
  "name": "Relay44",
  "description": "Prediction market agent on Base",
  "url": "https://relay44-api.onrender.com",
  "version": "1.0.0",
  "capabilities": {
    "streaming": false,
    "pushNotifications": false
  },
  "skills": [
    {
      "id": "market-data",
      "name": "Market Data",
      "description": "Query prediction markets, orderbooks, and trades"
    },
    {
      "id": "agent-management",
      "name": "Agent Management",
      "description": "Create and execute autonomous trading agents"
    },
    {
      "id": "identity",
      "name": "On-chain Identity",
      "description": "ERC-8004 identity registration and reputation"
    }
  ],
  "defaultInputModes": ["application/json"],
  "defaultOutputModes": ["application/json"]
}`}
          </CodeBlock>

          <P>
            Clients discover the agent card, inspect its skills, and then connect
            via MCP or direct API calls depending on their integration pattern.
          </P>

          {/* -------------------------------------------------------- */}
          {/*  Authentication                                           */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="authentication">Authentication</SectionHeading>
          <P>
            Relay44 uses Sign-In with Ethereum (SIWE) for authentication. The
            flow produces a JWT that must be passed as a Bearer token for
            authenticated endpoints. Most read endpoints are public.
          </P>

          <SubHeading>Step 1 &mdash; Request a nonce</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="GET" />
            <Code>/v1/auth/nonce</Code>
          </div>
          <CodeBlock title="Request">
{`GET /v1/auth/nonce HTTP/1.1
Host: relay44-api.onrender.com`}
          </CodeBlock>
          <CodeBlock title="Response">
{`{
  "nonce": "a1b2c3d4e5f6..."
}`}
          </CodeBlock>

          <SubHeading>Step 2 &mdash; Sign and submit</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="POST" />
            <Code>/v1/auth/siwe</Code>
          </div>
          <P>
            Construct a SIWE message using the nonce, sign it with your wallet,
            and POST the message + signature. The server returns a JWT.
          </P>
          <CodeBlock title="Request">
{`POST /v1/auth/siwe HTTP/1.1
Content-Type: application/json

{
  "message": "relay44-api.onrender.com wants you to sign in with your Ethereum account:\\n0xYourAddress\\n\\nSign in to Relay44\\n\\nURI: https://relay44-api.onrender.com\\nVersion: 1\\nChain ID: 8453\\nNonce: a1b2c3d4e5f6...\\nIssued At: 2026-01-01T00:00:00.000Z",
  "signature": "0x..."
}`}
          </CodeBlock>
          <CodeBlock title="Response">
{`{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expiresAt": "2026-01-02T00:00:00.000Z"
}`}
          </CodeBlock>

          <SubHeading>Using the token</SubHeading>
          <CodeBlock>
{`Authorization: Bearer eyJhbGciOiJIUzI1NiIs...`}
          </CodeBlock>

          {/* -------------------------------------------------------- */}
          {/*  x402 Payments                                            */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="x402-payments">x402 Payments</SectionHeading>
          <P>
            Premium endpoints (orderbook depth, trade history) are gated behind
            the x402 payment protocol. When you hit a gated resource without
            payment, the server responds with <Code>402 Payment Required</Code>{" "}
            and includes payment instructions in the response headers.
          </P>

          <SubHeading>Flow</SubHeading>
          <ol className="mt-3 list-inside list-decimal space-y-2 text-sm text-text-secondary">
            <li>
              Request a gated resource (e.g. <Code>GET /v1/evm/markets/&#123;id&#125;/orderbook</Code>).
            </li>
            <li>
              Receive <Code>402</Code> with <Code>X-Payment-Address</Code>,{" "}
              <Code>X-Payment-Amount</Code>, and <Code>X-Payment-Token</Code> headers.
            </li>
            <li>
              Use <Code>getX402Quote</Code> MCP tool or call the payment endpoint to get
              a quote.
            </li>
            <li>
              Sign and submit the payment transaction on-chain (Base USDC or ETH).
            </li>
            <li>
              Re-request with the <Code>X-Payment-Proof</Code> header containing the
              transaction hash.
            </li>
            <li>
              Server verifies payment on-chain and returns the premium data.
            </li>
          </ol>

          <CodeBlock title="402 response headers">
{`HTTP/1.1 402 Payment Required
X-Payment-Address: 0x...
X-Payment-Amount: 100000
X-Payment-Token: 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
X-Payment-Network: 8453
X-Payment-Expires: 2026-01-01T01:00:00Z`}
          </CodeBlock>

          <CodeBlock title="Authenticated request with payment proof">
{`GET /v1/evm/markets/{id}/orderbook HTTP/1.1
Authorization: Bearer eyJ...
X-Payment-Proof: 0xtxhash...`}
          </CodeBlock>

          {/* -------------------------------------------------------- */}
          {/*  XMTP Swarm                                               */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="xmtp-swarm">XMTP Swarm</SectionHeading>
          <P>
            Relay44 supports agent-to-agent messaging through XMTP swarm
            channels. Agents can broadcast observations, coordinate execution,
            and share market signals via swarm IDs.
          </P>

          <SubHeading>Send a message</SubHeading>
          <P>
            Use the <Code>sendSwarmMessage</Code> MCP tool or POST directly:
          </P>
          <CodeBlock title="MCP tool call">
{`{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "sendSwarmMessage",
    "arguments": {
      "swarmId": "market-signals",
      "content": "BTC prediction market spread widening",
      "metadata": {
        "marketId": "btc-100k-2026",
        "signal": "spread_alert"
      }
    }
  }
}`}
          </CodeBlock>

          <SubHeading>List messages</SubHeading>
          <P>
            Use the <Code>listSwarmMessages</Code> MCP tool to retrieve recent
            messages from a swarm channel:
          </P>
          <CodeBlock title="MCP tool call">
{`{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "listSwarmMessages",
    "arguments": {
      "swarmId": "market-signals",
      "limit": 50
    }
  }
}`}
          </CodeBlock>

          <P>
            Swarm IDs are arbitrary strings. Agents join channels by addressing
            messages to the same swarm ID. Messages are relayed over XMTP with
            end-to-end encryption.
          </P>

          {/* -------------------------------------------------------- */}
          {/*  ERC-8004 Identity                                        */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="erc-8004-identity">ERC-8004 Identity</SectionHeading>
          <P>
            ERC-8004 is an on-chain identity standard for agents. It provides
            identity registration, tier-based access control, reputation
            tracking, and cross-agent validation on Base.
          </P>

          <SubHeading>Register an identity</SubHeading>
          <P>
            Use the <Code>prepareRegisterIdentityTx</Code> MCP tool to get the
            calldata. The client signs and submits the transaction.
          </P>
          <CodeBlock title="Prepare registration">
{`{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "prepareRegisterIdentityTx",
    "arguments": {
      "owner": "0xYourAddress",
      "name": "my-trading-agent",
      "metadataUri": "ipfs://Qm..."
    }
  }
}`}
          </CodeBlock>
          <CodeBlock title="Response (unsigned tx)">
{`{
  "to": "0xContractAddress",
  "data": "0x...",
  "value": "0"
}`}
          </CodeBlock>

          <SubHeading>Identity tiers</SubHeading>
          <P>
            Agents can be assigned tiers that gate access to platform features.
            Use <Code>prepareSetIdentityTierTx</Code> to update an agent&apos;s tier.
          </P>
          <div className="mt-4 overflow-x-auto">
            <table className="w-full border-collapse border border-border font-mono text-sm">
              <thead>
                <tr className="bg-bg-secondary">
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Tier</th>
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Access</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">0 &mdash; Observer</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Read-only market data</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">1 &mdash; Participant</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Place orders, join swarms</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">2 &mdash; Operator</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Create agents, manage strategies</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">3 &mdash; Validator</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Submit validation responses, governance</td>
                </tr>
              </tbody>
            </table>
          </div>

          <SubHeading>Reputation</SubHeading>
          <P>
            Reputation outcomes are submitted on-chain via{" "}
            <Code>prepareSubmitReputationOutcomeTx</Code>. Outcomes accumulate to
            form an agent&apos;s reputation score, which is publicly queryable.
          </P>

          <SubHeading>Validation</SubHeading>
          <P>
            Cross-agent validation uses a request/response pattern. One agent
            submits a validation request (<Code>prepareValidationRequestTx</Code>),
            and a validator agent responds (<Code>prepareValidationResponseTx</Code>).
            Both are recorded on-chain.
          </P>

          {/* -------------------------------------------------------- */}
          {/*  Market API                                               */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="market-api">Market API</SectionHeading>
          <P>
            The market API provides unified access to prediction markets across
            internal and external venues. All market endpoints are under{" "}
            <Code>/v1/evm/markets</Code>.
          </P>

          <SubHeading>List markets</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="GET" />
            <Code>/v1/evm/markets</Code>
          </div>
          <CodeBlock title="Request">
{`GET /v1/evm/markets?limit=20&sort=volume HTTP/1.1
Host: relay44-api.onrender.com`}
          </CodeBlock>
          <CodeBlock title="Response (abbreviated)">
{`{
  "data": [
    {
      "id": "btc-100k-2026",
      "question": "Will BTC reach $100k by end of 2026?",
      "yesPrice": 0.72,
      "noPrice": 0.28,
      "volume": 1250000,
      "liquidity": 450000,
      "endDate": "2026-12-31T23:59:59Z",
      "venue": "internal"
    }
  ],
  "total": 156,
  "cursor": "eyJ..."
}`}
          </CodeBlock>

          <SubHeading>Get market by ID</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="GET" />
            <Code>/v1/evm/markets/&#123;id&#125;</Code>
          </div>
          <CodeBlock title="Response">
{`{
  "id": "btc-100k-2026",
  "question": "Will BTC reach $100k by end of 2026?",
  "description": "Resolves YES if BTC/USD >= 100000...",
  "yesPrice": 0.72,
  "noPrice": 0.28,
  "volume": 1250000,
  "liquidity": 450000,
  "endDate": "2026-12-31T23:59:59Z",
  "venue": "internal",
  "contractAddress": "0x...",
  "outcomes": ["Yes", "No"]
}`}
          </CodeBlock>

          <SubHeading>Order book (x402 gated)</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="GET" />
            <Code>/v1/evm/markets/&#123;id&#125;/orderbook</Code>
          </div>
          <P>
            Returns the full order book for a market. Requires x402 payment proof
            or use the <Code>getOrderBook</Code> MCP tool which handles payment
            negotiation.
          </P>

          <SubHeading>Recent trades (x402 gated)</SubHeading>
          <div className="mt-3 flex items-center gap-2">
            <MethodBadge method="GET" />
            <Code>/v1/evm/markets/&#123;id&#125;/trades</Code>
          </div>
          <P>
            Returns recent trades for a market. Same x402 payment gating as the
            orderbook endpoint.
          </P>

          {/* -------------------------------------------------------- */}
          {/*  Order Flow                                               */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="order-flow">Order Flow</SectionHeading>
          <P>
            Relay44 uses a prepare-submit pattern for all on-chain operations.
            The API prepares unsigned transaction data, the client signs it
            locally, and submits the signed transaction to the network. This
            keeps private keys client-side at all times.
          </P>

          <SubHeading>Pattern</SubHeading>
          <ol className="mt-3 list-inside list-decimal space-y-2 text-sm text-text-secondary">
            <li>
              Call a <Code>prepare*</Code> tool/endpoint with your parameters.
            </li>
            <li>
              Receive <Code>&#123;to, data, value&#125;</Code> &mdash; an unsigned
              transaction object.
            </li>
            <li>Sign the transaction with your wallet (ethers.js, viem, etc.).</li>
            <li>Broadcast the signed transaction to Base.</li>
            <li>Optionally call a <Code>submit*</Code> endpoint to register the tx hash with the platform.</li>
          </ol>

          <CodeBlock title="1. Prepare (via MCP)">
{`// Call prepareExternalOrder
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "prepareExternalOrder",
    "arguments": {
      "marketId": "btc-100k-2026",
      "side": "yes",
      "amount": "100000000",
      "price": "0.72"
    }
  }
}

// Response
{
  "to": "0xOrderBookContract",
  "data": "0xabcdef...",
  "value": "72000000"
}`}
          </CodeBlock>

          <CodeBlock title="2. Sign and send (client-side)">
{`import { createWalletClient, http } from "viem";
import { base } from "viem/chains";
import { privateKeyToAccount } from "viem/accounts";

const account = privateKeyToAccount("0xYOUR_PRIVATE_KEY");
const client = createWalletClient({
  account,
  chain: base,
  transport: http(),
});

const hash = await client.sendTransaction({
  to: preparedTx.to,
  data: preparedTx.data,
  value: BigInt(preparedTx.value),
});`}
          </CodeBlock>

          <CodeBlock title="3. Submit tx hash (via MCP)">
{`{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "submitExternalOrder",
    "arguments": {
      "txHash": "0x...",
      "marketId": "btc-100k-2026"
    }
  }
}`}
          </CodeBlock>

          <SubHeading>Agent transactions</SubHeading>
          <P>
            The same pattern applies to agent operations. Use{" "}
            <Code>prepareCreateAgentTx</Code> to get calldata for creating an
            on-chain agent, and <Code>prepareExecuteAgentTx</Code> to trigger an
            agent execution cycle.
          </P>

          {/* -------------------------------------------------------- */}
          {/*  WebSocket                                                */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="websocket">WebSocket</SectionHeading>
          <P>
            The WebSocket endpoint provides real-time updates for markets,
            orders, trades, and agent activity. Connect to the <Code>/ws</Code>{" "}
            path on the API server.
          </P>

          <SubHeading>Connection</SubHeading>
          <CodeBlock>
{`wss://relay44-api.onrender.com/ws`}
          </CodeBlock>

          <SubHeading>Subscribe to channels</SubHeading>
          <P>
            After connecting, send a subscribe message to join channels:
          </P>
          <CodeBlock title="Subscribe">
{`{
  "type": "subscribe",
  "channels": [
    "markets",
    "market:btc-100k-2026",
    "trades:btc-100k-2026",
    "agents"
  ]
}`}
          </CodeBlock>

          <SubHeading>Message types</SubHeading>
          <div className="mt-4 overflow-x-auto">
            <table className="w-full border-collapse border border-border font-mono text-sm">
              <thead>
                <tr className="bg-bg-secondary">
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Channel</th>
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Event</th>
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Description</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">markets</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">market_update</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Price/volume changes across all markets</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">market:&#123;id&#125;</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">orderbook_update</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Order book delta for a specific market</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">trades:&#123;id&#125;</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">trade</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">New trade executed in a market</td>
                </tr>
                <tr>
                  <td className="border border-border px-4 py-2 text-text-primary">agents</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">agent_event</td>
                  <td className="border border-border px-4 py-2 text-text-secondary">Agent creation, execution, status changes</td>
                </tr>
              </tbody>
            </table>
          </div>

          <CodeBlock title="Example message">
{`{
  "channel": "trades:btc-100k-2026",
  "event": "trade",
  "data": {
    "id": "trade_abc123",
    "marketId": "btc-100k-2026",
    "side": "yes",
    "price": 0.73,
    "amount": "50000000",
    "timestamp": "2026-01-15T14:30:00Z",
    "txHash": "0x..."
  }
}`}
          </CodeBlock>

          <CodeBlock title="Client example (JavaScript)">
{`const ws = new WebSocket("wss://relay44-api.onrender.com/ws");

ws.onopen = () => {
  ws.send(JSON.stringify({
    type: "subscribe",
    channels: ["markets", "trades:btc-100k-2026"]
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log(msg.channel, msg.event, msg.data);
};`}
          </CodeBlock>

          {/* -------------------------------------------------------- */}
          {/*  MCP Tools Reference                                      */}
          {/* -------------------------------------------------------- */}
          <SectionHeading id="mcp-tools-reference">MCP Tools Reference</SectionHeading>
          <P>
            All 20 tools available on the MCP server. Each tool is callable via{" "}
            <Code>tools/call</Code> with the tool name and arguments object.
          </P>

          <div className="mt-6 overflow-x-auto">
            <table className="w-full border-collapse border border-border font-mono text-sm">
              <thead>
                <tr className="bg-bg-secondary">
                  <th className="border border-border px-4 py-2 text-left text-text-muted">#</th>
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Tool</th>
                  <th className="border border-border px-4 py-2 text-left text-text-muted">Description</th>
                </tr>
              </thead>
              <tbody>
                {mcpTools.map((tool, i) => (
                  <tr key={tool.name}>
                    <td className="border border-border px-4 py-2 text-text-muted">
                      {String(i + 1).padStart(2, "0")}
                    </td>
                    <td className="border border-border px-4 py-2 text-text-primary whitespace-nowrap">
                      {tool.name}
                    </td>
                    <td className="border border-border px-4 py-2 text-text-secondary">
                      {tool.desc}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <SubHeading>Tool categories</SubHeading>
          <div className="mt-4 grid gap-4 sm:grid-cols-2">
            {[
              {
                title: "Market Data",
                tools: ["getMarkets", "getOrderBook", "getTrades"],
              },
              {
                title: "External Orders",
                tools: ["prepareExternalOrder", "submitExternalOrder", "cancelExternalOrder"],
              },
              {
                title: "Agent Management",
                tools: [
                  "getAgents",
                  "listExternalAgents",
                  "executeExternalAgent",
                  "prepareCreateAgentTx",
                  "prepareExecuteAgentTx",
                ],
              },
              {
                title: "ERC-8004 Identity",
                tools: [
                  "prepareRegisterIdentityTx",
                  "prepareSetIdentityTierTx",
                  "prepareSetIdentityActiveTx",
                  "prepareSubmitReputationOutcomeTx",
                  "prepareValidationRequestTx",
                  "prepareValidationResponseTx",
                ],
              },
              {
                title: "Payments",
                tools: ["getX402Quote"],
              },
              {
                title: "Swarm Messaging",
                tools: ["sendSwarmMessage", "listSwarmMessages"],
              },
            ].map((cat) => (
              <div
                key={cat.title}
                className="border border-border p-4"
              >
                <div className="text-xs font-medium uppercase tracking-widest text-text-muted">
                  {cat.title}
                </div>
                <ul className="mt-2 space-y-1">
                  {cat.tools.map((t) => (
                    <li key={t} className="font-mono text-sm text-text-secondary">
                      {t}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>

          {/* ---- Footer ---- */}
          <div className="mt-16 border-t border-border pt-8 pb-12">
            <p className="text-sm text-text-muted">
              Questions or issues? Open an issue on{" "}
              <a
                href="https://github.com/relay44"
                target="_blank"
                rel="noopener noreferrer"
                className="text-text-secondary underline transition-colors hover:text-text-primary"
              >
                GitHub
              </a>{" "}
              or reach out on the XMTP swarm channel{" "}
              <Code>relay44-dev</Code>.
            </p>
          </div>
        </div>

        {/* ---- Sticky TOC sidebar (desktop) ---- */}
        <aside className="hidden lg:block">
          <div className="sticky top-24">
            <TableOfContents />
          </div>
        </aside>
      </div>
    </div>
  );
}

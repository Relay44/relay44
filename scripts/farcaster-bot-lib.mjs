import { privateKeyToAccount } from 'viem/accounts';

// ── Config ──────────────────────────────────────────────────────────────────

function normalizeApiBase(raw) {
  const trimmed = String(raw || '').trim().replace(/\/$/, '');
  if (!trimmed) return 'http://localhost:8080/v1';
  const withScheme =
    trimmed.startsWith('http://') || trimmed.startsWith('https://') ? trimmed : `http://${trimmed}`;
  return withScheme.endsWith('/v1') ? withScheme : `${withScheme}/v1`;
}

export const apiBase = normalizeApiBase(process.env.FARCASTER_BOT_API_URL);
export const apiOrigin = apiBase.replace(/\/v1$/, '');
export const siweDomain = (process.env.FARCASTER_BOT_SIWE_DOMAIN || 'relay44.com').trim();
export const chainId = Number(process.env.FARCASTER_BOT_CHAIN_ID || 8453);
export const siteUrl = (process.env.FARCASTER_BOT_SITE_URL || 'https://relay44.com').trim();

export const PRICE_CHANGE_THRESHOLD = Number(process.env.FARCASTER_BOT_PRICE_CHANGE_THRESHOLD || 10) / 100;
export const VOLUME_THRESHOLD = Number(process.env.FARCASTER_BOT_VOLUME_THRESHOLD || 10000);
export const MAX_CASTS_PER_TICK = Number(process.env.FARCASTER_BOT_MAX_CASTS || 5);
export const COOLDOWN_MS = Number(process.env.FARCASTER_BOT_COOLDOWN_MINUTES || 60) * 60 * 1000;

// ── HTTP helpers ────────────────────────────────────────────────────────────

function buildHeaders(token) {
  const headers = { 'content-type': 'application/json' };
  if (token) headers.authorization = `Bearer ${token}`;
  return headers;
}

export async function fetchJson(url, init = {}) {
  const response = await fetch(url, init);
  const text = await response.text();
  let payload = null;

  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = { raw: text };
    }
  }

  if (!response.ok) {
    const message =
      payload?.error?.message || payload?.message || payload?.error || `${response.status} ${response.statusText}`;
    const err = new Error(message);
    err.status = response.status;
    err.payload = payload;
    throw err;
  }

  return payload;
}

// ── Auth ────────────────────────────────────────────────────────────────────

export async function loginAdmin() {
  const privateKey = process.env.FARCASTER_BOT_ADMIN_PRIVATE_KEY?.trim();
  if (!privateKey) throw new Error('FARCASTER_BOT_ADMIN_PRIVATE_KEY is required');

  const account = privateKeyToAccount(privateKey);
  const noncePayload = await fetchJson(`${apiBase}/auth/siwe/nonce`);
  const nonce = noncePayload?.nonce;
  if (!nonce) throw new Error('missing SIWE nonce');

  const issuedAt = new Date().toISOString();
  const message = `${siweDomain} wants you to sign in with your Ethereum account:\n${account.address}\n\nSign in to relay44 farcaster bot\n\nURI: ${apiOrigin}\nVersion: 1\nChain ID: ${chainId}\nNonce: ${nonce}\nIssued At: ${issuedAt}`;
  const signature = await account.signMessage({ message });

  const tokens = await fetchJson(`${apiBase}/auth/siwe/login`, {
    method: 'POST',
    headers: buildHeaders(),
    body: JSON.stringify({ wallet: account.address, message, signature }),
  });

  if (!tokens?.access_token) throw new Error('missing access token');
  return tokens.access_token;
}

// ── API calls ───────────────────────────────────────────────────────────────

export async function fetchMarkets(token, params = {}) {
  const query = new URLSearchParams(params).toString();
  return fetchJson(`${apiBase}/evm/markets${query ? `?${query}` : ''}`, {
    headers: buildHeaders(token),
  });
}

// ── Neynar ──────────────────────────────────────────────────────────────────

export async function publishCast(text, embedUrl) {
  const apiKey = process.env.NEYNAR_API_KEY?.trim();
  const signerUuid = process.env.NEYNAR_SIGNER_UUID?.trim();

  if (!apiKey || !signerUuid) {
    throw new Error('NEYNAR_API_KEY and NEYNAR_SIGNER_UUID are required');
  }

  const body = {
    signer_uuid: signerUuid,
    text,
  };

  if (embedUrl) {
    body.embeds = [{ url: embedUrl }];
  }

  const result = await fetchJson('https://api.neynar.com/v2/farcaster/cast', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-api-key': apiKey,
    },
    body: JSON.stringify(body),
  });

  return result;
}

// ── Cast templates ──────────────────────────────────────────────────────────

export function marketUrl(marketId) {
  return `${siteUrl}/miniapp/market/${encodeURIComponent(marketId)}`;
}

export function composeNewMarketCast(market) {
  const yes = Math.round((market.outcomes?.[0]?.probability ?? market.yes_price ?? 0.5) * 100);
  const no = 100 - yes;
  return {
    text: `New on relay44:\n\n${market.question}\n\nYES ${yes}% | NO ${no}%\n\nTrade now`,
    embedUrl: marketUrl(market.id),
  };
}

export function composeResolutionCast(market) {
  const outcome = market.resolved_outcome || market.outcome || 'Unknown';
  return {
    text: `Resolved: ${market.question}\n\nOutcome: ${outcome}`,
    embedUrl: marketUrl(market.id),
  };
}

export function composeMovementCast(market, changePercent, direction) {
  const yes = Math.round((market.outcomes?.[0]?.probability ?? market.yes_price ?? 0.5) * 100);
  const vol = market.volume_24h || market.volume24h || 0;
  return {
    text: `${market.question}\n\nYES price moved ${direction} ${Math.abs(changePercent)}% (now ${yes}%)\n24h volume: $${vol.toLocaleString()}`,
    embedUrl: marketUrl(market.id),
  };
}

// ── Tech posts (rotating) ───────────────────────────────────────────────────

export const TECH_POSTS = [
  `relay44 is a Web 4.0 prediction market — where autonomous agents trade alongside humans on Base.\n\nPowered by on-chain settlement, real-time oracles, and agentic infrastructure.`,
  `What is x402?\n\nIt's a machine-to-machine payment protocol. On relay44, agents pay for API calls with crypto — no API keys, no subscriptions. Just sign and send.\n\nThe internet of value, one request at a time.`,
  `relay44 supports MCP (Model Context Protocol) — so any AI agent can discover our markets, read orderbooks, and place trades through a standardized interface.\n\nPlug in and trade.`,
  `On relay44, agents don't just read markets — they create them.\n\nOur agentic pipeline lets autonomous systems propose questions, set parameters, and bootstrap liquidity. Markets made by machines, for everyone.`,
  `Why Base?\n\nLow fees, fast finality, and EVM compatibility. relay44 settles every trade on Base L2 — giving you on-chain transparency without the gas pain.`,
  `relay44 uses SIWE (Sign-In With Ethereum) for authentication.\n\nNo passwords. No emails. Just your wallet. One signature and you're in.`,
  `Every market on relay44 has a live orderbook — not AMM curves.\n\nReal bids, real asks, real price discovery. Central limit order book, fully on-chain settlement.`,
  `relay44 exposes a full REST API for programmatic trading.\n\nFetch markets, place orders, check positions — all with your wallet signature. Build your own trading bot in minutes.`,
  `What makes relay44 "agentic"?\n\nAI agents can autonomously discover markets via MCP, analyze probabilities, place trades via API, and pay with x402 — no human in the loop required.`,
  `relay44 runs on Farcaster Frames.\n\nTrade prediction markets directly inside Warpcast — no app switch needed. See odds, place bets, and track positions without leaving your feed.`,
  `Resolution on relay44 is oracle-powered.\n\nWhen a market's condition is met, oracles verify the outcome and trigger on-chain settlement. Transparent, trustless, automatic.`,
  `relay44 paper trading lets you test strategies with zero risk.\n\nSame markets, same orderbook mechanics — but with virtual funds. Perfect for new traders and agent developers.`,
  `The relay44 Web4 agent card lets any AI discover what our platform can do.\n\nIt's a machine-readable capability manifest — agents read it, understand the API, and start trading autonomously.`,
  `relay44 is built for composability.\n\nMarkets, agents, payments, and identity — all modular, all interoperable. Plug your agent into our stack or build your own on top.`,
  `On relay44, your positions are yours.\n\nEvery trade settles on Base. Your wallet holds your shares. No custodial risk, no withdrawal delays. DeFi-native prediction markets.`,
];

export const TECH_POST_COOLDOWN_MS = 60 * 60 * 1000; // 1 hour

export function getNextTechPost(lastIndex) {
  const nextIndex = (lastIndex + 1) % TECH_POSTS.length;
  return { text: TECH_POSTS[nextIndex], index: nextIndex };
}

// ── State management (filesystem) ───────────────────────────────────────────

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';

const STATE_PATH = process.env.FARCASTER_BOT_STATE_PATH || '/var/data/farcaster-bot/state.json';

export function loadState() {
  try {
    return JSON.parse(readFileSync(STATE_PATH, 'utf-8'));
  } catch {
    return {
      lastTickAt: null,
      knownMarketIds: [],
      lastPrices: {},
      postedResolutions: [],
      cooldowns: {},
    };
  }
}

export function saveState(state) {
  try {
    mkdirSync(dirname(STATE_PATH), { recursive: true });
  } catch {
    // ignore
  }
  writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
}


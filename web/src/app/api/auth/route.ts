import { NextRequest, NextResponse } from 'next/server';
import { cookies } from 'next/headers';

const API_BASE =
  process.env.API_PROXY_TARGET || process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080/v1';
const IS_PRODUCTION = process.env.NODE_ENV === 'production';
const MAX_BODY_BYTES = 16 * 1024;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_MAX_REQUESTS = 30;
const RATE_LIMIT_BUCKETS = new Map<string, { count: number; resetAt: number }>();
const ALLOWED_ORIGINS = new Set(
  (process.env.AUTH_ALLOWED_ORIGINS || '')
    .split(',')
    .map((origin) => origin.trim())
    .filter(Boolean)
);

const REFRESH_TOKEN_COOKIE = 'relay44_refresh';
const COMPAT_REFRESH_TOKEN_COOKIES: string[] = [];
const COOKIE_MAX_AGE = 7 * 24 * 60 * 60;

function jsonError(status: number, error: string) {
  return NextResponse.json({ error }, { status });
}

function getClientIp(request: NextRequest): string {
  const forwardedFor = request.headers.get('x-forwarded-for');
  if (forwardedFor) {
    const firstIp = forwardedFor.split(',')[0]?.trim();
    if (firstIp) return firstIp;
  }

  const realIp = request.headers.get('x-real-ip')?.trim();
  if (realIp) return realIp;

  return 'unknown';
}

function cleanupRateLimitBuckets(now: number) {
  if (RATE_LIMIT_BUCKETS.size < 4_096) return;

  RATE_LIMIT_BUCKETS.forEach((bucket, key) => {
    if (bucket.resetAt <= now) {
      RATE_LIMIT_BUCKETS.delete(key);
    }
  });
}

function checkRateLimit(request: NextRequest): NextResponse | null {
  const now = Date.now();
  cleanupRateLimitBuckets(now);

  const key = getClientIp(request);
  const existing = RATE_LIMIT_BUCKETS.get(key);

  if (!existing || existing.resetAt <= now) {
    RATE_LIMIT_BUCKETS.set(key, { count: 1, resetAt: now + RATE_LIMIT_WINDOW_MS });
    return null;
  }

  if (existing.count >= RATE_LIMIT_MAX_REQUESTS) {
    const retryAfterSeconds = Math.max(1, Math.ceil((existing.resetAt - now) / 1000));
    const response = jsonError(429, 'Too many requests');
    response.headers.set('Retry-After', String(retryAfterSeconds));
    return response;
  }

  existing.count += 1;
  return null;
}

function buildExpectedOrigin(request: NextRequest): string | null {
  const host = request.headers.get('host');
  if (!host) return null;
  const protocol = request.headers.get('x-forwarded-proto') || 'https';
  return `${protocol}://${host}`;
}

function normalizeOrigin(value: string | null): string | null {
  if (!value) return null;
  try {
    return new URL(value).origin;
  } catch {
    return null;
  }
}

function isAllowedOrigin(request: NextRequest): boolean {
  if (!IS_PRODUCTION) return true;

  const allowed = new Set(ALLOWED_ORIGINS);
  const expectedOrigin = buildExpectedOrigin(request);
  if (expectedOrigin) allowed.add(expectedOrigin);

  const origin = normalizeOrigin(request.headers.get('origin'));
  if (origin) return allowed.has(origin);

  const refererOrigin = normalizeOrigin(request.headers.get('referer'));
  if (refererOrigin) return allowed.has(refererOrigin);

  return false;
}

function validateBodySize(request: NextRequest): NextResponse | null {
  const contentLength = Number(request.headers.get('content-length') || 0);
  if (Number.isFinite(contentLength) && contentLength > MAX_BODY_BYTES) {
    return jsonError(413, 'Request body too large');
  }
  return null;
}

function requireMutatingRequestGuards(request: NextRequest): NextResponse | null {
  const rateLimitResult = checkRateLimit(request);
  if (rateLimitResult) return rateLimitResult;

  if (!isAllowedOrigin(request)) {
    return jsonError(403, 'Forbidden origin');
  }

  return validateBodySize(request);
}

type LoginFlow = 'siwe' | 'solana' | 'farcaster';

type LoginRequestBody =
  | { wallet: string; signature: string; message: string; flow: 'siwe' | 'solana' }
  | { signature: string; message: string; nonce: string; flow: 'farcaster' };

interface UpstreamTokenPayload {
  accessToken?: unknown;
  refreshToken?: unknown;
  expiresAt?: unknown;
  access_token?: unknown;
  refresh_token?: unknown;
  expires_at?: unknown;
  expires_in?: unknown;
}

function parseNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return null;
}

function normalizeTokens(payload: UpstreamTokenPayload): {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
} | null {
  const accessToken =
    typeof payload.accessToken === 'string'
      ? payload.accessToken
      : typeof payload.access_token === 'string'
        ? payload.access_token
        : null;
  const refreshToken =
    typeof payload.refreshToken === 'string'
      ? payload.refreshToken
      : typeof payload.refresh_token === 'string'
        ? payload.refresh_token
        : null;

  const expiresAtRaw = parseNumber(payload.expiresAt) ?? parseNumber(payload.expires_at);
  const expiresInRaw = parseNumber(payload.expires_in);
  const expiresAt = expiresAtRaw
    ? (expiresAtRaw > 1_000_000_000_000 ? expiresAtRaw : expiresAtRaw * 1000)
    : expiresInRaw
      ? Date.now() + expiresInRaw * 1000
      : null;

  if (!accessToken || !refreshToken || !expiresAt) {
    return null;
  }

  return { accessToken, refreshToken, expiresAt };
}

function parseLoginRequestBody(bodyText: string): LoginRequestBody | null {
  try {
    const parsed = JSON.parse(bodyText) as Record<string, unknown>;

    const flow = (typeof parsed.flow === 'string' ? parsed.flow : 'siwe') as LoginFlow;

    if (flow === 'farcaster') {
      if (
        typeof parsed.signature !== 'string' ||
        typeof parsed.message !== 'string' ||
        typeof parsed.nonce !== 'string'
      ) {
        return null;
      }

      if (parsed.signature.length > 1_024 || parsed.message.length > 4_096 || parsed.nonce.length > 256) {
        return null;
      }

      return {
        signature: (parsed.signature as string).trim(),
        message: parsed.message as string,
        nonce: (parsed.nonce as string).trim(),
        flow: 'farcaster',
      };
    }

    if (typeof parsed.wallet !== 'string' || typeof parsed.signature !== 'string' || typeof parsed.message !== 'string') {
      return null;
    }

    if (
      (parsed.wallet as string).length > 96 ||
      (parsed.signature as string).length > 1_024 ||
      (parsed.message as string).length > 4_096
    ) {
      return null;
    }

    if (flow !== 'siwe' && flow !== 'solana') {
      return null;
    }

    return {
      wallet: (parsed.wallet as string).trim(),
      signature: (parsed.signature as string).trim(),
      message: parsed.message as string,
      flow,
    };
  } catch {
    return null;
  }
}

function getRefreshToken(cookieStore: Awaited<ReturnType<typeof cookies>>): string | undefined {
  const current = cookieStore.get(REFRESH_TOKEN_COOKIE)?.value;
  if (current) return current;

  for (const key of COMPAT_REFRESH_TOKEN_COOKIES) {
    const compat = cookieStore.get(key)?.value;
    if (compat) return compat;
  }

  return undefined;
}

function setRefreshTokenCookie(response: NextResponse, refreshToken: string) {
  response.cookies.set(REFRESH_TOKEN_COOKIE, refreshToken, {
    httpOnly: true,
    secure: IS_PRODUCTION,
    sameSite: 'strict',
    maxAge: COOKIE_MAX_AGE,
    path: '/',
  });
  for (const key of COMPAT_REFRESH_TOKEN_COOKIES) {
    response.cookies.delete(key);
  }
}

function clearRefreshTokenCookies(response: NextResponse) {
  response.cookies.delete(REFRESH_TOKEN_COOKIE);
  for (const key of COMPAT_REFRESH_TOKEN_COOKIES) {
    response.cookies.delete(key);
  }
}

export async function POST(request: NextRequest) {
  try {
    const guardError = requireMutatingRequestGuards(request);
    if (guardError) return guardError;

    const bodyText = await request.text();
    if (Buffer.byteLength(bodyText, 'utf8') > MAX_BODY_BYTES) {
      return jsonError(413, 'Request body too large');
    }

    const body = parseLoginRequestBody(bodyText);
    if (!body) {
      return jsonError(400, 'Invalid request body');
    }

    let target: string;
    let upstreamBody: string;

    if (body.flow === 'farcaster') {
      const { signature, message, nonce } = body;
      if (!signature || !message || !nonce) {
        return jsonError(400, 'Missing required fields');
      }
      target = `${API_BASE}/auth/farcaster/login`;
      upstreamBody = JSON.stringify({ signature, message, nonce });
    } else {
      const { wallet, signature, message } = body;
      if (!wallet || !signature || !message) {
        return jsonError(400, 'Missing required fields');
      }
      target = body.flow === 'solana'
        ? `${API_BASE}/auth/solana/login`
        : `${API_BASE}/auth/siwe/login`;
      upstreamBody = JSON.stringify({ wallet, signature, message });
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    const res = await fetch(target, {
      method: 'POST',
      headers,
      body: upstreamBody,
    });

    if (!res.ok) {
      const text = await res.text();
      return jsonError(res.status, text || 'Authentication failed');
    }

    const rawTokens = (await res.json()) as UpstreamTokenPayload;
    const tokens = normalizeTokens(rawTokens);
    if (!tokens) {
      console.error('Invalid upstream auth payload', rawTokens);
      return jsonError(502, 'Authentication payload was invalid');
    }

    const response = NextResponse.json({
      accessToken: tokens.accessToken,
      expiresAt: tokens.expiresAt,
    });
    setRefreshTokenCookie(response, tokens.refreshToken);
    return response;
  } catch (error) {
    console.error('Login error:', error);
    return jsonError(500, 'Internal server error');
  }
}

export async function PUT(request: NextRequest) {
  try {
    const guardError = requireMutatingRequestGuards(request);
    if (guardError) return guardError;

    const cookieStore = await cookies();
    const refreshToken = getRefreshToken(cookieStore);

    if (!refreshToken) {
      return jsonError(401, 'No refresh token');
    }

    const res = await fetch(`${API_BASE}/auth/refresh`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ refresh_token: refreshToken }),
    });

    if (!res.ok) {
      const response = jsonError(res.status, 'Token refresh failed');
      clearRefreshTokenCookies(response);
      return response;
    }

    const rawTokens = (await res.json()) as UpstreamTokenPayload;
    const tokens = normalizeTokens(rawTokens);
    if (!tokens) {
      console.error('Invalid upstream refresh payload', rawTokens);
      const response = jsonError(502, 'Refresh payload was invalid');
      clearRefreshTokenCookies(response);
      return response;
    }

    const response = NextResponse.json({
      accessToken: tokens.accessToken,
      expiresAt: tokens.expiresAt,
    });
    setRefreshTokenCookie(response, tokens.refreshToken);
    return response;
  } catch (error) {
    console.error('Refresh error:', error);
    return jsonError(500, 'Internal server error');
  }
}

export async function DELETE(request: NextRequest) {
  try {
    const guardError = requireMutatingRequestGuards(request);
    if (guardError) return guardError;

    const cookieStore = await cookies();
    const refreshToken = getRefreshToken(cookieStore);

    if (refreshToken) {
      await fetch(`${API_BASE}/auth/logout`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${refreshToken}`,
        },
      }).catch(() => {
      });
    }

    const response = NextResponse.json({ success: true });
    clearRefreshTokenCookies(response);
    return response;
  } catch (error) {
    console.error('Logout error:', error);
    const response = NextResponse.json({ success: true });
    clearRefreshTokenCookies(response);
    return response;
  }
}

export async function GET() {
  const cookieStore = await cookies();
  const hasRefreshToken = !!getRefreshToken(cookieStore);

  return NextResponse.json({ hasRefreshToken });
}

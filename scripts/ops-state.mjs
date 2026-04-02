#!/usr/bin/env node

import pg from "pg";

const { Client } = pg;

const ENSURE_TABLE_SQL = `
  create table if not exists ops_runner_state (
    runner_name text primary key,
    last_started_at timestamptz,
    last_succeeded_at timestamptz,
    last_failed_at timestamptz,
    last_status text not null default 'unknown',
    last_error_code text,
    last_error_message text,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
  );
`;

const RETRYABLE_MESSAGES = [
  "connection terminated unexpectedly",
  "terminating connection",
  "connection ended unexpectedly",
  "socket disconnected",
  "read etimedout",
  "connect etimedout",
  "econnreset",
  "ecconnreset",
  "could not connect",
];

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parsePositiveInt(value, fallback) {
  const parsed = Number.parseInt(String(value || "").trim(), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function buildConfig(connectionString) {
  const url = new URL(connectionString);
  const sslmode = (url.searchParams.get("sslmode") || "").trim().toLowerCase();
  const needsSsl =
    ["require", "verify-ca", "verify-full", "prefer"].includes(sslmode) ||
    /\.render\.com$/i.test(url.hostname);

  return needsSsl
    ? {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
        ssl: { rejectUnauthorized: false },
      }
    : {
        connectionString,
        connectionTimeoutMillis: 10_000,
        keepAlive: true,
      };
}

function isRetryableConnectionError(error) {
  const code = String(error?.code || "")
    .trim()
    .toLowerCase();
  const message = String(error?.message || "")
    .trim()
    .toLowerCase();

  if (["econnreset", "etimedout", "ehostunreach", "57p01"].includes(code)) {
    return true;
  }

  return RETRYABLE_MESSAGES.some((needle) => message.includes(needle));
}

async function safeEnd(client) {
  if (!client) {
    return;
  }

  try {
    await client.end();
  } catch {
    // Nothing useful to do here; this is cleanup after a failed connection.
  }
}

async function openClient(connectionString) {
  const client = new Client(buildConfig(connectionString));

  try {
    await client.connect();
    await client.query(ENSURE_TABLE_SQL);
    return client;
  } catch (error) {
    await safeEnd(client);
    throw error;
  }
}

async function connectWithRetries(connectionString, env) {
  const attempts = parsePositiveInt(env.OPS_STATE_CONNECT_ATTEMPTS, 4);
  const retryMs = parsePositiveInt(env.OPS_STATE_CONNECT_RETRY_MS, 750);
  let lastError = null;

  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await openClient(connectionString);
    } catch (error) {
      lastError = error;
      if (attempt === attempts || !isRetryableConnectionError(error)) {
        break;
      }

      await sleep(retryMs * attempt);
    }
  }

  const message = lastError?.message || "unknown error";
  const error = new Error(
    `ops_state connect failed after ${attempts} attempts: ${message}`,
  );
  error.cause = lastError;
  error.code = lastError?.code;
  throw error;
}

async function runQuery(state, sql, params = []) {
  if (!state) {
    return { rows: [] };
  }

  try {
    return await state.client.query(sql, params);
  } catch (error) {
    if (!isRetryableConnectionError(error)) {
      throw error;
    }

    await safeEnd(state.client);
    state.client = await connectWithRetries(state.connectionString, state.env);
    return state.client.query(sql, params);
  }
}

export async function connectOpsState(env = process.env) {
  const connectionString = String(env.DATABASE_URL || "").trim();
  if (!connectionString) {
    return null;
  }

  return {
    client: await connectWithRetries(connectionString, env),
    connectionString,
    env,
  };
}

export async function closeOpsState(state) {
  if (!state) {
    return;
  }

  await safeEnd(state.client);
}

export async function getRunnerState(state, runnerName) {
  if (!state) {
    return null;
  }

  const { rows } = await runQuery(
    state,
    `
      select
        runner_name,
        last_started_at,
        last_succeeded_at,
        last_failed_at,
        last_status,
        last_error_code,
        last_error_message,
        metadata,
        created_at,
        updated_at
      from ops_runner_state
      where runner_name = $1
    `,
    [runnerName],
  );

  return rows[0] || null;
}

export async function reportRunnerStarted(state, runnerName, metadata = {}) {
  if (!state) {
    return;
  }

  await runQuery(
    state,
    `
      insert into ops_runner_state (
        runner_name,
        last_started_at,
        last_status,
        metadata
      )
      values ($1, now(), 'running', $2::jsonb)
      on conflict (runner_name) do update
      set
        last_started_at = now(),
        last_status = 'running',
        metadata = $2::jsonb,
        updated_at = now()
    `,
    [runnerName, JSON.stringify(metadata)],
  );
}

export async function reportRunnerSuccess(state, runnerName, metadata = {}) {
  if (!state) {
    return;
  }

  await runQuery(
    state,
    `
      insert into ops_runner_state (
        runner_name,
        last_started_at,
        last_succeeded_at,
        last_status,
        last_error_code,
        last_error_message,
        metadata
      )
      values ($1, now(), now(), 'healthy', null, null, $2::jsonb)
      on conflict (runner_name) do update
      set
        last_started_at = now(),
        last_succeeded_at = now(),
        last_status = 'healthy',
        last_error_code = null,
        last_error_message = null,
        metadata = $2::jsonb,
        updated_at = now()
    `,
    [runnerName, JSON.stringify(metadata)],
  );
}

export async function reportRunnerFailure(
  state,
  runnerName,
  errorCode,
  errorMessage,
  metadata = {},
) {
  if (!state) {
    return;
  }

  await runQuery(
    state,
    `
      insert into ops_runner_state (
        runner_name,
        last_started_at,
        last_failed_at,
        last_status,
        last_error_code,
        last_error_message,
        metadata
      )
      values ($1, now(), now(), 'failed', $2, $3, $4::jsonb)
      on conflict (runner_name) do update
      set
        last_started_at = now(),
        last_failed_at = now(),
        last_status = 'failed',
        last_error_code = $2,
        last_error_message = $3,
        metadata = $4::jsonb,
        updated_at = now()
    `,
    [runnerName, errorCode, errorMessage, JSON.stringify(metadata)],
  );
}

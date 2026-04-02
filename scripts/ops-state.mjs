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

function buildConfig(connectionString) {
  const url = new URL(connectionString);
  const sslmode = (url.searchParams.get("sslmode") || "").trim().toLowerCase();
  const needsSsl = ["require", "verify-ca", "verify-full", "prefer"].includes(
    sslmode,
  );

  return needsSsl
    ? { connectionString, ssl: { rejectUnauthorized: false } }
    : { connectionString };
}

export async function connectOpsState(env = process.env) {
  const connectionString = String(env.DATABASE_URL || "").trim();
  if (!connectionString) {
    return null;
  }

  const client = new Client(buildConfig(connectionString));
  await client.connect();
  await client.query(ENSURE_TABLE_SQL);
  return client;
}

export async function closeOpsState(client) {
  if (!client) {
    return;
  }

  await client.end();
}

export async function getRunnerState(client, runnerName) {
  if (!client) {
    return null;
  }

  const { rows } = await client.query(
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

export async function reportRunnerStarted(client, runnerName, metadata = {}) {
  if (!client) {
    return;
  }

  await client.query(
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

export async function reportRunnerSuccess(client, runnerName, metadata = {}) {
  if (!client) {
    return;
  }

  await client.query(
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
  client,
  runnerName,
  errorCode,
  errorMessage,
  metadata = {},
) {
  if (!client) {
    return;
  }

  await client.query(
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

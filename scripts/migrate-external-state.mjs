#!/usr/bin/env node

import pg from 'pg';

const { Client } = pg;

const SOURCE_TABLES = [
  'external_credentials',
  'external_order_intents',
  'external_orders',
  'external_agents',
  'external_agent_runs',
];

const TRUNCATE_ORDER = [
  'external_agent_runs',
  'external_agents',
  'external_orders',
  'external_order_intents',
  'external_credentials',
];

const INSERT_ORDER = [...SOURCE_TABLES];

function readArg(name, fallback = '') {
  const prefix = `--${name}=`;
  const match = process.argv.find((entry) => entry.startsWith(prefix));
  return match ? match.slice(prefix.length) : fallback;
}

function readMode() {
  const mode = readArg('mode', 'counts').trim().toLowerCase();
  if (mode !== 'counts' && mode !== 'migrate') {
    throw new Error(`unsupported mode: ${mode}`);
  }
  return mode;
}

function requiredEnv(name) {
  const value = String(process.env[name] || '').trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function buildConfig(connectionString) {
  const url = new URL(connectionString);
  const sslmode = (url.searchParams.get('sslmode') || '').trim().toLowerCase();
  const needsSsl = ['require', 'verify-ca', 'verify-full', 'prefer'].includes(sslmode);
  return needsSsl
    ? { connectionString, ssl: { rejectUnauthorized: false } }
    : { connectionString };
}

function quoteIdent(value) {
  return `"${String(value).replaceAll('"', '""')}"`;
}

async function getColumns(client, table) {
  const { rows } = await client.query(
    `
      select column_name
      from information_schema.columns
      where table_schema = 'public' and table_name = $1
      order by ordinal_position
    `,
    [table],
  );
  return rows.map((row) => row.column_name);
}

async function getCounts(client) {
  const counts = {};
  for (const table of SOURCE_TABLES) {
    const { rows } = await client.query(`select count(*)::int as count from ${quoteIdent(table)}`);
    counts[table] = rows[0].count;
  }
  return counts;
}

async function loadRows(client, table, columns) {
  const selectList = columns.map(quoteIdent).join(', ');
  const { rows } = await client.query(`select ${selectList} from ${quoteIdent(table)}`);
  return rows;
}

async function insertRows(client, table, columns, rows) {
  if (rows.length === 0) {
    return 0;
  }

  const columnList = columns.map(quoteIdent).join(', ');
  const chunkSize = 100;

  for (let start = 0; start < rows.length; start += chunkSize) {
    const chunk = rows.slice(start, start + chunkSize);
    const values = [];
    const groups = chunk.map((row, rowIndex) => {
      const placeholders = columns.map((column, columnIndex) => {
        values.push(row[column]);
        return `$${rowIndex * columns.length + columnIndex + 1}`;
      });
      return `(${placeholders.join(', ')})`;
    });
    await client.query(
      `insert into ${quoteIdent(table)} (${columnList}) values ${groups.join(', ')}`,
      values,
    );
  }

  return rows.length;
}

async function main() {
  const mode = readMode();
  const source = new Client(buildConfig(requiredEnv('SRC_DATABASE_URL')));
  const target = new Client(buildConfig(requiredEnv('DST_DATABASE_URL')));

  await source.connect();
  await target.connect();

  try {
    const sourceCounts = await getCounts(source);
    const targetCountsBefore = await getCounts(target);

    if (mode === 'counts') {
      console.log(
        JSON.stringify(
          {
            mode,
            sourceCounts,
            targetCountsBefore,
          },
          null,
          2,
        ),
      );
      return;
    }

    const allowOverwrite = String(process.env.ALLOW_NON_EMPTY_DESTINATION || '')
      .trim()
      .toLowerCase();
    const destinationHasRows = Object.values(targetCountsBefore).some((count) => count > 0);
    if (destinationHasRows && !['1', 'true', 'yes', 'on'].includes(allowOverwrite)) {
      throw new Error('destination tables are not empty; set ALLOW_NON_EMPTY_DESTINATION=true to overwrite');
    }

    const tableData = new Map();
    for (const table of SOURCE_TABLES) {
      const columns = await getColumns(target, table);
      tableData.set(table, {
        columns,
        rows: await loadRows(source, table, columns),
      });
    }

    await target.query('begin');
    try {
      await target.query(
        `truncate table ${TRUNCATE_ORDER.map(quoteIdent).join(', ')} restart identity cascade`,
      );

      const inserted = {};
      for (const table of INSERT_ORDER) {
        const { columns, rows } = tableData.get(table);
        inserted[table] = await insertRows(target, table, columns, rows);
      }

      await target.query('commit');

      console.log(
        JSON.stringify(
          {
            mode,
            sourceCounts,
            targetCountsBefore,
            inserted,
            targetCountsAfter: await getCounts(target),
          },
          null,
          2,
        ),
      );
    } catch (error) {
      await target.query('rollback');
      throw error;
    }
  } finally {
    await Promise.allSettled([source.end(), target.end()]);
  }
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        ok: false,
        message: error instanceof Error ? error.message : String(error),
      },
      null,
      2,
    ),
  );
  process.exit(1);
});

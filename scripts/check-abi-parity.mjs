#!/usr/bin/env node
// Ensures the SDK's static ABI snapshot (sdk/agent/src/abis.ts) stays in
// sync with the canonical ABIs that power web/src/lib/contracts.ts — the
// source of truth served by the public /api/contracts/:name/abi endpoint.
//
// The check intentionally avoids running TypeScript: it parses the two
// TS files as source text, extracts the ABI literals by name, and compares
// the resulting JSON for structural equality. This lets us run the check
// from a plain `node` invocation with no build step.
//
// Usage: node scripts/check-abi-parity.mjs
// Exits 0 on parity, 1 on drift.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ROOT = resolve(__dirname, '..');

const WEB_FILE = resolve(ROOT, 'web/src/lib/contracts.ts');
const SDK_FILE = resolve(ROOT, 'sdk/agent/src/abis.ts');

// Constants we expect to exist in both files, byte-for-byte equivalent.
const SHARED_CONSTANTS = [
  'MARKET_CORE_ABI',
  'ORDER_BOOK_ABI',
  'ERC20_ABI',
  'RELAY_STAKING_ABI',
  'MARKET_CREATED_EVENT_ABI',
  'ORDER_PLACED_EVENT_ABI',
];

/**
 * Extract a TypeScript array-of-objects literal declared as:
 *   export const NAME = [ ... ] as const;
 * Returns the matching bracket contents (without surrounding `[...]`).
 */
function extractLiteral(source, name) {
  const marker = `export const ${name} = [`;
  const start = source.indexOf(marker);
  if (start === -1) {
    throw new Error(`Could not find \`export const ${name} = [\` in source`);
  }

  // Walk bracket depth to find the matching closing `]`.
  let depth = 0;
  let i = start + marker.length - 1; // position of the opening `[`
  for (; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === '[') depth += 1;
    else if (ch === ']') {
      depth -= 1;
      if (depth === 0) break;
    }
  }
  if (depth !== 0) {
    throw new Error(`Unbalanced brackets while reading ${name}`);
  }
  return source.slice(start + marker.length - 1, i + 1);
}

/**
 * Turn a TypeScript literal (with unquoted keys, trailing commas) into
 * equivalent JSON we can parse. Only handles the shape we actually use
 * for ABIs — object literals and string arrays.
 */
function tsLiteralToJson(ts) {
  // 1. Strip trailing commas before `]` or `}`.
  let cleaned = ts.replace(/,(\s*[}\]])/g, '$1');
  // 2. Strip `as const` suffix if present (shouldn't appear in slice, but safe).
  cleaned = cleaned.replace(/\bas const\b/g, '');
  // 3. Quote unquoted keys: `{ name: ... }` → `{ "name": ... }`.
  //    Keys in ABI literals are simple identifiers.
  cleaned = cleaned.replace(/([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:/g, '$1"$2":');
  // 4. Convert single-quoted string values to double-quoted.
  cleaned = cleaned.replace(/'([^'\\]*)'/g, (_m, inner) =>
    JSON.stringify(inner),
  );
  return cleaned;
}

function parseAbi(source, name) {
  const literal = extractLiteral(source, name);
  const json = tsLiteralToJson(literal);
  try {
    return JSON.parse(json);
  } catch (err) {
    const snippet = json.slice(0, 400);
    throw new Error(`Failed to parse ${name} as JSON: ${err.message}\n--- snippet ---\n${snippet}`);
  }
}

function deepEqual(a, b) {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i += 1) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }
  if (a && typeof a === 'object') {
    const keysA = Object.keys(a).sort();
    const keysB = Object.keys(b).sort();
    if (keysA.length !== keysB.length) return false;
    for (let i = 0; i < keysA.length; i += 1) {
      if (keysA[i] !== keysB[i]) return false;
      if (!deepEqual(a[keysA[i]], b[keysA[i]])) return false;
    }
    return true;
  }
  return false;
}

function main() {
  const webSource = readFileSync(WEB_FILE, 'utf8');
  const sdkSource = readFileSync(SDK_FILE, 'utf8');

  const failures = [];
  for (const name of SHARED_CONSTANTS) {
    let webAbi;
    let sdkAbi;
    try {
      webAbi = parseAbi(webSource, name);
    } catch (err) {
      failures.push(`  [WEB] ${name}: ${err.message}`);
      continue;
    }
    try {
      sdkAbi = parseAbi(sdkSource, name);
    } catch (err) {
      failures.push(`  [SDK] ${name}: ${err.message}`);
      continue;
    }
    if (!deepEqual(webAbi, sdkAbi)) {
      failures.push(
        `  [DRIFT] ${name}: sdk/agent/src/abis.ts does not match web/src/lib/contracts.ts`,
      );
    } else {
      process.stdout.write(`  [OK] ${name}\n`);
    }
  }

  if (failures.length > 0) {
    process.stderr.write(
      `\nABI parity check FAILED (${failures.length} issue(s)):\n`,
    );
    for (const line of failures) process.stderr.write(`${line}\n`);
    process.stderr.write(
      '\nUpdate sdk/agent/src/abis.ts so it matches web/src/lib/contracts.ts exactly.\n',
    );
    process.exit(1);
  }

  process.stdout.write('\nABI parity check passed ✓\n');
}

main();

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"

while true; do
  npm --prefix web run dev
  exit_code="$?"
  printf '[web-dev] next dev exited with code %s, restarting in 1s\n' "$exit_code" >&2
  sleep 1
done

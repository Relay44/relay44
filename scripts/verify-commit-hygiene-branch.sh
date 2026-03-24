#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

upstream_ref="$(git rev-parse --abbrev-ref --symbolic-full-name @{upstream} 2>/dev/null || true)"

if [[ -n "$upstream_ref" ]]; then
  ./scripts/verify-commit-hygiene.sh --history-range "${upstream_ref}..HEAD"
  exit 0
fi

./scripts/verify-commit-hygiene.sh --history-range HEAD

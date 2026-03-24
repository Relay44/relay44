#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

configured="$(git config --get core.hooksPath || true)"
if [[ "$configured" != ".githooks" ]]; then
  echo "core.hooksPath is not configured to .githooks"
  echo "Run: ./scripts/install-git-hooks.sh"
  exit 1
fi

missing=0
for hook in .githooks/pre-commit .githooks/pre-push .githooks/commit-msg; do
  if [[ ! -x "$hook" ]]; then
    echo "Hook missing or not executable: $hook"
    missing=1
  fi
done

if [[ $missing -ne 0 ]]; then
  exit 1
fi

echo "Git hooks verified (.githooks active)"

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ ! -d .githooks ]]; then
  echo "Missing .githooks directory"
  exit 1
fi

chmod +x .githooks/pre-commit .githooks/pre-push .githooks/commit-msg
chmod +x ./scripts/verify-no-internal-assets.sh
chmod +x ./scripts/verify-no-native-selects.sh

git config core.hooksPath .githooks

configured="$(git config --get core.hooksPath || true)"
if [[ "$configured" != ".githooks" ]]; then
  echo "Failed to set core.hooksPath"
  exit 1
fi

echo "Installed git hooks via core.hooksPath=.githooks"

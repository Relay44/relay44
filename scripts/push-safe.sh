#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

./scripts/verify-git-hooks.sh
SILO_CHECK_INCLUDE_DOCS=1 ./scripts/verify-project-silo.sh
node ./scripts/verify-open-core-boundary.mjs
./scripts/verify-no-native-selects.sh tracked

upstream_ref="$(git rev-parse --abbrev-ref --symbolic-full-name @{upstream} 2>/dev/null || true)"
if [[ -n "$upstream_ref" ]]; then
  ./scripts/verify-commit-hygiene.sh --history-range "${upstream_ref}..HEAD"
  ./scripts/verify-no-internal-assets.sh range "${upstream_ref}..HEAD"
else
  ./scripts/verify-commit-hygiene.sh --history-range "HEAD"
  ./scripts/verify-no-internal-assets.sh range "HEAD"
fi

git push "$@"

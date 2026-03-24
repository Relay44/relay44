#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FORBIDDEN_PATTERNS=(
  'github\.com/.*/singularity'
  'api\.singularity\.'
  'external\.singularity\.'
  'sign in to .*singularity'
  'singularity design tokens'
  'singularity-api-(staging|prod)'
  'singularity-web-(staging|prod)'
)

pattern="$(IFS='|'; echo "${FORBIDDEN_PATTERNS[*]}")"

if command -v rg >/dev/null 2>&1; then
  filters=(
    --hidden
    --glob
    '!.git/*'
    --glob
    '!node_modules/*'
    --glob
    '!target/*'
    --glob
    '!.next/*'
    --glob
    '!tmp/*'
    --glob
    '!vendor/singularity_dual_core/*'
    --glob
    '!scripts/verify-project-silo.sh'
  )
  if [[ "${SILO_CHECK_INCLUDE_DOCS:-0}" != "1" ]]; then
    filters+=(--glob '!docs/*')
  fi
  matches="$(rg "${filters[@]}" -n -i -e "$pattern" . || true)"
else
  grep_args=(
    -RInE
    "$pattern"
    .
    --exclude-dir=.git
    --exclude-dir=node_modules
    --exclude-dir=target
    --exclude-dir=.next
    --exclude-dir=tmp
    --exclude-dir=vendor/singularity_dual_core
    --exclude=verify-project-silo.sh
  )
  if [[ "${SILO_CHECK_INCLUDE_DOCS:-0}" != "1" ]]; then
    grep_args+=(--exclude-dir=docs)
  fi
  matches="$(grep "${grep_args[@]}" || true)"
fi

if [[ -n "$matches" ]]; then
  echo "Cross-project coupling detected. Remove legacy singularity references from relay44."
  echo "$matches"
  exit 1
fi

echo "Silo check passed: no legacy singularity references found in relay44."

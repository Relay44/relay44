#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BLOCKED_PREFIXES=(
  "docs/reports/"
  "docs/runbooks/"
  "docs/integrations/"
  "docs/internal/"
  "docs/private/"
)

BLOCKED_FILES=(
  ".render-workspace-lock.json"
  "docs/LAUNCH_COMMAND_CENTER.md"
  "docs/LAUNCH_ENV_CHECKLIST.md"
  "docs/DEPLOYMENT_PLAN.md"
  "docs/PRODUCTION_LAUNCH_PLAN.md"
  "docs/PRODUCTION_LOOP_GATES.md"
  "docs/PRODUCTION_READINESS_ASSESSMENT.md"
  "docs/PRODUCTION_ROADMAP.md"
  "docs/PRODUCTION_AUDIT.md"
  "docs/production-audit-full-system.md"
  "docs/production-audit-launch.md"
  "docs/SECURITY_ASSESSMENT.md"
  "docs/OPEN_PERMISSIONLESS_WEB4_LAUNCH_CLOSURE_STATUS.md"
  "docs/relay-token-integration-plan.md"
)

ALLOWED_EDGE_PATHS=(
  "edge/README.md"
  "edge/LICENSE"
  "edge/.gitignore"
)

ALLOWED_EDGE_PREFIXES=(
  "edge/interfaces/"
)

print_usage() {
  echo "Usage:"
  echo "  ./scripts/verify-no-internal-assets.sh staged"
  echo "  ./scripts/verify-no-internal-assets.sh range <git-range>"
  echo "  ./scripts/verify-no-internal-assets.sh tracked"
}

is_allowed_edge_path() {
  local path="$1"

  for allowed in "${ALLOWED_EDGE_PATHS[@]}"; do
    if [[ "$path" == "$allowed" ]]; then
      return 0
    fi
  done

  for prefix in "${ALLOWED_EDGE_PREFIXES[@]}"; do
    if [[ "$path" == "$prefix"* ]]; then
      return 0
    fi
  done

  return 1
}

mode="${1:-staged}"
if [[ "$mode" != "staged" && "$mode" != "range" && "$mode" != "tracked" ]]; then
  print_usage
  exit 1
fi

paths=()
if [[ "$mode" == "staged" ]]; then
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    paths+=("$line")
  done < <(git diff --cached --name-only)
elif [[ "$mode" == "range" ]]; then
  shift
  git_range="${1:-}"
  if [[ -z "$git_range" ]]; then
    echo "Missing git range for range mode"
    print_usage
    exit 1
  fi
  if [[ "$git_range" == "HEAD" ]]; then
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      paths+=("$line")
    done < <(git show --pretty='' --name-only HEAD)
  else
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      paths+=("$line")
    done < <(git diff --name-only "$git_range")
  fi
else
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    paths+=("$line")
  done < <(git ls-files)
fi

if [[ ${#paths[@]} -eq 0 ]]; then
  echo "Internal asset boundary check passed."
  exit 0
fi

violations=()
for p in "${paths[@]}"; do
  if [[ ! -e "$p" ]]; then
    continue
  fi

  for prefix in "${BLOCKED_PREFIXES[@]}"; do
    if [[ "$p" == "$prefix"* ]]; then
      violations+=("$p :: blocked private docs path")
      continue 2
    fi
  done

  for file in "${BLOCKED_FILES[@]}"; do
    if [[ "$p" == "$file" ]]; then
      violations+=("$p :: blocked private docs file")
      continue 2
    fi
  done

  if [[ "$p" == edge/* ]]; then
    if ! is_allowed_edge_path "$p"; then
      violations+=("$p :: private edge runtime code must not be tracked")
      continue
    fi
  fi
done

if [[ ${#violations[@]} -gt 0 ]]; then
  echo "Boundary check failed: private edge/internal assets detected in open repository."
  printf ' - %s\n' "${violations[@]}"
  exit 1
fi

echo "Internal asset boundary check passed."

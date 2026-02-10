#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -e "docs" ]]; then
  echo "Boundary check failed: root docs/ directory is not allowed in this repository."
  echo "Remove docs/ (or move it outside the repo) before committing or pushing."
  exit 1
fi

BLOCKED_PREFIXES=(
  "docs/reports/"
  "docs/runbooks/"
  "docs/integrations/"
  "docs/internal/"
  "docs/private/"
)

BLOCKED_FILES=(
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
  "docs/BASE_NATIVE_TOKEN_LAUNCH_PLAN.md"
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


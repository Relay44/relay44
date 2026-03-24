#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

print_usage() {
  echo "Usage:"
  echo "  ./scripts/verify-no-native-selects.sh staged"
  echo "  ./scripts/verify-no-native-selects.sh range <git-range>"
  echo "  ./scripts/verify-no-native-selects.sh tracked"
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
  done < <(git diff --cached --name-only -- 'web/src')
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
    done < <(git show --pretty='' --name-only HEAD -- 'web/src')
  else
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      paths+=("$line")
    done < <(git diff --name-only "$git_range" -- 'web/src')
  fi
else
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    paths+=("$line")
  done < <(git ls-files 'web/src')
fi

if [[ ${#paths[@]} -eq 0 ]]; then
  echo "Custom select check passed."
  exit 0
fi

matches=()
while IFS= read -r line; do
  [[ -z "$line" ]] && continue
  matches+=("$line")
done < <(rg -n '<select[[:space:]>]' --glob '*.{ts,tsx,js,jsx}' -- "${paths[@]}" || true)

if [[ ${#matches[@]} -gt 0 ]]; then
  echo "Custom select check failed: raw <select> is not allowed in web/src."
  printf ' - %s\n' "${matches[@]}"
  exit 1
fi

echo "Custom select check passed."

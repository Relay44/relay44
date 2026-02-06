#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ai_marker_pattern='(claude|anthropic|chatgpt|gpt[- ]?4|gpt[- ]?5|copilot|codex|cursor|gemini|perplexity|windsurf|lovable)'
subject_pattern='^[a-z0-9][a-z0-9 /:+._-]*$'

msg_file="${1:-}"

if [[ -z "$msg_file" || ! -f "$msg_file" ]]; then
  echo "Usage: $0 <commit-msg-file>"
  exit 1
fi

infer_subject() {
  local category=""
  local changed=0

  while IFS= read -r file; do
    [[ -n "$file" ]] || continue
    changed=1

    case "$file" in
      README.md)
        next="readme"
        ;;
      .github/*)
        next="ci"
        ;;
      app/*|migrations/*)
        next="backend"
        ;;
      web/*)
        next="web"
        ;;
      evm/*)
        next="contracts"
        ;;
      programs/*)
        next="solana programs"
        ;;
      services/x402-facilitator/*)
        next="x402"
        ;;
      services/*)
        next="services"
        ;;
      scripts/*|package.json|Cargo.toml)
        next="tooling"
        ;;
      render.yaml|docker/*)
        next="deployment"
        ;;
      *)
        next="project files"
        ;;
    esac

    if [[ -z "$category" ]]; then
      category="$next"
      continue
    fi

    if [[ "$category" != "$next" ]]; then
      category="project files"
      break
    fi
  done < <(git diff --cached --name-only --diff-filter=ACMRTUXB)

  if [[ $changed -eq 0 || -z "$category" ]]; then
    echo "update project files"
    return
  fi

  echo "update $category"
}

trim_subject() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[[:space:]]+/ /g; s/^[[:space:]]+//; s/[[:space:]]+$//; s/[.!?]+$//'
}

cleaned_file="$(mktemp)"
trap 'rm -f "$cleaned_file"' EXIT

awk '
  {
    line = $0
    lower = tolower(line)
    if (lower ~ /^[[:space:]]*co-authored-by:/) next
    if (lower ~ /^[[:space:]]*generated[[:space:]]+(with|by)/) next
    if (lower ~ /^[[:space:]]*(ai-generated|authored-by):/) next
    sub(/[[:space:]]+$/, "", line)
    print line
  }
' "$msg_file" > "$cleaned_file"

subject="$(sed -n '1p' "$cleaned_file" | tr -d '\r')"
body="$(sed '1d' "$cleaned_file" | tr -d '\r')"
subject="$(trim_subject "$subject")"

if [[ -z "$subject" || "$subject" =~ $ai_marker_pattern ]]; then
  subject="$(infer_subject)"
fi

if [[ ! "$subject" =~ $subject_pattern || ${#subject} -lt 4 || ${#subject} -gt 72 ]]; then
  subject="$(infer_subject)"
fi

{
  printf '%s\n' "$subject"
  if [[ -n "$body" ]]; then
    printf '\n%s\n' "$body"
  fi
} > "$msg_file"

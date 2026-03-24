#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ai_marker_pattern='(claude|anthropic|chatgpt|gpt[- ]?4|gpt[- ]?5|copilot|codex|cursor|gemini|perplexity|windsurf|lovable)'
trailer_pattern='^[[:space:]]*(co-authored-by:|generated (with|by)|ai-generated:|authored-by:)'
subject_pattern='^[a-z0-9][a-z0-9 /:+._-]*$'

check_staged=0
check_history_all=0
history_range=""
commit_msg_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --staged)
      check_staged=1
      shift
      ;;
    --history-all)
      check_history_all=1
      shift
      ;;
    --history-range)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --history-range"
        exit 1
      fi
      history_range="$2"
      shift 2
      ;;
    --commit-msg-file)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --commit-msg-file"
        exit 1
      fi
      commit_msg_file="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

if [[ $check_staged -eq 0 && $check_history_all -eq 0 && -z "$history_range" && -z "$commit_msg_file" ]]; then
  echo "No checks requested. Use one of: --staged | --history-all | --history-range <range> | --commit-msg-file <file>"
  exit 1
fi

subject_ok() {
  local subject="$1"

  [[ -n "$subject" ]] || return 1
  [[ ${#subject} -ge 4 && ${#subject} -le 72 ]] || return 1
  [[ "$subject" =~ $subject_pattern ]] || return 1
  [[ "$subject" != *"  "* ]] || return 1

  case "$subject" in
    *'.'|*'!'|*'?'|merge\ *|revert\ *)
      return 1
      ;;
  esac

  return 0
}

check_message_blob() {
  local label="$1"
  local subject="$2"
  local body="$3"
  local fail=0

  if ! subject_ok "$subject"; then
    echo "$label: invalid commit subject: $subject"
    fail=1
  fi

  if grep -Eiq "$ai_marker_pattern" <<<"$subject"$'\n'"$body"; then
    echo "$label: blocked AI marker found in commit message"
    fail=1
  fi

  if grep -Eiq "$trailer_pattern" <<<"$body"; then
    echo "$label: blocked attribution trailer found in commit message"
    fail=1
  fi

  return $fail
}

check_commit() {
  local commit="$1"
  local fail=0
  local author_name author_email committer_name committer_email subject body metadata

  author_name="$(git show -s --format='%an' "$commit")"
  author_email="$(git show -s --format='%ae' "$commit")"
  committer_name="$(git show -s --format='%cn' "$commit")"
  committer_email="$(git show -s --format='%ce' "$commit")"
  subject="$(git show -s --format='%s' "$commit")"
  body="$(git show -s --format='%b' "$commit")"
  metadata="$(printf '%s\n%s\n%s\n%s\n' "$author_name" "$author_email" "$committer_name" "$committer_email")"

  if grep -Eiq "$ai_marker_pattern" <<<"$metadata"; then
    echo "commit $commit: blocked AI identity in author or committer metadata"
    echo "  author: $author_name <$author_email>"
    echo "  committer: $committer_name <$committer_email>"
    fail=1
  fi

  if ! check_message_blob "commit $commit" "$subject" "$body"; then
    fail=1
  fi

  return $fail
}

commit_list_for_range() {
  local range="$1"
  local after=""

  if [[ "$range" == *".."* || "$range" == *"^!"* || "$range" == *"^@"* ]]; then
    if git rev-list --no-merges "$range" >/dev/null 2>&1; then
      git rev-list --no-merges "$range"
      return
    fi

    if [[ "$range" == *".."* ]]; then
      after="${range##*..}"
      if git rev-parse --verify "$after" >/dev/null 2>&1; then
        git rev-list --max-count=1 --no-merges "$after"
        return
      fi
    fi

    return 1
  fi

  git rev-list --max-count=1 --no-merges "$range"
}

fail=0

if [[ $check_staged -eq 1 ]]; then
  :
fi

if [[ -n "$commit_msg_file" ]]; then
  if [[ ! -f "$commit_msg_file" ]]; then
    echo "Commit message file not found: $commit_msg_file"
    exit 1
  fi

  subject="$(sed -n '1p' "$commit_msg_file" | tr -d '\r')"
  body="$(sed '1d' "$commit_msg_file" | tr -d '\r')"

  if ! check_message_blob "commit-msg" "$subject" "$body"; then
    fail=1
  fi
fi

if [[ $check_history_all -eq 1 ]]; then
  while IFS= read -r commit; do
    [[ -n "$commit" ]] || continue
    if ! check_commit "$commit"; then
      fail=1
    fi
  done < <(git rev-list --all --no-merges)
fi

if [[ -n "$history_range" ]]; then
  if commit_list_for_range "$history_range" >/dev/null 2>&1; then
    while IFS= read -r commit; do
      [[ -n "$commit" ]] || continue
      if ! check_commit "$commit"; then
        fail=1
      fi
    done < <(commit_list_for_range "$history_range")
  else
    echo "Invalid commit range: $history_range"
    exit 1
  fi
fi

if [[ $fail -ne 0 ]]; then
  exit 1
fi

echo "Commit hygiene check passed."

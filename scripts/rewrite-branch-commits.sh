#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

base_ref="${1:-origin/main}"
target_branch="${2:-}"
rewrite_name="${COMMIT_REWRITE_NAME:-relay44}"
rewrite_email="${COMMIT_REWRITE_EMAIL:-hello@relay44.com}"

if [[ -z "$target_branch" ]]; then
  target_branch="$(git branch --show-current)"
fi

if [[ -z "$target_branch" ]]; then
  echo "Cannot resolve target branch."
  exit 1
fi

if [[ "$target_branch" == "main" ]]; then
  echo "Refusing to rewrite main."
  exit 1
fi

dirty="$(git status --porcelain | grep -vE '^[ MARCUD?!]{2} \.render-workspace-lock\.json$' || true)"
if [[ -n "$dirty" ]]; then
  echo "Working tree must be clean before rewriting branch commits."
  exit 1
fi

if ! git rev-parse "$base_ref" >/dev/null 2>&1; then
  if [[ "$base_ref" == "origin/main" ]] && git rev-parse main >/dev/null 2>&1; then
    base_ref="main"
  else
    echo "Base ref not found: $base_ref"
    exit 1
  fi
fi

if ! git rev-parse "$target_branch" >/dev/null 2>&1; then
  echo "Target branch not found: $target_branch"
  exit 1
fi

merge_base="$(git merge-base "$base_ref" "$target_branch")"
mapfile -t commits < <(git rev-list --reverse --no-merges "${merge_base}..${target_branch}")

if [[ ${#commits[@]} -eq 0 ]]; then
  echo "No commits to rewrite."
  exit 0
fi

tmp_branch="rewrite/${target_branch////-}-$$"
git checkout -q -b "$tmp_branch" "$merge_base"

cleanup() {
  if git rev-parse --verify "$tmp_branch" >/dev/null 2>&1; then
    git checkout -q "$target_branch" 2>/dev/null || true
    git branch -D "$tmp_branch" >/dev/null 2>&1 || true
  fi
}

trap cleanup INT TERM

for commit in "${commits[@]}"; do
  git cherry-pick --no-commit "$commit" >/dev/null

  msg_file="$(mktemp)"
  git show -s --format='%s%n%n%b' "$commit" > "$msg_file"
  ./scripts/normalize-commit-message.sh "$msg_file"

  author_date="$(git show -s --format='%aI' "$commit")"
  committer_date="$(git show -s --format='%cI' "$commit")"

  GIT_AUTHOR_NAME="$rewrite_name" \
  GIT_AUTHOR_EMAIL="$rewrite_email" \
  GIT_COMMITTER_NAME="$rewrite_name" \
  GIT_COMMITTER_EMAIL="$rewrite_email" \
  GIT_AUTHOR_DATE="$author_date" \
  GIT_COMMITTER_DATE="$committer_date" \
  git commit -q --file "$msg_file"

  rm -f "$msg_file"
done

rewritten_head="$(git rev-parse HEAD)"
git checkout -q "$target_branch"
git branch -f "$target_branch" "$rewritten_head"
git reset -q --hard "$rewritten_head"
git branch -D "$tmp_branch" >/dev/null 2>&1 || true
trap - INT TERM

echo "Rewrote ${#commits[@]} commit(s) on $target_branch"
echo "Force-push required: git push --force-with-lease"

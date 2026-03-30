#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

required_files=(
  "README.md"
  "CHANGELOG.md"
  "CONTRIBUTING.md"
  "CODE_OF_CONDUCT.md"
  "GOVERNANCE.md"
  "MAINTAINERS.md"
  "RELEASING.md"
  "SECURITY.md"
  "SUPPORT.md"
  ".github/CODEOWNERS"
  ".github/PULL_REQUEST_TEMPLATE.md"
  ".github/ISSUE_TEMPLATE/bug_report.yml"
  ".github/ISSUE_TEMPLATE/feature_request.yml"
  ".github/ISSUE_TEMPLATE/documentation.yml"
  ".github/ISSUE_TEMPLATE/config.yml"
  ".github/workflows/ci.yml"
  ".github/workflows/workflow-lint.yml"
)

violations=()

for file in "${required_files[@]}"; do
  if [[ ! -s "$file" ]]; then
    violations+=("$file :: missing or empty")
  fi
done

check_contains() {
  local file="$1"
  local pattern="$2"
  local label="$3"

  if ! grep -Fq "$pattern" "$file"; then
    violations+=("$file :: missing $label")
  fi
}

check_contains "README.md" "CONTRIBUTING.md" "contributing reference"
check_contains "README.md" "MAINTAINERS.md" "maintainers reference"
check_contains "README.md" "RELEASING.md" "releasing reference"
check_contains "README.md" "CHANGELOG.md" "changelog reference"
check_contains "README.md" "SECURITY.md" "security reference"
check_contains "README.md" "SUPPORT.md" "support reference"
check_contains "CHANGELOG.md" "## Unreleased" "unreleased section"
check_contains "MAINTAINERS.md" "## Ownership Map" "ownership map"
check_contains "RELEASING.md" "## Tagging and GitHub Release" "tagging guidance"
check_contains ".github/CODEOWNERS" "/app/" "backend ownership"
check_contains ".github/CODEOWNERS" "/web/" "web ownership"
check_contains ".github/CODEOWNERS" "/evm/" "contract ownership"
check_contains ".github/CODEOWNERS" "/.github/" "repo metadata ownership"

if [[ ${#violations[@]} -gt 0 ]]; then
  echo "Public repo standards check failed."
  printf ' - %s\n' "${violations[@]}"
  exit 1
fi

echo "Public repo standards check passed."

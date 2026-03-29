#!/usr/bin/env bash
set -euo pipefail

MODE="staging"
STRICT=0
ALLOW_MISSING_SECRETS=0
REQUIRE_DX_SNAPSHOT=0
EXTRA_CONFIG_ARGS=()
EXTRA_PRODUCTION_ARGS=()

while (($#)); do
  case "$1" in
    --mode=*)
      MODE="${1#*=}"
      ;;
    --mode)
      MODE="${2:-}"
      shift
      ;;
    --strict)
      STRICT=1
      ;;
    --allow-missing-secrets)
      ALLOW_MISSING_SECRETS=1
      ;;
    --require-dx-snapshot)
      REQUIRE_DX_SNAPSHOT=1
      ;;
    *)
      EXTRA_PRODUCTION_ARGS+=("$1")
      ;;
  esac
  shift
done

if [[ "$MODE" != "staging" && "$MODE" != "production" ]]; then
  echo "Invalid mode: $MODE" >&2
  exit 1
fi

if ((ALLOW_MISSING_SECRETS)); then
  EXTRA_CONFIG_ARGS+=("--allow-missing-secrets")
fi

if ((STRICT)); then
  EXTRA_PRODUCTION_ARGS+=("--strict")
fi

ENVIRONMENT="$MODE"
if [[ "$ENVIRONMENT" == "staging" ]]; then
  ENVIRONMENT="staging"
else
  ENVIRONMENT="production"
fi

echo "==> validating launch config ($MODE)"
node scripts/validate-launch-config.mjs --mode="$MODE" --write-report "${EXTRA_CONFIG_ARGS[@]}"

echo "==> validating address manifest ($ENVIRONMENT)"
node scripts/validate-address-manifest.mjs --environment="$ENVIRONMENT" --write-report

if command -v forge >/dev/null 2>&1; then
  echo "==> running evm tests"
  forge test --root evm
elif ((STRICT)); then
  echo "forge is required for strict launch readiness" >&2
  exit 1
else
  echo "==> skipping evm tests (forge not installed)"
fi

echo "==> running backend tests"
cargo test --manifest-path app/Cargo.toml --release

echo "==> running web lint"
npm --prefix web run lint

echo "==> running web build"
npm --prefix web run build

if [[ "$MODE" == "production" ]]; then
  echo "==> verifying onchain deployment"
  node scripts/verify-base-mainnet-deployment.mjs
fi

if [[ -n "${BASE_URL:-}" ]]; then
  echo "==> checking production loop ($BASE_URL)"
  node scripts/production-loop-report.mjs "${EXTRA_PRODUCTION_ARGS[@]}"
fi

if ((REQUIRE_DX_SNAPSHOT)); then
  echo "==> dx snapshot requirement requested"
  if command -v npm >/dev/null 2>&1; then
    npm run dx:snapshot
  fi
fi

echo "launch readiness passed"

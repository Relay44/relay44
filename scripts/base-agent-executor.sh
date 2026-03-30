#!/usr/bin/env bash
set -euo pipefail

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      export BASE_AGENT_OPERATOR_DRY_RUN=true
      shift
      ;;
    --network)
      case "${2:-}" in
        sepolia)
          export BASE_CHAIN_ID=84532
          ;;
        mainnet)
          export BASE_CHAIN_ID=8453
          ;;
      esac
      shift 2
      ;;
    --network=sepolia)
      export BASE_CHAIN_ID=84532
      shift
      ;;
    --network=mainnet)
      export BASE_CHAIN_ID=8453
      shift
      ;;
    *)
      shift
      ;;
  esac
done

exec node scripts/base-agent-operator.mjs

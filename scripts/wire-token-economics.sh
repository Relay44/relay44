#!/usr/bin/env bash
set -euo pipefail

# Wire token economics on deployed contracts.
# Requires: BASE_DEPLOYER_KEY env var (admin private key)
# Run once after deployment to connect staking ↔ orderbook and disable broken execution fee.

RPC_URL="${BASE_RPC_URL:-https://mainnet.base.org}"
PRIVATE_KEY="${BASE_DEPLOYER_KEY:?BASE_DEPLOYER_KEY required}"

ORDER_BOOK="0xD84a495398c2Fec40a03B3D60D78A251058fE66b"
AGENT_RUNTIME="0xC44d686548513FF2a921201fa0811B1f30AA1a65"
RELAY_STAKING="0x709D6006f026950b531d4883260c8416650c5AB7"

echo "==> OrderBook.setStakingContract(${RELAY_STAKING})"
cast send "$ORDER_BOOK" "setStakingContract(address)" "$RELAY_STAKING" \
  --rpc-url "$RPC_URL" \
  --private-key "$PRIVATE_KEY" \
  --slow

echo "==> AgentRuntime.setExecutionFee(0)"
cast send "$AGENT_RUNTIME" "setExecutionFee(uint256)" 0 \
  --rpc-url "$RPC_URL" \
  --private-key "$PRIVATE_KEY" \
  --slow

echo "done"

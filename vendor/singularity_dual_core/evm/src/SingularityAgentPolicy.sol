// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";

contract SingularityAgentPolicy is RoleAuth {
    error PolicyViolation(bytes32 code);

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    struct Policy {
        bool enabled;
        uint128 maxOrderQuantity;
        uint128 maxOrderNotional;
        uint32 maxOpenOrders;
    }

    event PolicySet(
        address indexed agent,
        bool enabled,
        uint128 maxOrderQuantity,
        uint128 maxOrderNotional,
        uint32 maxOpenOrders
    );

    mapping(address => Policy) public policies;

    constructor(address admin) RoleAuth(admin) {}

    function setPolicy(address agent, Policy calldata policy) external onlyRole(OPERATOR_ROLE) {
        if (agent == address(0)) revert InvalidAddress();
        policies[agent] = policy;
        emit PolicySet(
            agent,
            policy.enabled,
            policy.maxOrderQuantity,
            policy.maxOrderNotional,
            policy.maxOpenOrders
        );
    }

    function clearPolicy(address agent) external onlyRole(OPERATOR_ROLE) {
        if (agent == address(0)) revert InvalidAddress();
        delete policies[agent];
        emit PolicySet(agent, false, 0, 0, 0);
    }

    function enforceOrder(address agent, uint256 quantity, uint256 notional, uint256 openOrders) external view {
        Policy memory policy = policies[agent];
        if (!policy.enabled) return;

        if (policy.maxOrderQuantity > 0 && quantity > policy.maxOrderQuantity) {
            revert PolicyViolation("MAX_ORDER_QTY");
        }
        if (policy.maxOrderNotional > 0 && notional > policy.maxOrderNotional) {
            revert PolicyViolation("MAX_ORDER_NOTIONAL");
        }
        if (policy.maxOpenOrders > 0 && openOrders >= policy.maxOpenOrders) {
            revert PolicyViolation("MAX_OPEN_ORDERS");
        }
    }
}

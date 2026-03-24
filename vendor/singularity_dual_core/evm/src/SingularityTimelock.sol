// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";

contract SingularityTimelock is RoleAuth {
    error OperationNotReady();
    error OperationUnknown();
    error DelayTooShort();
    error CallFailed();

    bytes32 public constant PROPOSER_ROLE = keccak256("PROPOSER_ROLE");
    bytes32 public constant EXECUTOR_ROLE = keccak256("EXECUTOR_ROLE");

    uint256 public minDelay;
    mapping(bytes32 => uint256) public operationEta;

    event OperationScheduled(bytes32 indexed operationId, address indexed target, uint256 value, uint256 executeAt);
    event OperationCancelled(bytes32 indexed operationId);
    event OperationExecuted(bytes32 indexed operationId, address indexed target, uint256 value);
    event MinDelayUpdated(uint256 previousDelay, uint256 newDelay);

    constructor(address admin, uint256 minDelaySeconds) RoleAuth(admin) {
        minDelay = minDelaySeconds;
        _grantRole(PROPOSER_ROLE, admin);
        _grantRole(EXECUTOR_ROLE, admin);
    }

    function hashOperation(address target, uint256 value, bytes calldata data, bytes32 salt)
        public
        pure
        returns (bytes32)
    {
        return keccak256(abi.encode(target, value, data, salt));
    }

    function schedule(address target, uint256 value, bytes calldata data, bytes32 salt, uint256 delay)
        external
        onlyRole(PROPOSER_ROLE)
        returns (bytes32 operationId)
    {
        if (delay < minDelay) revert DelayTooShort();
        operationId = hashOperation(target, value, data, salt);
        uint256 executeAt = block.timestamp + delay;
        operationEta[operationId] = executeAt;
        emit OperationScheduled(operationId, target, value, executeAt);
    }

    function cancel(bytes32 operationId) external onlyRole(PROPOSER_ROLE) {
        if (operationEta[operationId] == 0) revert OperationUnknown();
        delete operationEta[operationId];
        emit OperationCancelled(operationId);
    }

    function execute(address target, uint256 value, bytes calldata data, bytes32 salt)
        external
        payable
        onlyRole(EXECUTOR_ROLE)
        returns (bytes memory)
    {
        bytes32 operationId = hashOperation(target, value, data, salt);
        uint256 eta = operationEta[operationId];
        if (eta == 0) revert OperationUnknown();
        if (block.timestamp < eta) revert OperationNotReady();

        delete operationEta[operationId];
        (bool ok, bytes memory result) = target.call{ value: value }(data);
        if (!ok) revert CallFailed();
        emit OperationExecuted(operationId, target, value);
        return result;
    }

    function setMinDelay(uint256 newDelay) external onlyRole(DEFAULT_ADMIN_ROLE) {
        uint256 previousDelay = minDelay;
        minDelay = newDelay;
        emit MinDelayUpdated(previousDelay, newDelay);
    }
}

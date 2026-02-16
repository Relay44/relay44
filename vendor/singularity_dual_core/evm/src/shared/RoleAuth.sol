// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

abstract contract RoleAuth {
    error Unauthorized();
    error InvalidAddress();

    event RoleGranted(bytes32 indexed role, address indexed account, address indexed grantedBy);
    event RoleRevoked(bytes32 indexed role, address indexed account, address indexed revokedBy);

    bytes32 public constant DEFAULT_ADMIN_ROLE = 0x00;

    mapping(bytes32 => mapping(address => bool)) private roles;

    constructor(address admin) {
        if (admin == address(0)) revert InvalidAddress();
        roles[DEFAULT_ADMIN_ROLE][admin] = true;
        emit RoleGranted(DEFAULT_ADMIN_ROLE, admin, msg.sender);
    }

    modifier onlyRole(bytes32 role) {
        if (!roles[role][msg.sender]) revert Unauthorized();
        _;
    }

    function hasRole(bytes32 role, address account) public view returns (bool) {
        return roles[role][account];
    }

    function grantRole(bytes32 role, address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        _grantRole(role, account);
    }

    function revokeRole(bytes32 role, address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (account == address(0)) revert InvalidAddress();
        if (!roles[role][account]) return;
        roles[role][account] = false;
        emit RoleRevoked(role, account, msg.sender);
    }

    function _grantRole(bytes32 role, address account) internal {
        if (account == address(0)) revert InvalidAddress();
        if (roles[role][account]) return;
        roles[role][account] = true;
        emit RoleGranted(role, account, msg.sender);
    }
}

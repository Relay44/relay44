// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";

interface IAccessControlLike {
    function grantRole(bytes32 role, address account) external;
    function revokeRole(bytes32 role, address account) external;
    function hasRole(bytes32 role, address account) external view returns (bool);
}

contract HandoffToTimelockScript is Script {
    bytes32 internal constant DEFAULT_ADMIN_ROLE = bytes32(0);

    error ZeroAddress(string field);

    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address timelock = vm.envAddress("TIMELOCK_ADDRESS");
        address marketCore = vm.envAddress("MARKET_CORE_ADDRESS");
        address orderBook = vm.envAddress("ORDER_BOOK_ADDRESS");
        address collateralVault = vm.envAddress("COLLATERAL_VAULT_ADDRESS");
        address agentRuntime = vm.envAddress("AGENT_RUNTIME_ADDRESS");

        if (admin == address(0)) revert ZeroAddress("BASE_ADMIN");
        if (timelock == address(0)) revert ZeroAddress("TIMELOCK_ADDRESS");
        if (marketCore == address(0)) revert ZeroAddress("MARKET_CORE_ADDRESS");
        if (orderBook == address(0)) revert ZeroAddress("ORDER_BOOK_ADDRESS");
        if (collateralVault == address(0)) revert ZeroAddress("COLLATERAL_VAULT_ADDRESS");
        if (agentRuntime == address(0)) revert ZeroAddress("AGENT_RUNTIME_ADDRESS");

        vm.startBroadcast();
        _handoffAdmin(IAccessControlLike(marketCore), admin, timelock);
        _handoffAdmin(IAccessControlLike(orderBook), admin, timelock);
        _handoffAdmin(IAccessControlLike(collateralVault), admin, timelock);
        _handoffAdmin(IAccessControlLike(agentRuntime), admin, timelock);
        vm.stopBroadcast();

        console2.log("timelock handoff completed");
        console2.log("marketCore:", marketCore);
        console2.log("orderBook:", orderBook);
        console2.log("collateralVault:", collateralVault);
        console2.log("agentRuntime:", agentRuntime);
        console2.log("timelock:", timelock);
    }

    function _handoffAdmin(IAccessControlLike target, address admin, address timelock) internal {
        if (!target.hasRole(DEFAULT_ADMIN_ROLE, timelock)) {
            target.grantRole(DEFAULT_ADMIN_ROLE, timelock);
        }
        if (target.hasRole(DEFAULT_ADMIN_ROLE, admin)) {
            target.revokeRole(DEFAULT_ADMIN_ROLE, admin);
        }
    }
}

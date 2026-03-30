// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";

interface IAccessControlLike {
    function grantRole(bytes32 role, address account) external;
    function revokeRole(bytes32 role, address account) external;
    function hasRole(bytes32 role, address account) external view returns (bool);
}

interface IAgentRuntimeLike {
    function identityRegistry() external view returns (address);
}

contract RotateAgentRuntimeScript is Script {
    error MissingAdmin();
    error MissingOrderBook();
    error MissingCurrentRuntime();
    error MissingIdentityRegistry();

    function run() external returns (AgentRuntime newRuntime) {
        address admin = vm.envAddress("BASE_ADMIN");
        address orderBookAddress = vm.envAddress("ORDER_BOOK_ADDRESS");
        address currentRuntimeAddress = vm.envAddress("AGENT_RUNTIME_ADDRESS");

        if (admin == address(0)) revert MissingAdmin();
        if (orderBookAddress == address(0)) revert MissingOrderBook();
        if (currentRuntimeAddress == address(0)) revert MissingCurrentRuntime();

        address identityRegistryAddress = IAgentRuntimeLike(currentRuntimeAddress).identityRegistry();
        if (identityRegistryAddress == address(0)) revert MissingIdentityRegistry();

        vm.startBroadcast();

        newRuntime = new AgentRuntime(admin, orderBookAddress);
        newRuntime.setIdentityRegistry(identityRegistryAddress);

        OrderBook orderBook = OrderBook(orderBookAddress);
        AgentIdentityRegistry identityRegistry = AgentIdentityRegistry(identityRegistryAddress);

        _grantRoleIfMissing(
            IAccessControlLike(address(orderBook)), orderBook.AGENT_RUNTIME_ROLE(), address(newRuntime)
        );
        _grantRoleIfMissing(
            IAccessControlLike(address(identityRegistry)), identityRegistry.REGISTRAR_ROLE(), address(newRuntime)
        );
        _revokeRoleIfPresent(
            IAccessControlLike(address(orderBook)), orderBook.AGENT_RUNTIME_ROLE(), currentRuntimeAddress
        );
        _revokeRoleIfPresent(
            IAccessControlLike(address(identityRegistry)), identityRegistry.REGISTRAR_ROLE(), currentRuntimeAddress
        );

        vm.stopBroadcast();

        console2.log("admin:", admin);
        console2.log("orderBook:", orderBookAddress);
        console2.log("identityRegistry:", identityRegistryAddress);
        console2.log("oldRuntime:", currentRuntimeAddress);
        console2.log("newRuntime:", address(newRuntime));
    }

    function _grantRoleIfMissing(IAccessControlLike target, bytes32 role, address account) internal {
        if (!target.hasRole(role, account)) {
            target.grantRole(role, account);
        }
    }

    function _revokeRoleIfPresent(IAccessControlLike target, bytes32 role, address account) internal {
        if (target.hasRole(role, account)) {
            target.revokeRole(role, account);
        }
    }
}

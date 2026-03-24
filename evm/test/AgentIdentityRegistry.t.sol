// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";

contract AgentIdentityRegistryTest is Test {
    address internal admin = makeAddr("admin");
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");
    address internal registrar = makeAddr("registrar");

    AgentIdentityRegistry internal registry;

    function setUp() external {
        registry = new AgentIdentityRegistry(admin);

        vm.startPrank(admin);
        registry.grantRole(registry.REGISTRAR_ROLE(), registrar);
        vm.stopPrank();
    }

    function test_registerSelf() external {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent/alice");

        assertEq(agentId, 1);
        assertEq(registry.ownerOf(agentId), alice);
        assertEq(registry.tokenURI(agentId), "ipfs://agent/alice");
    }

    function test_registerForByRegistrar() external {
        vm.prank(registrar);
        uint256 agentId = registry.registerFor(bob, "ipfs://agent/bob");

        assertEq(agentId, 1);
        assertEq(registry.ownerOf(agentId), bob);
    }

    function test_metadataSetAndGetByOwner() external {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent/alice");

        vm.prank(alice);
        registry.setMetadata(agentId, "strategy", bytes("mean-reversion"));

        bytes memory value = registry.getMetadata(agentId, "strategy");
        assertEq(string(value), "mean-reversion");
    }

    function test_nonOwnerCannotSetMetadata() external {
        vm.prank(alice);
        uint256 agentId = registry.register("ipfs://agent/alice");

        vm.prank(bob);
        vm.expectRevert(AgentIdentityRegistry.NotAuthorized.selector);
        registry.setMetadata(agentId, "strategy", bytes("sniper"));
    }
}

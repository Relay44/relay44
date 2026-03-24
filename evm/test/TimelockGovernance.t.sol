// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {TimelockController} from "openzeppelin-contracts/contracts/governance/TimelockController.sol";
import {MarketCore} from "../src/MarketCore.sol";

contract TimelockGovernanceTest is Test {
    address internal admin = makeAddr("admin");
    address internal proposer = makeAddr("proposer");
    address internal executor = makeAddr("executor");

    MarketCore internal marketCore;
    TimelockController internal timelock;

    uint256 internal constant MIN_DELAY = 1 days;

    function setUp() external {
        marketCore = new MarketCore(admin);

        address[] memory proposers = new address[](1);
        proposers[0] = proposer;
        address[] memory executors = new address[](1);
        executors[0] = executor;

        timelock = new TimelockController(MIN_DELAY, proposers, executors, admin);

        vm.startPrank(admin);
        marketCore.grantRole(marketCore.DEFAULT_ADMIN_ROLE(), address(timelock));
        marketCore.grantRole(marketCore.PAUSER_ROLE(), address(timelock));
        marketCore.revokeRole(marketCore.PAUSER_ROLE(), admin);
        marketCore.revokeRole(marketCore.DEFAULT_ADMIN_ROLE(), admin);
        vm.stopPrank();
    }

    function test_timelockControlsAdminOperations() external {
        vm.prank(admin);
        vm.expectRevert();
        marketCore.pause();

        bytes memory callData = abi.encodeCall(MarketCore.pause, ());
        bytes32 salt = keccak256("market-core-pause");

        vm.prank(proposer);
        timelock.schedule(address(marketCore), 0, callData, bytes32(0), salt, MIN_DELAY);

        vm.prank(executor);
        vm.expectRevert();
        timelock.execute(address(marketCore), 0, callData, bytes32(0), salt);

        vm.warp(block.timestamp + MIN_DELAY + 1);

        vm.prank(executor);
        timelock.execute(address(marketCore), 0, callData, bytes32(0), salt);

        assertTrue(marketCore.paused());
    }
}

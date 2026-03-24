// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

contract CollateralVaultTest is Test {
    address internal admin = makeAddr("admin");
    address internal operator = makeAddr("operator");
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");

    MockERC20 internal usdc;
    CollateralVault internal vault;

    function setUp() external {
        usdc = new MockERC20("USD Coin", "USDC");
        vault = new CollateralVault(admin, address(usdc));

        bytes32 operatorRole = vault.OPERATOR_ROLE();
        vm.prank(admin);
        vault.grantRole(operatorRole, operator);

        usdc.mint(alice, 1_000e6);
        usdc.mint(bob, 1_000e6);

        vm.prank(alice);
        usdc.approve(address(vault), type(uint256).max);

        vm.prank(bob);
        usdc.approve(address(vault), type(uint256).max);
    }

    function test_depositAndWithdraw() external {
        vm.prank(alice);
        vault.deposit(100e6);

        assertEq(vault.availableBalance(alice), 100e6);
        assertEq(usdc.balanceOf(address(vault)), 100e6);

        vm.prank(alice);
        vault.withdraw(40e6);

        assertEq(vault.availableBalance(alice), 60e6);
        assertEq(usdc.balanceOf(alice), 940e6);
    }

    function test_lockAndUnlock() external {
        vm.prank(alice);
        vault.deposit(200e6);

        vm.prank(operator);
        vault.lock(alice, 150e6);

        assertEq(vault.availableBalance(alice), 50e6);
        assertEq(vault.lockedBalance(alice), 150e6);

        vm.prank(operator);
        vault.unlock(alice, 70e6);

        assertEq(vault.availableBalance(alice), 120e6);
        assertEq(vault.lockedBalance(alice), 80e6);
    }

    function test_settleMovesLockedBalance() external {
        vm.prank(alice);
        vault.deposit(300e6);

        vm.prank(operator);
        vault.lock(alice, 180e6);

        vm.prank(operator);
        vault.settle(alice, bob, 120e6);

        assertEq(vault.lockedBalance(alice), 60e6);
        assertEq(vault.availableBalance(bob), 120e6);
    }

    function test_transferAvailableMovesBalance() external {
        vm.prank(alice);
        vault.deposit(150e6);

        vm.prank(operator);
        vault.transferAvailable(alice, bob, 55e6);

        assertEq(vault.availableBalance(alice), 95e6);
        assertEq(vault.availableBalance(bob), 55e6);
    }

    function test_onlyOperatorCanLock() external {
        vm.prank(alice);
        vault.deposit(100e6);

        vm.prank(alice);
        vm.expectRevert();
        vault.lock(alice, 10e6);
    }

    function test_withdrawFailsWhenInsufficientAvailable() external {
        vm.prank(alice);
        vault.deposit(25e6);

        vm.prank(alice);
        vm.expectRevert(CollateralVault.InsufficientAvailable.selector);
        vault.withdraw(30e6);
    }

    function test_pauseBlocksStateChanges() external {
        vm.prank(admin);
        vault.pause();

        vm.prank(alice);
        vm.expectRevert();
        vault.deposit(1e6);
    }
}

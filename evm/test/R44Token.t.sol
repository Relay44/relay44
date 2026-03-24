// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {R44Token} from "../src/R44Token.sol";

contract R44TokenTest is Test {
    address internal admin = makeAddr("admin");
    address internal treasury = makeAddr("treasury");
    address internal user = makeAddr("user");

    uint256 internal constant CAP = 1_000_000e18;
    uint256 internal constant INITIAL_SUPPLY = 200_000e18;

    R44Token internal token;

    function setUp() external {
        token = new R44Token("Relay44", "R44", CAP, admin, treasury, INITIAL_SUPPLY);
    }

    function test_initialConfig() external view {
        assertEq(token.totalSupply(), INITIAL_SUPPLY);
        assertEq(token.balanceOf(treasury), INITIAL_SUPPLY);
        assertEq(token.cap(), CAP);
    }

    function test_onlyMinterCanMint() external {
        vm.prank(user);
        vm.expectRevert();
        token.mint(user, 1e18);

        vm.prank(admin);
        token.mint(user, 1e18);
        assertEq(token.balanceOf(user), 1e18);
    }

    function test_mintCannotExceedCap() external {
        vm.startPrank(admin);
        token.mint(user, CAP - INITIAL_SUPPLY);

        vm.expectRevert();
        token.mint(user, 1);
        vm.stopPrank();
    }

    function test_pauseBlocksTransfers() external {
        vm.prank(admin);
        token.pause();

        vm.prank(treasury);
        vm.expectRevert();
        token.transfer(user, 1e18);

        vm.prank(admin);
        token.unpause();

        vm.prank(treasury);
        token.transfer(user, 1e18);
        assertEq(token.balanceOf(user), 1e18);
    }

    function test_initialSupplyCannotExceedCap() external {
        vm.expectRevert(R44Token.InitialSupplyExceedsCap.selector);
        new R44Token("Relay44", "R44", CAP, admin, treasury, CAP + 1);
    }
}

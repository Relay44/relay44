// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {MarketCore} from "../src/MarketCore.sol";

contract MarketCoreTest is Test {
    address internal admin = makeAddr("admin");
    address internal creator = makeAddr("creator");
    address internal resolver = makeAddr("resolver");
    address internal outsider = makeAddr("outsider");

    MarketCore internal marketCore;

    function setUp() external {
        marketCore = new MarketCore(admin);

        vm.startPrank(admin);
        marketCore.grantRole(marketCore.MARKET_CREATOR_ROLE(), creator);
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        vm.stopPrank();
    }

    function test_createMarket() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        bytes32 questionHash = keccak256("Will ETH close above 5k in 2026?");

        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(questionHash, closeTime, resolver);

        (bytes32 storedHash, uint64 storedCloseTime,, address storedResolver, bool resolved,) =
            marketCore.markets(marketId);

        assertEq(marketId, 1);
        assertEq(storedHash, questionHash);
        assertEq(storedCloseTime, closeTime);
        assertEq(storedResolver, resolver);
        assertEq(resolved, false);
    }

    function test_createMarketRichStoresMetadata() external {
        uint64 closeTime = uint64(block.timestamp + 12 hours);
        string memory question = "Will Base TPS exceed 1k by Q3 2026?";
        string memory description = "Resolution based on official Base metrics dashboard.";
        string memory category = "base";
        string memory resolutionSource = "https://base.org";

        vm.prank(creator);
        uint256 marketId =
            marketCore.createMarketRich(question, description, category, resolutionSource, closeTime, resolver);

        (bytes32 storedHash, uint64 storedCloseTime,, address storedResolver, bool resolved,) =
            marketCore.markets(marketId);
        assertEq(storedHash, keccak256(bytes(question)));
        assertEq(storedCloseTime, closeTime);
        assertEq(storedResolver, resolver);
        assertEq(resolved, false);

        (
            string memory storedQuestion,
            string memory storedDescription,
            string memory storedCategory,
            string memory storedResolutionSource
        ) = marketCore.getMarketMetadata(marketId);
        assertEq(storedQuestion, question);
        assertEq(storedDescription, description);
        assertEq(storedCategory, category);
        assertEq(storedResolutionSource, resolutionSource);
    }

    function test_setMarketMetadataRequiresExistingMarket() external {
        vm.prank(creator);
        vm.expectRevert(MarketCore.MarketNotFound.selector);
        marketCore.setMarketMetadata(999, "q?", "d", "c", "s");
    }

    function test_onlyCreatorCanCreate() external {
        vm.prank(outsider);
        vm.expectRevert();
        marketCore.createMarket(keccak256("question"), uint64(block.timestamp + 1 days), resolver);
    }

    function test_resolveMarket() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("question"), uint64(block.timestamp + 4 hours), resolver);

        vm.warp(block.timestamp + 4 hours + 1);

        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertEq(resolved, true);
        assertEq(outcome, true);
    }

    function test_onlyDesignatedResolverCanResolve() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("question"), uint64(block.timestamp + 1 hours), resolver);

        vm.startPrank(admin);
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), outsider);
        vm.stopPrank();

        vm.warp(block.timestamp + 1 hours + 1);

        vm.prank(outsider);
        vm.expectRevert(MarketCore.NotDesignatedResolver.selector);
        marketCore.resolveMarket(marketId, true);
    }

    function test_cannotResolveBeforeCloseTime() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("question"), uint64(block.timestamp + 1 days), resolver);

        vm.prank(resolver);
        vm.expectRevert(MarketCore.MarketNotClosed.selector);
        marketCore.resolveMarket(marketId, true);
    }

    function test_pauseBlocksCreateAndResolve() external {
        vm.prank(admin);
        marketCore.pause();

        vm.prank(creator);
        vm.expectRevert();
        marketCore.createMarket(keccak256("paused"), uint64(block.timestamp + 1 days), resolver);

        vm.prank(admin);
        marketCore.unpause();

        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("question"), uint64(block.timestamp + 2 hours), resolver);

        vm.warp(block.timestamp + 2 hours + 1);

        vm.prank(admin);
        marketCore.pause();

        vm.prank(resolver);
        vm.expectRevert();
        marketCore.resolveMarket(marketId, true);
    }
}

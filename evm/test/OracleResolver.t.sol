// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OracleResolver} from "../src/OracleResolver.sol";
import {MockChainlinkFeed} from "./mocks/MockChainlinkFeed.sol";

contract OracleResolverTest is Test {
    address internal admin = makeAddr("admin");
    address internal keeper = makeAddr("keeper");
    address internal outsider = makeAddr("outsider");

    MarketCore internal marketCore;
    OracleResolver internal oracleResolver;
    MockChainlinkFeed internal ethFeed;

    function setUp() external {
        marketCore = new MarketCore(admin);
        oracleResolver = new OracleResolver(admin, address(marketCore));
        ethFeed = new MockChainlinkFeed(3500e8, 8); // ETH at $3500, 8 decimals

        vm.startPrank(admin);
        // Grant RESOLVER_ROLE to OracleResolver so it can call resolveMarket
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), address(oracleResolver));
        // Grant CONFIGURATOR_ROLE to keeper
        oracleResolver.grantRole(oracleResolver.CONFIGURATOR_ROLE(), keeper);
        vm.stopPrank();
    }

    function _createMarketWithOracleResolver(uint64 closeTime) internal returns (uint256) {
        // Admin creates market with OracleResolver as the resolver
        vm.prank(admin);
        return marketCore.createMarket(keccak256("Will ETH > $3000?"), closeTime, address(oracleResolver));
    }

    // --- configureOracle ---

    function test_configureOracle() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId,
            OracleResolver.FeedType.CHAINLINK,
            address(ethFeed),
            OracleResolver.Comparison.GT,
            3000e8
        );

        (
            OracleResolver.FeedType feedType,
            address feedAddress,
            OracleResolver.Comparison comparison,
            int256 targetValue,
            bool configured
        ) = oracleResolver.oracleConfigs(marketId);

        assertEq(uint8(feedType), uint8(OracleResolver.FeedType.CHAINLINK));
        assertEq(feedAddress, address(ethFeed));
        assertEq(uint8(comparison), uint8(OracleResolver.Comparison.GT));
        assertEq(targetValue, 3000e8);
        assertTrue(configured);
    }

    function test_configureOracle_revertUnauthorized() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        vm.expectRevert();
        oracleResolver.configureOracle(
            marketId,
            OracleResolver.FeedType.CHAINLINK,
            address(ethFeed),
            OracleResolver.Comparison.GT,
            3000e8
        );
    }

    function test_configureOracle_revertAlreadyConfigured() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.startPrank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        vm.expectRevert(abi.encodeWithSelector(OracleResolver.AlreadyConfigured.selector, marketId));
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 4000e8
        );
        vm.stopPrank();
    }

    function test_configureOracle_revertZeroFeedAddress() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.prank(keeper);
        vm.expectRevert(OracleResolver.ZeroAddress.selector);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(0), OracleResolver.Comparison.GT, 3000e8
        );
    }

    function test_configureOracle_manualAllowsZeroFeed() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.MANUAL, address(0), OracleResolver.Comparison.GT, 0
        );

        (,,,, bool configured) = oracleResolver.oracleConfigs(marketId);
        assertTrue(configured);
    }

    // --- resolve (Chainlink) ---

    function test_resolve_GT_true() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        // Warp past close time
        vm.warp(closeTime + 1);
        // Feed at $3500 > $3000 → YES
        ethFeed.setPrice(3500e8);

        vm.prank(outsider); // permissionless
        oracleResolver.resolve(marketId);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertTrue(resolved);
        assertTrue(outcome); // YES
    }

    function test_resolve_GT_false() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 4000e8
        );

        vm.warp(closeTime + 1);
        ethFeed.setPrice(3500e8); // $3500 is NOT > $4000

        oracleResolver.resolve(marketId);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertTrue(resolved);
        assertFalse(outcome); // NO
    }

    function test_resolve_LTE() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.LTE, 3500e8
        );

        vm.warp(closeTime + 1);
        ethFeed.setPrice(3500e8); // $3500 <= $3500 → YES

        oracleResolver.resolve(marketId);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertTrue(resolved);
        assertTrue(outcome);
    }

    function test_resolve_EQ() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.EQ, 3500e8
        );

        vm.warp(closeTime + 1);
        ethFeed.setPrice(3500e8);

        oracleResolver.resolve(marketId);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertTrue(resolved);
        assertTrue(outcome);
    }

    function test_resolve_revertNotConfigured() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.warp(block.timestamp + 1 days + 1);

        vm.expectRevert(abi.encodeWithSelector(OracleResolver.NotConfigured.selector, marketId));
        oracleResolver.resolve(marketId);
    }

    function test_resolve_revertManualOnly() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.MANUAL, address(0), OracleResolver.Comparison.GT, 0
        );

        vm.warp(closeTime + 1);

        vm.expectRevert(OracleResolver.ManualOnly.selector);
        oracleResolver.resolve(marketId);
    }

    function test_resolve_revertMarketNotClosed() external {
        uint256 marketId = _createMarketWithOracleResolver(uint64(block.timestamp + 1 days));

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        // Don't warp — market not closed yet
        vm.expectRevert(OracleResolver.MarketNotClosed.selector);
        oracleResolver.resolve(marketId);
    }

    function test_resolve_revertAlreadyResolved() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        vm.warp(closeTime + 1);
        ethFeed.setPrice(3500e8);
        oracleResolver.resolve(marketId);

        // Try again
        vm.expectRevert(OracleResolver.MarketAlreadyResolved.selector);
        oracleResolver.resolve(marketId);
    }

    function test_resolve_revertFeedStale() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        vm.warp(closeTime + 1);
        // Set feed updatedAt to 2 hours ago (beyond 1hr staleness threshold)
        ethFeed.setUpdatedAt(block.timestamp - 7200);

        vm.expectRevert(
            abi.encodeWithSelector(OracleResolver.FeedStale.selector, block.timestamp - 7200, 3600)
        );
        oracleResolver.resolve(marketId);
    }

    // --- resolveManual ---

    function test_resolveManual() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.MANUAL, address(0), OracleResolver.Comparison.GT, 0
        );

        vm.warp(closeTime + 1);

        vm.prank(keeper);
        oracleResolver.resolveManual(marketId, true);

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        assertTrue(resolved);
        assertTrue(outcome);
    }

    function test_resolveManual_revertUnauthorized() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.MANUAL, address(0), OracleResolver.Comparison.GT, 0
        );

        vm.warp(closeTime + 1);

        vm.prank(outsider);
        vm.expectRevert();
        oracleResolver.resolveManual(marketId, true);
    }

    function test_resolveManual_revertNotManual() external {
        uint64 closeTime = uint64(block.timestamp + 1 days);
        uint256 marketId = _createMarketWithOracleResolver(closeTime);

        vm.prank(keeper);
        oracleResolver.configureOracle(
            marketId, OracleResolver.FeedType.CHAINLINK, address(ethFeed), OracleResolver.Comparison.GT, 3000e8
        );

        vm.warp(closeTime + 1);

        vm.prank(keeper);
        vm.expectRevert(abi.encodeWithSelector(OracleResolver.NotConfigured.selector, marketId));
        oracleResolver.resolveManual(marketId, true);
    }

    // --- constructor ---

    function test_constructor_revertZeroAdmin() external {
        vm.expectRevert(OracleResolver.ZeroAddress.selector);
        new OracleResolver(address(0), address(marketCore));
    }

    function test_constructor_revertZeroMarketCore() external {
        vm.expectRevert(OracleResolver.ZeroAddress.selector);
        new OracleResolver(admin, address(0));
    }
}

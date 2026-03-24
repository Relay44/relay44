// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

contract OrderBookTest is Test {
    address internal admin = makeAddr("admin");
    address internal creator = makeAddr("creator");
    address internal resolver = makeAddr("resolver");
    address internal yesTrader = makeAddr("yes-trader");
    address internal noTrader = makeAddr("no-trader");
    address internal outsider = makeAddr("outsider");

    MarketCore internal marketCore;
    OrderBook internal orderBook;
    CollateralVault internal collateralVault;
    MockERC20 internal usdc;

    function setUp() external {
        marketCore = new MarketCore(admin);
        usdc = new MockERC20("USD Coin", "USDC");
        collateralVault = new CollateralVault(admin, address(usdc));
        orderBook = new OrderBook(admin, address(marketCore), address(collateralVault));

        vm.startPrank(admin);
        marketCore.grantRole(marketCore.MARKET_CREATOR_ROLE(), creator);
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        vm.stopPrank();

        usdc.mint(yesTrader, 1_000e6);
        usdc.mint(noTrader, 1_000e6);

        vm.prank(yesTrader);
        usdc.approve(address(collateralVault), type(uint256).max);
        vm.prank(noTrader);
        usdc.approve(address(collateralVault), type(uint256).max);

        vm.prank(yesTrader);
        collateralVault.deposit(500e6);
        vm.prank(noTrader);
        collateralVault.deposit(500e6);
    }

    function test_placeOrder() external {
        vm.prank(yesTrader);
        uint256 orderId = orderBook.placeOrder(1, true, 5_000, 100e6, uint64(block.timestamp + 1 days));

        (
            address maker,
            uint256 marketId,
            bool isYes,
            uint128 priceBps,
            uint128 size,
            uint128 remaining,
            uint64 expiry,
            bool canceled
        ) = orderBook.orders(orderId);

        assertEq(orderId, 1);
        assertEq(maker, yesTrader);
        assertEq(marketId, 1);
        assertEq(isYes, true);
        assertEq(priceBps, 5_000);
        assertEq(size, 100e6);
        assertEq(remaining, 100e6);
        assertGt(expiry, block.timestamp);
        assertEq(canceled, false);
    }

    function test_matchAndClaimResolvedMarket() external {
        uint64 closeTime = uint64(block.timestamp + 4 hours);

        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("Will BTC close above 120k?"), closeTime, resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_500, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_800, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 40e6);

        (,,,,, uint128 yesRemaining,,) = orderBook.orders(yesOrderId);
        (,,,,, uint128 noRemaining,,) = orderBook.orders(noOrderId);
        assertEq(yesRemaining, 60e6);
        assertEq(noRemaining, 60e6);
        assertEq(collateralVault.availableBalance(address(orderBook)), 80e6);

        (uint128 yesShares,, bool yesClaimed) = orderBook.positions(marketId, yesTrader);
        (, uint128 noShares, bool noClaimed) = orderBook.positions(marketId, noTrader);
        assertEq(yesShares, 40e6);
        assertEq(noShares, 40e6);
        assertEq(yesClaimed, false);
        assertEq(noClaimed, false);

        vm.warp(closeTime + 1);
        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        assertEq(orderBook.claimable(marketId, yesTrader), 80e6);
        assertEq(orderBook.claimable(marketId, noTrader), 0);

        vm.prank(yesTrader);
        uint256 payout = orderBook.claim(marketId);
        assertEq(payout, 80e6);
        assertEq(collateralVault.availableBalance(yesTrader), 540e6);
        assertEq(collateralVault.availableBalance(address(orderBook)), 0);

        vm.prank(noTrader);
        vm.expectRevert(OrderBook.NoWinningShares.selector);
        orderBook.claim(marketId);
    }

    function test_matchFailsForInvalidPriceCross() external {
        vm.prank(creator);
        uint256 marketId =
            marketCore.createMarket(keccak256("price-cross"), uint64(block.timestamp + 2 hours), resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 4_000, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 5_000, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        vm.expectRevert(OrderBook.PriceCrossFailed.selector);
        orderBook.matchOrders(yesOrderId, noOrderId, 10e6);
    }

    function test_permissionlessMatcherCanMatch() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("auth"), uint64(block.timestamp + 2 hours), resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_200, 50e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_900, 50e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 10e6);

        assertEq(collateralVault.availableBalance(address(orderBook)), 20e6);
    }

    function test_claimFailsBeforeResolve() external {
        vm.prank(creator);
        uint256 marketId =
            marketCore.createMarket(keccak256("resolve-gate"), uint64(block.timestamp + 2 hours), resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_100, 50e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_900, 50e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 15e6);

        vm.prank(yesTrader);
        vm.expectRevert(OrderBook.MarketNotResolved.selector);
        orderBook.claim(marketId);
    }

    function test_claimForSettlesToOwner() external {
        uint64 closeTime = uint64(block.timestamp + 2 hours);

        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("agent-claim"), closeTime, resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_300, 60e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_800, 60e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 20e6);

        vm.warp(closeTime + 1);
        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        vm.prank(outsider);
        uint256 payout = orderBook.claimFor(yesTrader, marketId);
        assertEq(payout, 40e6);
        assertEq(collateralVault.availableBalance(yesTrader), 520e6);
        assertEq(orderBook.claimable(marketId, yesTrader), 0);
    }

    function test_pauseBlocksMatchingAndClaim() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("pause"), uint64(block.timestamp + 1 hours), resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_100, 40e6, uint64(block.timestamp + 1 days));

        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_900, 40e6, uint64(block.timestamp + 1 days));

        vm.prank(admin);
        orderBook.pause();

        vm.prank(outsider);
        vm.expectRevert();
        orderBook.matchOrders(yesOrderId, noOrderId, 10e6);

        vm.prank(admin);
        orderBook.unpause();

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 10e6);

        vm.warp(block.timestamp + 1 hours + 1);
        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        vm.prank(admin);
        orderBook.pause();

        vm.prank(yesTrader);
        vm.expectRevert();
        orderBook.claim(marketId);
    }
}

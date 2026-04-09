// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {DistributionMarket} from "../src/DistributionMarket.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {MockERC20} from "./mocks/MockERC20.sol";
import {MockChainlinkFeed} from "./mocks/MockChainlinkFeed.sol";

contract DistributionMarketTest is Test {
    address internal admin = makeAddr("admin");
    address internal creator = makeAddr("creator");
    address internal resolver = makeAddr("resolver");
    address internal operator = makeAddr("operator");
    address internal traderA = makeAddr("trader-a");
    address internal traderB = makeAddr("trader-b");
    address internal outsider = makeAddr("outsider");

    DistributionMarket internal distMarket;
    CollateralVault internal collateralVault;
    MockERC20 internal usdc;
    MockChainlinkFeed internal priceFeed;

    uint256 internal constant SCALE = 1e18;
    uint256 internal constant OUTCOME_SCALE = 1e6;

    function setUp() external {
        usdc = new MockERC20("USD Coin", "USDC");
        collateralVault = new CollateralVault(admin, address(usdc));
        distMarket = new DistributionMarket(admin, address(collateralVault), address(usdc), address(0));
        priceFeed = new MockChainlinkFeed(100_000_000, 8); // $100.00 with 8 decimals

        vm.startPrank(admin);
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(distMarket));
        distMarket.grantRole(distMarket.MARKET_CREATOR_ROLE(), creator);
        distMarket.grantRole(distMarket.OPERATOR_ROLE(), operator);
        vm.stopPrank();

        // Fund traders
        usdc.mint(traderA, 10_000e6);
        usdc.mint(traderB, 10_000e6);

        vm.prank(traderA);
        usdc.approve(address(collateralVault), type(uint256).max);
        vm.prank(traderB);
        usdc.approve(address(collateralVault), type(uint256).max);

        vm.prank(traderA);
        collateralVault.deposit(5_000e6);
        vm.prank(traderB);
        collateralVault.deposit(5_000e6);
    }

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    function _createDefaultMarket() internal returns (uint256 marketId) {
        vm.prank(creator);
        marketId = distMarket.createMarket(
            "What will BTC price be at close?",
            90_000 * OUTCOME_SCALE, // outcomeMin: 90,000
            110_000 * OUTCOME_SCALE, // outcomeMax: 110,000
            100 * SCALE, // liquidityParam
            uint64(block.timestamp + 4 hours),
            resolver,
            false,
            address(0)
        );
    }

    // ----------------------------------------------------------------
    // Tests
    // ----------------------------------------------------------------

    function test_createMarket() external {
        uint256 marketId = _createDefaultMarket();
        assertEq(marketId, 1);

        {
            (
                bytes32 questionHash,
                uint64 closeTime,
                uint64 resolveTime,
                uint256 outcomeMin,
                uint256 outcomeMax,
                uint256 liquidityParam,
                uint256 resolvedValue,
                address res
            ) = distMarket.getMarketCore(marketId);

            assertEq(questionHash, keccak256("What will BTC price be at close?"));
            assertGt(closeTime, block.timestamp);
            assertEq(resolveTime, 0);
            assertEq(outcomeMin, 90_000 * OUTCOME_SCALE);
            assertEq(outcomeMax, 110_000 * OUTCOME_SCALE);
            assertEq(liquidityParam, 100 * SCALE);
            assertEq(resolvedValue, 0);
            assertEq(res, resolver);
        }
        {
            (bool resolved, bool useOracle, address oracleFeed, uint256 totalCollateral, uint256 totalPaidOut) =
                distMarket.getMarketState(marketId);

            assertEq(resolved, false);
            assertEq(useOracle, false);
            assertEq(oracleFeed, address(0));
            assertEq(totalCollateral, 0);
            assertEq(totalPaidOut, 0);
        }

        // Check initial aggregate state
        uint256 expectedMu = (90_000 * OUTCOME_SCALE + 110_000 * OUTCOME_SCALE) / 2;
        uint256 expectedSigma = (110_000 * OUTCOME_SCALE - 90_000 * OUTCOME_SCALE) / 6;
        assertEq(distMarket.marketMu(marketId), expectedMu);
        assertEq(distMarket.marketSigma(marketId), expectedSigma);
    }

    function test_openPosition() external {
        uint256 marketId = _createDefaultMarket();

        uint256 mu = 100_000 * OUTCOME_SCALE;
        uint256 sigma = 2_000 * OUTCOME_SCALE;
        uint256 size = 500e6;

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, mu, sigma, size, size);

        assertEq(posId, 1);

        (address owner, uint256 pMu, uint256 pSigma, uint256 pSize, uint256 pCollateral, bool closed, bool claimed) =
            distMarket.positions(marketId, posId);

        assertEq(owner, traderA);
        assertEq(pMu, mu);
        assertEq(pSigma, sigma);
        assertEq(pSize, size);
        assertEq(pCollateral, size);
        assertEq(closed, false);
        assertEq(claimed, false);

        // Check collateral was locked (transferred to contract in vault)
        assertEq(collateralVault.availableBalance(address(distMarket)), size);

        // Check user position tracking
        uint256[] memory ids = distMarket.getUserPositionIds(marketId, traderA);
        assertEq(ids.length, 1);
        assertEq(ids[0], 1);
    }

    function test_openPosition_invalidMu() external {
        uint256 marketId = _createDefaultMarket();

        // mu below outcomeMin
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.InvalidMu.selector);
        distMarket.openPosition(marketId, 80_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 100e6, 100e6);

        // mu above outcomeMax
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.InvalidMu.selector);
        distMarket.openPosition(marketId, 120_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 100e6, 100e6);
    }

    function test_openPosition_invalidSigma() external {
        uint256 marketId = _createDefaultMarket();

        // sigma below MIN_SIGMA
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.InvalidSigma.selector);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 100_000, 100e6, 100e6); // 0.1 < 0.15
    }

    function test_closePosition() external {
        uint256 marketId = _createDefaultMarket();

        uint256 size = 500e6;
        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, size, size);

        uint256 balBefore = collateralVault.availableBalance(traderA);

        vm.prank(traderA);
        distMarket.closePosition(marketId, posId);

        (,,,,, bool closed,) = distMarket.positions(marketId, posId);
        assertEq(closed, true);

        // No fee configured, so full refund
        uint256 balAfter = collateralVault.availableBalance(traderA);
        assertEq(balAfter - balBefore, size);
    }

    function test_closePosition_notOwner() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        vm.prank(traderB);
        vm.expectRevert(DistributionMarket.NotPositionOwner.selector);
        distMarket.closePosition(marketId, posId);
    }

    function test_resolveAndClaim() external {
        uint256 marketId = _createDefaultMarket();
        uint256 posIdA;
        uint256 posIdB;

        {
            // Trader A: believes price will be 102k, tight sigma (high conviction)
            vm.prank(traderA);
            posIdA = distMarket.openPosition(
                marketId, 102_000 * OUTCOME_SCALE, 1_000 * OUTCOME_SCALE, 500e6, 500e6
            );

            // Trader B: believes price will be 95k, wider sigma
            vm.prank(traderB);
            posIdB = distMarket.openPosition(
                marketId, 95_000 * OUTCOME_SCALE, 3_000 * OUTCOME_SCALE, 500e6, 500e6
            );
        }

        // Update market aggregate state (normally done by backend)
        vm.prank(operator);
        distMarket.updateMarketState(marketId, 98_500 * OUTCOME_SCALE, 4_000 * OUTCOME_SCALE);

        // Warp past close time and resolve at 102k (trader A was right)
        vm.warp(block.timestamp + 5 hours);
        vm.prank(resolver);
        distMarket.resolve(marketId, 102_000 * OUTCOME_SCALE);

        // Check market is resolved
        {
            (bool isResolved,,,,) = distMarket.getMarketState(marketId);
            (,,,,,, uint256 rv,) = distMarket.getMarketCore(marketId);
            assertEq(isResolved, true);
            assertEq(rv, 102_000 * OUTCOME_SCALE);
        }

        // Trader A claims — their position was centered on the resolved value
        uint256 payoutA;
        {
            uint256 balBefore = collateralVault.availableBalance(traderA);
            vm.prank(traderA);
            payoutA = distMarket.claim(marketId, posIdA);
            uint256 balAfter = collateralVault.availableBalance(traderA);
            assertEq(balAfter - balBefore, payoutA);
            assertGt(payoutA, 0);
        }

        // Trader B claims — their position was far from resolved value
        uint256 payoutB;
        {
            uint256 balBefore = collateralVault.availableBalance(traderB);
            vm.prank(traderB);
            payoutB = distMarket.claim(marketId, posIdB);
            uint256 balAfter = collateralVault.availableBalance(traderB);
            assertEq(balAfter - balBefore, payoutB);
        }

        // Trader A should get more than trader B since they predicted correctly
        assertGt(payoutA, payoutB);

        // Total paid out should not exceed total collateral
        assertLe(payoutA + payoutB, 1_000e6);
    }

    function test_resolveBeforeClose() external {
        uint256 marketId = _createDefaultMarket();

        // Try to resolve before close time
        vm.prank(resolver);
        vm.expectRevert(DistributionMarket.MarketNotClosed.selector);
        distMarket.resolve(marketId, 100_000 * OUTCOME_SCALE);
    }

    function test_updateMarketState() external {
        uint256 marketId = _createDefaultMarket();

        uint256 newMu = 105_000 * OUTCOME_SCALE;
        uint256 newSigma = 1_500 * OUTCOME_SCALE;

        vm.prank(operator);
        distMarket.updateMarketState(marketId, newMu, newSigma);

        assertEq(distMarket.marketMu(marketId), newMu);
        assertEq(distMarket.marketSigma(marketId), newSigma);
    }

    function test_gaussianPdf() external view {
        // Standard normal: mu=0, sigma=1 (in OUTCOME_SCALE: mu=0, sigma=1_000_000)
        // PDF at x=0 should be ~0.3989 (in SCALE: ~398_942_280_401_432_678)
        uint256 sigma = 1 * OUTCOME_SCALE; // 1.0
        uint256 mu = 0;
        uint256 x = 0;

        uint256 pdf0 = distMarket.gaussianPdf(x, mu, sigma);
        // Expected: 1 / sqrt(2*pi) ≈ 0.39894228... → 398942280401432678 in SCALE
        // Allow 0.5% tolerance
        uint256 expected = 398_942_280_401_432_678;
        uint256 tolerance = expected / 200; // 0.5%
        assertApproxEqAbs(pdf0, expected, tolerance);

        // PDF at x = 1*sigma (z=1): expected ~0.2419707 → 241970724519143365
        uint256 pdf1 = distMarket.gaussianPdf(sigma, mu, sigma);
        uint256 expected1 = 241_970_724_519_143_365;
        uint256 tolerance1 = expected1 / 200;
        assertApproxEqAbs(pdf1, expected1, tolerance1);

        // PDF at x = 2*sigma (z=2): expected ~0.05399 → 53990966513188063
        // Taylor-series exp approximation has ~2% error in tails
        uint256 pdf2 = distMarket.gaussianPdf(2 * sigma, mu, sigma);
        uint256 expected2 = 53_990_966_513_188_063;
        uint256 tolerance2 = expected2 / 50; // 2% tolerance for tail
        assertApproxEqAbs(pdf2, expected2, tolerance2);
    }

    function test_resolveFromOracle() external {
        // Create an oracle-backed market
        vm.prank(creator);
        uint256 marketId = distMarket.createMarket(
            "What will ETH price be?",
            80_000_000, // $80 in OUTCOME_SCALE
            120_000_000, // $120 in OUTCOME_SCALE
            100 * SCALE,
            uint64(block.timestamp + 4 hours),
            resolver,
            true,
            address(priceFeed)
        );

        vm.warp(block.timestamp + 5 hours);

        // Feed has $100.00 (100_000_000 in 8 decimals → 100_000_000 in OUTCOME_SCALE)
        // 100_000_000 / 10^(8-6) = 1_000_000 ... wait, that's $1.
        // We need the feed to return the value in a range that maps correctly.
        // Feed: 100_000_000 (8 decimals) = $100.00
        // Scaled to OUTCOME_SCALE (6 decimals): 100_000_000 / 10^2 = 1_000_000 = $1.00 in OUTCOME_SCALE
        // That's not right for our range. Let's set the feed to match our range.
        // Our range is 80_000_000 to 120_000_000 (OUTCOME_SCALE).
        // That means $80 to $120 if OUTCOME_SCALE represents dollars.
        // Feed at 8 decimals: $100 = 10_000_000_000 (1e10)
        // Scaled: 10_000_000_000 / 10^2 = 100_000_000 ✓
        priceFeed.setPrice(10_000_000_000); // $100 at 8 decimals

        distMarket.resolveFromOracle(marketId);

        {
            (bool isResolved,,,,) = distMarket.getMarketState(marketId);
            (,,,,,, uint256 rv,) = distMarket.getMarketCore(marketId);
            assertEq(isResolved, true);
            assertEq(rv, 100_000_000); // $100 in OUTCOME_SCALE
        }
    }

    function test_slippageProtection() external {
        uint256 marketId = _createDefaultMarket();

        // maxCollateral less than size should revert
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.SlippageExceeded.selector);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 500e6, 400e6);
    }

    function test_cannotClaimBeforeResolution() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.MarketNotResolved.selector);
        distMarket.claim(marketId, posId);
    }

    function test_cannotClaimTwice() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        // Set market state so claim works
        vm.prank(operator);
        distMarket.updateMarketState(marketId, 100_000 * OUTCOME_SCALE, 3_000 * OUTCOME_SCALE);

        vm.warp(block.timestamp + 5 hours);
        vm.prank(resolver);
        distMarket.resolve(marketId, 100_000 * OUTCOME_SCALE);

        vm.prank(traderA);
        distMarket.claim(marketId, posId);

        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.PositionAlreadyClaimed.selector);
        distMarket.claim(marketId, posId);
    }

    // ----------------------------------------------------------------
    // New hardening tests
    // ----------------------------------------------------------------

    function test_openPosition_sigmaTooLarge() external {
        uint256 marketId = _createDefaultMarket();
        // Max sigma = (110k - 90k) / 2 = 10k in OUTCOME_SCALE = 10_000 * OUTCOME_SCALE
        // Try sigma = 11k which exceeds half the range
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.SigmaTooLarge.selector);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 11_000 * OUTCOME_SCALE, 200e6, 200e6);
    }

    function test_openPosition_sizeTooSmall() external {
        uint256 marketId = _createDefaultMarket();
        // MIN_SIZE = 1_000
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.InvalidSize.selector);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 999, 999);
    }

    function test_closePosition_afterCloseTime() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        // Warp past close time
        vm.warp(block.timestamp + 5 hours);

        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.MarketClosed.selector);
        distMarket.closePosition(marketId, posId);
    }

    function test_cancelMarket() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        // Admin cancels
        vm.prank(admin);
        distMarket.cancelMarket(marketId);

        // Market should be resolved (flag used to prevent trading)
        (bool isResolved,,,,) = distMarket.getMarketState(marketId);
        assertEq(isResolved, true);

        // Cannot open new positions
        vm.prank(traderB);
        vm.expectRevert(DistributionMarket.MarketAlreadyResolved.selector);
        distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);
    }

    function test_emergencyWithdraw() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(marketId, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 200e6, 200e6);

        uint256 balBefore = collateralVault.availableBalance(traderA);

        // Admin cancels
        vm.prank(admin);
        distMarket.cancelMarket(marketId);

        // Trader A emergency withdraws — full refund
        vm.prank(traderA);
        distMarket.emergencyWithdraw(marketId, posId);

        uint256 balAfter = collateralVault.availableBalance(traderA);
        assertEq(balAfter - balBefore, 200e6);

        // Cannot withdraw again
        vm.prank(traderA);
        vm.expectRevert(DistributionMarket.PositionAlreadyClosed.selector);
        distMarket.emergencyWithdraw(marketId, posId);
    }

    function test_resolveFromOracle_negativePrice() external {
        vm.prank(creator);
        uint256 marketId = distMarket.createMarket(
            "Negative price test",
            80_000_000,
            120_000_000,
            100 * SCALE,
            uint64(block.timestamp + 4 hours),
            resolver,
            true,
            address(priceFeed)
        );

        vm.warp(block.timestamp + 5 hours);

        // Set negative price
        priceFeed.setPrice(-100_000_000);

        vm.expectRevert(DistributionMarket.OracleNegativePrice.selector);
        distMarket.resolveFromOracle(marketId);
    }

    function test_payoutRatioCapped() external {
        uint256 marketId = _createDefaultMarket();

        // Trader A: very tight sigma (high conviction) at 102k
        vm.prank(traderA);
        uint256 posId = distMarket.openPosition(
            marketId, 102_000 * OUTCOME_SCALE, 500 * OUTCOME_SCALE, 500e6, 500e6
        );

        // Set market state to very wide distribution
        vm.prank(operator);
        distMarket.updateMarketState(marketId, 100_000 * OUTCOME_SCALE, 8_000 * OUTCOME_SCALE);

        vm.warp(block.timestamp + 5 hours);
        vm.prank(resolver);
        distMarket.resolve(marketId, 102_000 * OUTCOME_SCALE);

        vm.prank(traderA);
        uint256 payout = distMarket.claim(marketId, posId);

        // Payout should be capped at 10x collateral (MAX_PAYOUT_RATIO)
        // Also capped at pool size (500e6), so max is min(10*500e6, 500e6) = 500e6
        assertLe(payout, 500e6);
    }

    // ----------------------------------------------------------------
    // openPositionFor (operator-submitted LMSR collateral)
    // ----------------------------------------------------------------

    function test_openPositionFor() external {
        uint256 marketId = _createDefaultMarket();

        uint256 mu = 100_000 * OUTCOME_SCALE;
        uint256 sigma = 2_000 * OUTCOME_SCALE;
        uint256 size = 500e6;
        uint256 lmsrCollateral = 350e6; // LMSR-computed collateral < size

        vm.prank(operator);
        uint256 posId = distMarket.openPositionFor(marketId, traderA, mu, sigma, size, lmsrCollateral, lmsrCollateral);

        assertEq(posId, 1);

        (address owner,, , , uint256 pCollateral,,) = distMarket.positions(marketId, posId);
        assertEq(owner, traderA);
        assertEq(pCollateral, lmsrCollateral); // Collateral is LMSR-computed, not size

        // Check collateral was locked from trader's balance
        assertEq(collateralVault.availableBalance(address(distMarket)), lmsrCollateral);

        // Check user position tracking
        uint256[] memory ids = distMarket.getUserPositionIds(marketId, traderA);
        assertEq(ids.length, 1);
        assertEq(ids[0], 1);
    }

    function test_openPositionFor_notOperator() external {
        uint256 marketId = _createDefaultMarket();

        // Non-operator cannot call openPositionFor
        vm.prank(traderA);
        vm.expectRevert();
        distMarket.openPositionFor(
            marketId, traderA, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 500e6, 350e6, 350e6
        );
    }

    function test_openPositionFor_slippageCheck() external {
        uint256 marketId = _createDefaultMarket();

        // Collateral exceeds maxCollateral
        vm.prank(operator);
        vm.expectRevert(DistributionMarket.SlippageExceeded.selector);
        distMarket.openPositionFor(
            marketId, traderA, 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 500e6, 400e6, 300e6
        );
    }

    function test_openPositionFor_zeroTrader() external {
        uint256 marketId = _createDefaultMarket();

        vm.prank(operator);
        vm.expectRevert(DistributionMarket.ZeroAddress.selector);
        distMarket.openPositionFor(
            marketId, address(0), 100_000 * OUTCOME_SCALE, 2_000 * OUTCOME_SCALE, 500e6, 350e6, 350e6
        );
    }
}

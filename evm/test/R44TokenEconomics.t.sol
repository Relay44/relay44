// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {R44Token} from "../src/R44Token.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {R44Staking} from "../src/R44Staking.sol";
import {RewardDistributor} from "../src/RewardDistributor.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

contract R44TokenBurnTest is Test {
    address internal admin = makeAddr("admin");
    address internal treasury = makeAddr("treasury");
    address internal user = makeAddr("user");
    address internal burner = makeAddr("burner");

    R44Token internal token;

    function setUp() external {
        token = new R44Token("Relay44", "R44", 1_000_000e18, admin, treasury, 200_000e18);
    }

    function test_userCanBurnOwnTokens() external {
        vm.prank(treasury);
        token.transfer(user, 100e18);

        vm.prank(user);
        token.burn(50e18);
        assertEq(token.balanceOf(user), 50e18);
        assertEq(token.totalSupply(), 200_000e18 - 50e18);
    }

    function test_burnerRoleCanBurnFrom() external {
        vm.startPrank(admin);
        token.grantRole(token.BURNER_ROLE(), burner);
        token.mint(user, 100e18);
        vm.stopPrank();

        vm.prank(user);
        token.approve(burner, 50e18);

        vm.prank(burner);
        token.burnFrom(user, 50e18);
        assertEq(token.balanceOf(user), 50e18);
    }

    function test_nonBurnerCannotBurnFrom() external {
        vm.prank(admin);
        token.mint(user, 100e18);

        vm.prank(user);
        token.approve(burner, 100e18);

        vm.prank(burner);
        vm.expectRevert();
        token.burnFrom(user, 50e18);
    }
}

contract OrderBookFeeTest is Test {
    address internal admin = makeAddr("admin");
    address internal resolver = makeAddr("resolver");
    address internal yesTrader = makeAddr("yes-trader");
    address internal noTrader = makeAddr("no-trader");
    address internal outsider = makeAddr("outsider");
    address internal feeWallet = makeAddr("fee-wallet");

    MarketCore internal marketCore;
    OrderBook internal orderBook;
    CollateralVault internal collateralVault;
    MockERC20 internal usdc;
    R44Token internal r44;

    function setUp() external {
        vm.startPrank(admin);
        marketCore = new MarketCore(admin);
        r44 = new R44Token("Relay44", "R44", 1_000_000e18, admin, admin, 500_000e18);
        vm.stopPrank();

        usdc = new MockERC20("USD Coin", "USDC");

        vm.startPrank(admin);
        collateralVault = new CollateralVault(admin, address(usdc));
        orderBook = new OrderBook(admin, address(marketCore), address(collateralVault), address(r44));

        marketCore.grantRole(marketCore.MARKET_CREATOR_ROLE(), resolver);
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        orderBook.setFeeConfig(200, feeWallet); // 2% fee
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

    function test_claimDeductsFee() external {
        uint64 closeTime = uint64(block.timestamp + 4 hours);

        vm.prank(resolver);
        uint256 marketId = marketCore.createMarket(keccak256("fee-test"), closeTime, resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_500, 100e6, uint64(block.timestamp + 1 days));
        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_800, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 40e6);

        vm.warp(closeTime + 1);
        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        // Gross payout = 80e6, fee = 80e6 * 200 / 10000 = 1.6e6
        uint256 expectedFee = (80e6 * 200) / 10_000;
        uint256 expectedPayout = 80e6 - expectedFee;

        assertEq(orderBook.claimable(marketId, yesTrader), expectedPayout);

        vm.prank(yesTrader);
        uint256 payout = orderBook.claim(marketId);
        assertEq(payout, expectedPayout);
        assertEq(orderBook.accruedFees(), expectedFee);
    }

    function test_r44HolderGetsDiscount() external {
        // Give yesTrader 10K R44 for Tier 2 (50% discount)
        vm.startPrank(admin);
        r44.mint(yesTrader, 10_000e18);
        vm.stopPrank();

        assertEq(orderBook.getDiscountBps(yesTrader), 5_000);

        uint256 grossPayout = 80e6;
        uint256 baseFee = (grossPayout * 200) / 10_000; // 1.6e6
        uint256 discountedFee = baseFee - (baseFee * 5_000) / 10_000; // 0.8e6

        assertEq(orderBook.calculateFee(grossPayout, yesTrader), discountedFee);
    }

    function test_tier3Gets75PercentDiscount() external {
        vm.prank(admin);
        r44.mint(yesTrader, 100_000e18);

        assertEq(orderBook.getDiscountBps(yesTrader), 7_500);
    }

    function test_noDiscountWithoutR44() external {
        assertEq(orderBook.getDiscountBps(yesTrader), 0);
    }

    function test_zeroFeeMeansNoPayout() external {
        vm.prank(admin);
        orderBook.setFeeConfig(0, address(0));

        assertEq(orderBook.calculateFee(80e6, yesTrader), 0);
    }

    function test_setFeeConfigOnlyAdmin() external {
        vm.prank(outsider);
        vm.expectRevert();
        orderBook.setFeeConfig(100, feeWallet);
    }

    function test_feeCannotExceedMax() external {
        vm.prank(admin);
        vm.expectRevert(OrderBook.FeeTooHigh.selector);
        orderBook.setFeeConfig(1_001, feeWallet);
    }

    function test_withdrawFees() external {
        uint64 closeTime = uint64(block.timestamp + 4 hours);

        vm.prank(resolver);
        uint256 marketId = marketCore.createMarket(keccak256("withdraw-fee"), closeTime, resolver);

        vm.prank(yesTrader);
        uint256 yesOrderId = orderBook.placeOrder(marketId, true, 5_500, 100e6, uint64(block.timestamp + 1 days));
        vm.prank(noTrader);
        uint256 noOrderId = orderBook.placeOrder(marketId, false, 4_800, 100e6, uint64(block.timestamp + 1 days));

        vm.prank(outsider);
        orderBook.matchOrders(yesOrderId, noOrderId, 40e6);

        vm.warp(closeTime + 1);
        vm.prank(resolver);
        marketCore.resolveMarket(marketId, true);

        vm.prank(yesTrader);
        orderBook.claim(marketId);

        uint256 fees = orderBook.accruedFees();
        assertGt(fees, 0);

        vm.prank(admin);
        orderBook.withdrawFees();
        assertEq(orderBook.accruedFees(), 0);
        assertEq(collateralVault.availableBalance(feeWallet), fees);
    }
}

contract AgentRuntimeBurnTest is Test {
    address internal admin = makeAddr("admin");
    address internal resolver = makeAddr("resolver");
    address internal alice = makeAddr("alice");
    address internal executor = makeAddr("executor");

    R44Token internal r44;
    MarketCore internal marketCore;
    MockERC20 internal usdc;
    CollateralVault internal collateralVault;
    OrderBook internal orderBook;
    AgentRuntime internal agentRuntime;
    AgentIdentityRegistry internal identityRegistry;
    uint256 internal marketId;

    function setUp() external {
        vm.startPrank(admin);
        r44 = new R44Token("Relay44", "R44", 1_000_000e18, admin, admin, 500_000e18);
        marketCore = new MarketCore(admin);
        vm.stopPrank();

        usdc = new MockERC20("USD Coin", "USDC");

        vm.startPrank(admin);
        collateralVault = new CollateralVault(admin, address(usdc));
        orderBook = new OrderBook(admin, address(marketCore), address(collateralVault), address(r44));
        agentRuntime = new AgentRuntime(admin, address(orderBook));
        identityRegistry = new AgentIdentityRegistry(admin);

        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        orderBook.grantRole(orderBook.AGENT_RUNTIME_ROLE(), address(agentRuntime));
        identityRegistry.grantRole(identityRegistry.REGISTRAR_ROLE(), address(agentRuntime));
        agentRuntime.setIdentityRegistry(address(identityRegistry));

        // Wire R44 burn
        r44.grantRole(r44.BURNER_ROLE(), address(agentRuntime));
        agentRuntime.setR44Token(address(r44));
        agentRuntime.setExecutionFee(1e15); // 0.001 R44

        // Fund alice with R44
        r44.transfer(alice, 10e18);
        vm.stopPrank();

        vm.prank(alice);
        r44.approve(address(agentRuntime), type(uint256).max);

        usdc.mint(alice, 1_000e6);
        vm.prank(alice);
        usdc.approve(address(collateralVault), type(uint256).max);
        vm.prank(alice);
        collateralVault.deposit(500e6);

        vm.prank(resolver);
        marketId = marketCore.createMarket(keccak256("burn-test"), uint64(block.timestamp + 2 days), resolver);
    }

    function test_executionBurnsR44() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_200, 25e6, 60, 3_600, "burn-test");

        uint256 balanceBefore = r44.balanceOf(alice);

        vm.prank(executor);
        agentRuntime.executeAgent(agentId);

        assertEq(r44.balanceOf(alice), balanceBefore - 1e15);
    }

    function test_executionFailsWithoutR44() external {
        // Create agent first while alice still has R44
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_200, 25e6, 60, 3_600, "no-r44");

        // Now drain alice's R44
        uint256 aliceBalance = r44.balanceOf(alice);
        vm.prank(alice);
        r44.transfer(admin, aliceBalance);
        assertEq(r44.balanceOf(alice), 0);

        vm.prank(executor);
        vm.expectRevert(AgentRuntime.InsufficientR44ForExecution.selector);
        agentRuntime.executeAgent(agentId);
    }

    function test_noFeeWhenDisabled() external {
        vm.prank(admin);
        agentRuntime.setExecutionFee(0);

        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_200, 25e6, 60, 3_600, "no-fee");

        uint256 balanceBefore = r44.balanceOf(alice);

        vm.prank(executor);
        agentRuntime.executeAgent(agentId);

        assertEq(r44.balanceOf(alice), balanceBefore);
    }
}

contract MarketCreationDepositTest is Test {
    address internal admin = makeAddr("admin");
    address internal resolver = makeAddr("resolver");
    address internal creator = makeAddr("creator");
    address internal slashWallet = makeAddr("slash-wallet");

    R44Token internal r44;
    MarketCore internal marketCore;

    function setUp() external {
        vm.startPrank(admin);
        r44 = new R44Token("Relay44", "R44", 1_000_000e18, admin, admin, 500_000e18);
        marketCore = new MarketCore(admin);

        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        marketCore.setR44Token(address(r44));
        marketCore.setCreationDeposit(100e18, slashWallet);

        r44.transfer(creator, 1_000e18);
        vm.stopPrank();
        vm.prank(creator);
        r44.approve(address(marketCore), type(uint256).max);
    }

    function test_marketCreationCollectsDeposit() external {
        uint256 balanceBefore = r44.balanceOf(creator);

        vm.prank(creator);
        marketCore.createMarket(keccak256("deposit-test"), uint64(block.timestamp + 1 days), creator);

        assertEq(r44.balanceOf(creator), balanceBefore - 100e18);
        assertEq(r44.balanceOf(address(marketCore)), 100e18);
    }

    function test_depositRefundedOnResolution() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("refund-test"), uint64(block.timestamp + 1 hours), creator);

        vm.warp(block.timestamp + 1 hours + 1);
        vm.prank(creator);
        marketCore.resolveMarket(marketId, true);

        uint256 balanceBefore = r44.balanceOf(creator);
        vm.prank(creator);
        marketCore.refundDeposit(marketId);

        assertEq(r44.balanceOf(creator), balanceBefore + 100e18);
    }

    function test_depositSlashedByAdmin() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("slash-test"), uint64(block.timestamp + 1 hours), creator);

        vm.prank(admin);
        marketCore.slashDeposit(marketId);

        assertEq(r44.balanceOf(slashWallet), 100e18);
        assertTrue(marketCore.depositRefunded(marketId));
    }

    function test_cannotDoubleRefund() external {
        vm.prank(creator);
        uint256 marketId = marketCore.createMarket(keccak256("double-refund"), uint64(block.timestamp + 1 hours), creator);

        vm.warp(block.timestamp + 1 hours + 1);
        vm.prank(creator);
        marketCore.resolveMarket(marketId, true);

        vm.prank(creator);
        marketCore.refundDeposit(marketId);

        vm.prank(creator);
        vm.expectRevert(MarketCore.DepositAlreadyRefunded.selector);
        marketCore.refundDeposit(marketId);
    }

    function test_noDepositWhenZero() external {
        vm.prank(admin);
        marketCore.setCreationDeposit(0, address(0));

        uint256 balanceBefore = r44.balanceOf(creator);
        vm.prank(creator);
        marketCore.createMarket(keccak256("no-deposit"), uint64(block.timestamp + 1 days), creator);

        assertEq(r44.balanceOf(creator), balanceBefore);
    }
}

contract R44StakingTest is Test {
    address internal admin = makeAddr("admin");
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");
    address internal distributor = makeAddr("distributor");

    R44Token internal r44;
    R44Staking internal staking;

    function setUp() external {
        vm.startPrank(admin);
        r44 = new R44Token("Relay44", "R44", 1_000_000e18, admin, admin, 500_000e18);
        staking = new R44Staking(admin, address(r44));

        staking.grantRole(staking.DISTRIBUTOR_ROLE(), distributor);

        r44.transfer(alice, 50_000e18);
        r44.transfer(bob, 50_000e18);
        r44.transfer(distributor, 100_000e18);
        vm.stopPrank();

        vm.prank(alice);
        r44.approve(address(staking), type(uint256).max);
        vm.prank(bob);
        r44.approve(address(staking), type(uint256).max);
        vm.prank(distributor);
        r44.approve(address(staking), type(uint256).max);
    }

    function test_stakeAndUnstake() external {
        vm.prank(alice);
        staking.stake(10_000e18, uint64(7 days));

        (uint256 amount, uint64 unlockAt) = staking.stakeOf(alice);
        assertEq(amount, 10_000e18);
        assertEq(unlockAt, uint64(block.timestamp + 7 days));
        assertEq(staking.totalStaked(), 10_000e18);

        // Cannot unstake early
        vm.prank(alice);
        vm.expectRevert(R44Staking.StakeStillLocked.selector);
        staking.unstake();

        vm.warp(block.timestamp + 7 days);
        vm.prank(alice);
        staking.unstake();

        assertEq(r44.balanceOf(alice), 50_000e18);
        assertEq(staking.totalStaked(), 0);
    }

    function test_tiers() external {
        assertEq(staking.getTier(alice), 0);

        vm.prank(alice);
        staking.stake(1_000e18, uint64(30 days));
        assertEq(staking.getTier(alice), 1);

        vm.warp(block.timestamp + 30 days);
        vm.prank(alice);
        staking.unstake();

        vm.prank(alice);
        staking.stake(10_000e18, uint64(30 days));
        assertEq(staking.getTier(alice), 2);
    }

    function test_rewardDistribution() external {
        vm.prank(alice);
        staking.stake(10_000e18, uint64(30 days));

        vm.prank(bob);
        staking.stake(10_000e18, uint64(30 days));

        // Distribute 1000 R44 rewards
        vm.prank(distributor);
        staking.depositRewards(1_000e18);

        // Each should get 500 R44
        assertEq(staking.pendingRewardOf(alice), 500e18);
        assertEq(staking.pendingRewardOf(bob), 500e18);

        vm.prank(alice);
        staking.claimRewards();
        assertEq(r44.balanceOf(alice), 40_000e18 + 500e18); // initial - staked + reward
    }

    function test_cannotStakeTwice() external {
        vm.prank(alice);
        staking.stake(5_000e18, uint64(7 days));

        vm.prank(alice);
        vm.expectRevert(R44Staking.AlreadyStaked.selector);
        staking.stake(5_000e18, uint64(7 days));
    }

    function test_extendLock() external {
        vm.prank(alice);
        staking.stake(5_000e18, uint64(7 days));

        uint64 newUnlock = uint64(block.timestamp + 30 days);
        vm.prank(alice);
        staking.extendLock(newUnlock);

        (, uint64 unlockAt) = staking.stakeOf(alice);
        assertEq(unlockAt, newUnlock);
    }
}

contract RewardDistributorTest is Test {
    address internal admin = makeAddr("admin");
    address internal treasury = makeAddr("treasury");
    address internal keeper = makeAddr("keeper");
    address internal agent1 = makeAddr("agent1");
    address internal agent2 = makeAddr("agent2");
    address internal creator1 = makeAddr("creator1");

    R44Token internal r44;
    R44Staking internal staking;
    RewardDistributor internal distributor;

    function setUp() external {
        vm.startPrank(admin);
        r44 = new R44Token("Relay44", "R44", 1_000_000e18, admin, admin, 500_000e18);
        staking = new R44Staking(admin, address(r44));
        distributor = new RewardDistributor(admin, address(r44), treasury, 7 days);

        distributor.grantRole(distributor.KEEPER_ROLE(), keeper);
        staking.grantRole(staking.DISTRIBUTOR_ROLE(), address(distributor));
        distributor.setStakingPool(address(staking));

        // Fund distributor with R44 (simulating Clanker fee income)
        r44.transfer(address(distributor), 10_000e18);
        vm.stopPrank();
    }

    function test_distribute() external {
        vm.warp(block.timestamp + 7 days + 1);

        vm.prank(keeper);
        distributor.distribute();

        assertEq(distributor.currentEpoch(), 1);
        // 20% staking, 40% agents, 30% creators, 10% treasury
        assertEq(distributor.epochAgentPool(1), 4_000e18);
        assertEq(distributor.epochCreatorPool(1), 3_000e18);
        assertEq(r44.balanceOf(treasury), 1_000e18);
    }

    function test_agentRewardClaim() external {
        vm.warp(block.timestamp + 7 days + 1);

        vm.prank(keeper);
        distributor.distribute();

        address[] memory agents = new address[](2);
        agents[0] = agent1;
        agents[1] = agent2;
        uint256[] memory amounts = new uint256[](2);
        amounts[0] = 2_500e18;
        amounts[1] = 1_500e18;

        vm.prank(keeper);
        distributor.setAgentRewards(1, agents, amounts);

        vm.prank(agent1);
        distributor.claimAgentReward(1);
        assertEq(r44.balanceOf(agent1), 2_500e18);

        vm.prank(agent1);
        vm.expectRevert(RewardDistributor.AlreadyClaimed.selector);
        distributor.claimAgentReward(1);
    }

    function test_cannotDistributeTooEarly() external {
        vm.prank(keeper);
        vm.expectRevert(RewardDistributor.EpochNotReady.selector);
        distributor.distribute();
    }

    function test_cannotExceedAgentPool() external {
        vm.warp(block.timestamp + 7 days + 1);
        vm.prank(keeper);
        distributor.distribute();

        address[] memory agents = new address[](1);
        agents[0] = agent1;
        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 5_000e18; // exceeds 4_000e18 pool

        vm.prank(keeper);
        vm.expectRevert(RewardDistributor.AllocationExceedsPool.selector);
        distributor.setAgentRewards(1, agents, amounts);
    }
}

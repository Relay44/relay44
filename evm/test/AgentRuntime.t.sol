// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {MockERC20} from "./mocks/MockERC20.sol";

contract AgentRuntimeTest is Test {
    address internal admin = makeAddr("admin");
    address internal creator = makeAddr("creator");
    address internal resolver = makeAddr("resolver");
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");

    MarketCore internal marketCore;
    MockERC20 internal usdc;
    CollateralVault internal collateralVault;
    OrderBook internal orderBook;
    AgentRuntime internal agentRuntime;
    AgentIdentityRegistry internal identityRegistry;
    uint256 internal marketId;

    function setUp() external {
        marketCore = new MarketCore(admin);
        usdc = new MockERC20("USD Coin", "USDC");
        collateralVault = new CollateralVault(admin, address(usdc));
        orderBook = new OrderBook(admin, address(marketCore), address(collateralVault));
        agentRuntime = new AgentRuntime(admin, address(orderBook));
        identityRegistry = new AgentIdentityRegistry(admin);

        vm.startPrank(admin);
        marketCore.grantRole(marketCore.MARKET_CREATOR_ROLE(), creator);
        marketCore.grantRole(marketCore.RESOLVER_ROLE(), resolver);
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        orderBook.grantRole(orderBook.AGENT_RUNTIME_ROLE(), address(agentRuntime));
        identityRegistry.grantRole(identityRegistry.REGISTRAR_ROLE(), address(agentRuntime));
        agentRuntime.setIdentityRegistry(address(identityRegistry));
        vm.stopPrank();

        usdc.mint(alice, 1_000e6);
        vm.prank(alice);
        usdc.approve(address(collateralVault), type(uint256).max);
        vm.prank(alice);
        collateralVault.deposit(500e6);

        vm.prank(creator);
        marketId = marketCore.createMarket(keccak256("agent-runtime"), uint64(block.timestamp + 2 days), resolver);
    }

    function test_createAndExecuteAgentPlacesOrder() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_200, 25e6, 60, 3_600, "fixed-schedule");

        vm.prank(bob);
        uint256 orderId = agentRuntime.executeAgent(agentId);

        assertEq(orderId, 1);
        (
            address maker,
            uint256 storedMarketId,
            bool isYes,
            uint128 priceBps,
            uint128 size,
            uint128 remaining,
            uint64 expiry,
            bool canceled
        ) = orderBook.orders(orderId);

        assertEq(maker, alice);
        assertEq(storedMarketId, marketId);
        assertEq(isYes, true);
        assertEq(priceBps, 5_200);
        assertEq(size, 25e6);
        assertEq(remaining, 25e6);
        assertGt(expiry, block.timestamp);
        assertEq(canceled, false);
    }

    function test_executeRespectsCadence() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_000, 10e6, 600, 3_600, "cadence");

        agentRuntime.executeAgent(agentId);

        vm.expectRevert(AgentRuntime.ExecutionTooEarly.selector);
        agentRuntime.executeAgent(agentId);

        vm.warp(block.timestamp + 600);
        agentRuntime.executeAgent(agentId);
        assertEq(orderBook.orderCount(), 2);
    }

    function test_onlyOwnerCanUpdateAgent() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_000, 10e6, 300, 3_600, "base");

        vm.prank(bob);
        vm.expectRevert(AgentRuntime.NotOwner.selector);
        agentRuntime.updateAgent(agentId, false, 4_800, 12e6, 300, 3_600, "updated");

        vm.prank(alice);
        agentRuntime.updateAgent(agentId, false, 4_800, 12e6, 300, 3_600, "updated");

        uint256 orderId = agentRuntime.executeAgent(agentId);
        (, uint256 storedMarketId, bool isYes, uint128 priceBps, uint128 size,,,) = orderBook.orders(orderId);
        assertEq(storedMarketId, marketId);
        assertEq(isYes, false);
        assertEq(priceBps, 4_800);
        assertEq(size, 12e6);
    }

    function test_pauseBlocksExecution() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_000, 10e6, 60, 600, "pause");

        vm.prank(admin);
        agentRuntime.pause();

        vm.expectRevert();
        agentRuntime.executeAgent(agentId);
    }

    function test_registerAgentIdentity() external {
        vm.prank(alice);
        uint256 agentId = agentRuntime.createAgent(marketId, true, 5_000, 10e6, 60, 600, "identity");

        vm.prank(alice);
        uint256 identityId = agentRuntime.registerAgentIdentity(agentId, "ipfs://neura-agent/1");

        assertEq(identityId, 1);
        assertEq(agentRuntime.agentIdentityId(agentId), 1);
        assertEq(identityRegistry.ownerOf(identityId), alice);
    }
}

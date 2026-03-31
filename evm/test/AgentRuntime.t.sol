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
    address internal operator = makeAddr("operator");
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
        orderBook = new OrderBook(admin, address(marketCore), address(collateralVault), address(0));
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

        vm.prank(resolver);
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

    function test_managerCanProvisionAndUpdateCreatorOwnedBootstrapAgents() external {
        vm.prank(alice);
        agentRuntime.setManagerApproval(operator, true);

        AgentRuntime.AgentConfig[] memory configs = new AgentRuntime.AgentConfig[](2);
        configs[0] = AgentRuntime.AgentConfig({
            marketId: marketId, isYes: true, priceBps: 5_100, size: 10e6, cadence: 300, expiryWindow: 900
        });
        configs[1] = AgentRuntime.AgentConfig({
            marketId: marketId, isYes: false, priceBps: 4_900, size: 12e6, cadence: 300, expiryWindow: 900
        });

        vm.prank(operator);
        uint256[] memory agentIds = agentRuntime.createAgentsFor(alice, operator, configs, "ladder_v1");

        assertEq(agentIds.length, 2);
        (
            address owner,
            address manager,
            uint256 storedMarketId,
            bool isYes,
            uint128 priceBps,
            uint128 size,
            uint64 cadence,
            uint64 expiryWindow,
            uint64 lastExecutedAt,
            bool active,
            string memory strategy
        ) = agentRuntime.agents(agentIds[0]);

        assertEq(owner, alice);
        assertEq(manager, operator);
        assertEq(storedMarketId, marketId);
        assertTrue(isYes);
        assertEq(priceBps, 5_100);
        assertEq(size, 10e6);
        assertEq(cadence, 300);
        assertEq(expiryWindow, 900);
        assertEq(lastExecutedAt, 0);
        assertTrue(active);
        assertEq(strategy, "ladder_v1");

        AgentRuntime.AgentUpdateConfig[] memory updates = new AgentRuntime.AgentUpdateConfig[](1);
        updates[0] = AgentRuntime.AgentUpdateConfig({
            agentId: agentIds[0], isYes: false, priceBps: 4_700, size: 15e6, cadence: 180, expiryWindow: 1_200
        });

        vm.prank(operator);
        agentRuntime.updateAgents(updates, "ladder_v1");

        vm.prank(bob);
        uint256 orderId = agentRuntime.executeAgent(agentIds[0]);
        (, uint256 updatedMarketId, bool updatedIsYes, uint128 updatedPrice, uint128 updatedSize,,,) =
            orderBook.orders(orderId);
        assertEq(updatedMarketId, marketId);
        assertFalse(updatedIsYes);
        assertEq(updatedPrice, 4_700);
        assertEq(updatedSize, 15e6);
    }

    function test_managerCanDeactivateBootstrapAgents() external {
        vm.prank(alice);
        agentRuntime.setManagerApproval(operator, true);

        AgentRuntime.AgentConfig[] memory configs = new AgentRuntime.AgentConfig[](1);
        configs[0] = AgentRuntime.AgentConfig({
            marketId: marketId, isYes: true, priceBps: 5_000, size: 10e6, cadence: 300, expiryWindow: 900
        });

        vm.prank(operator);
        uint256[] memory agentIds = agentRuntime.createAgentsFor(alice, operator, configs, "ladder_v1");

        uint256[] memory ids = new uint256[](1);
        ids[0] = agentIds[0];

        vm.prank(operator);
        agentRuntime.deactivateAgents(ids);

        vm.expectRevert(AgentRuntime.AgentInactive.selector);
        agentRuntime.executeAgent(agentIds[0]);
    }

    function test_unapprovedManagerCannotProvisionOrUpdate() external {
        AgentRuntime.AgentConfig[] memory configs = new AgentRuntime.AgentConfig[](1);
        configs[0] = AgentRuntime.AgentConfig({
            marketId: marketId, isYes: true, priceBps: 5_000, size: 10e6, cadence: 300, expiryWindow: 900
        });

        vm.prank(operator);
        vm.expectRevert(AgentRuntime.ManagerNotApproved.selector);
        agentRuntime.createAgentsFor(alice, operator, configs, "ladder_v1");

        vm.prank(alice);
        agentRuntime.setManagerApproval(operator, true);

        vm.prank(operator);
        uint256[] memory agentIds = agentRuntime.createAgentsFor(alice, operator, configs, "ladder_v1");

        vm.prank(alice);
        agentRuntime.setManagerApproval(operator, false);

        AgentRuntime.AgentUpdateConfig[] memory updates = new AgentRuntime.AgentUpdateConfig[](1);
        updates[0] = AgentRuntime.AgentUpdateConfig({
            agentId: agentIds[0], isYes: false, priceBps: 4_900, size: 11e6, cadence: 120, expiryWindow: 600
        });

        vm.prank(operator);
        vm.expectRevert(AgentRuntime.NotAuthorized.selector);
        agentRuntime.updateAgents(updates, "ladder_v1");
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { MockERC20 } from "./MockERC20.sol";
import { SingularityAgentPolicy } from "../src/SingularityAgentPolicy.sol";
import { SingularityCollateralVault } from "../src/SingularityCollateralVault.sol";
import { SingularityMarketCore } from "../src/SingularityMarketCore.sol";
import { SingularityOrderbookCore } from "../src/SingularityOrderbookCore.sol";
import { SingularityOutcomeToken1155 } from "../src/SingularityOutcomeToken1155.sol";

contract TraderProxy {
    function approveToken(address token, address spender, uint256 amount) external {
        (bool ok, bytes memory data) = token.call(abi.encodeWithSignature("approve(address,uint256)", spender, amount));
        require(ok, "approve-failed");
        if (data.length > 0) {
            require(abi.decode(data, (bool)), "approve-false");
        }
    }

    function placeOrder(address orderbook, SingularityOrderbookCore.PlaceOrderParams calldata params)
        external
        returns (uint256)
    {
        return SingularityOrderbookCore(orderbook).placeOrder(params);
    }

    function claim(address orderbook, uint256 marketId, uint128 quantity) external {
        SingularityOrderbookCore(orderbook).claim(marketId, quantity);
    }
}

contract SingularityCoreTest {
    MockERC20 internal collateral;
    SingularityMarketCore internal marketCore;
    SingularityOutcomeToken1155 internal outcomeToken;
    SingularityCollateralVault internal vault;
    SingularityAgentPolicy internal agentPolicy;
    SingularityOrderbookCore internal orderbook;

    TraderProxy internal traderA;
    TraderProxy internal traderB;

    address internal treasury = address(0xBEEF);

    function setUp() public {
        collateral = new MockERC20();
        marketCore = new SingularityMarketCore(address(this));
        outcomeToken = new SingularityOutcomeToken1155(address(this));
        vault = new SingularityCollateralVault(address(this), address(collateral), treasury);
        agentPolicy = new SingularityAgentPolicy(address(this));
        orderbook = new SingularityOrderbookCore(
            address(this),
            address(marketCore),
            address(outcomeToken),
            address(vault),
            address(agentPolicy)
        );

        traderA = new TraderProxy();
        traderB = new TraderProxy();

        marketCore.grantRole(marketCore.OPERATOR_ROLE(), address(this));
        marketCore.grantRole(marketCore.ORACLE_ROLE(), address(this));
        marketCore.grantRole(marketCore.PAUSER_ROLE(), address(this));

        outcomeToken.grantRole(outcomeToken.MINTER_ROLE(), address(orderbook));
        vault.grantRole(vault.OPERATOR_ROLE(), address(orderbook));
        agentPolicy.grantRole(agentPolicy.OPERATOR_ROLE(), address(this));
        orderbook.grantRole(orderbook.OPERATOR_ROLE(), address(this));

        collateral.mint(address(traderA), 1_000_000_000);
        collateral.mint(address(traderB), 1_000_000_000);

        traderA.approveToken(address(collateral), address(vault), type(uint256).max);
        traderB.approveToken(address(collateral), address(vault), type(uint256).max);
    }

    function testCreatePlaceMatchResolveClaim() public {
        uint256 marketId = marketCore.createMarket(
            address(this),
            "Will dual core parity ship before launch?",
            keccak256("market-meta"),
            uint64(block.timestamp + 7 days),
            uint64(block.timestamp + 14 days)
        );

        SingularityOrderbookCore.PlaceOrderParams memory buyOrder = SingularityOrderbookCore.PlaceOrderParams({
            marketId: marketId,
            side: SingularityOrderbookCore.Side.Buy,
            outcome: 0,
            priceBps: 6_000,
            quantity: 100_000_000,
            expiresAt: uint64(block.timestamp + 3 days),
            agent: address(0)
        });

        SingularityOrderbookCore.PlaceOrderParams memory sellOrder = SingularityOrderbookCore.PlaceOrderParams({
            marketId: marketId,
            side: SingularityOrderbookCore.Side.Sell,
            outcome: 0,
            priceBps: 6_000,
            quantity: 100_000_000,
            expiresAt: uint64(block.timestamp + 3 days),
            agent: address(0)
        });

        uint256 buyOrderId = traderA.placeOrder(address(orderbook), buyOrder);
        uint256 sellOrderId = traderB.placeOrder(address(orderbook), sellOrder);

        orderbook.matchOrders(buyOrderId, sellOrderId, 100_000_000, 6_000);

        (uint256 yesTokenId,) = marketCore.getOutcomeTokenIds(marketId);
        _assertEq(outcomeToken.balanceOf(yesTokenId, address(traderA)), 100_000_000, "trader A token balance");

        marketCore.resolveMarket(marketId, 0, address(this), keccak256("evidence"), "committee");

        uint256 beforePayout = collateral.balanceOf(address(traderA));
        traderA.claim(address(orderbook), marketId, 100_000_000);
        uint256 afterPayout = collateral.balanceOf(address(traderA));
        uint256 treasuryBalance = collateral.balanceOf(treasury);

        _assertEq(afterPayout - beforePayout, 99_500_000, "winner net payout");
        _assertEq(treasuryBalance, 500_000, "fee payout");
    }

    function testCancelRefundsUnfilled() public {
        uint256 marketId = marketCore.createMarket(
            address(this),
            "Will cancellation return collateral?",
            keccak256("cancel-meta"),
            uint64(block.timestamp + 2 days),
            uint64(block.timestamp + 4 days)
        );

        SingularityOrderbookCore.PlaceOrderParams memory buyOrder = SingularityOrderbookCore.PlaceOrderParams({
            marketId: marketId,
            side: SingularityOrderbookCore.Side.Buy,
            outcome: 1,
            priceBps: 4_000,
            quantity: 50_000_000,
            expiresAt: uint64(block.timestamp + 1 days),
            agent: address(0)
        });

        uint256 before = collateral.balanceOf(address(traderA));
        uint256 orderId = traderA.placeOrder(address(orderbook), buyOrder);

        orderbook.cancelOrder(orderId);

        uint256 afterBalance = collateral.balanceOf(address(traderA));
        _assertEq(afterBalance, before, "cancel should refund full collateral");
    }

    function testAgentPolicyBlocksOversizedOrder() public {
        agentPolicy.setPolicy(
            address(0xA11CE),
            SingularityAgentPolicy.Policy({
                enabled: true,
                maxOrderQuantity: 10_000_000,
                maxOrderNotional: 10_000_000,
                maxOpenOrders: 1
            })
        );

        uint256 marketId = marketCore.createMarket(
            address(this),
            "Will policy enforce limits?",
            keccak256("policy-meta"),
            uint64(block.timestamp + 3 days),
            uint64(block.timestamp + 6 days)
        );

        SingularityOrderbookCore.PlaceOrderParams memory orderParams = SingularityOrderbookCore.PlaceOrderParams({
            marketId: marketId,
            side: SingularityOrderbookCore.Side.Buy,
            outcome: 0,
            priceBps: 9_000,
            quantity: 20_000_000,
            expiresAt: uint64(block.timestamp + 1 days),
            agent: address(0xA11CE)
        });

        (bool ok,) = address(traderA).call(
            abi.encodeWithSelector(TraderProxy.placeOrder.selector, address(orderbook), orderParams)
        );
        require(!ok, "expected policy revert");
    }

    function _assertEq(uint256 left, uint256 right, string memory reason) internal pure {
        require(left == right, reason);
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";
import { ISingularityMarketCore } from "./interfaces/ISingularityMarketCore.sol";
import { SingularityOutcomeToken1155 } from "./SingularityOutcomeToken1155.sol";
import { SingularityCollateralVault } from "./SingularityCollateralVault.sol";
import { SingularityAgentPolicy } from "./SingularityAgentPolicy.sol";

contract SingularityOrderbookCore is RoleAuth {
    error InvalidState();
    error InvalidOrder();
    error InvalidPrice();
    error InvalidQuantity();
    error NotOrderOwner();
    error OrderNotOpen();
    error OrderExpired();
    error MarketNotTradable();
    error UnsupportedOutcome();

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    enum Side {
        Buy,
        Sell
    }

    enum OrderStatus {
        Open,
        PartiallyFilled,
        Filled,
        Cancelled,
        Expired
    }

    struct Order {
        uint256 id;
        uint256 marketId;
        address owner;
        address agent;
        Side side;
        uint8 outcome;
        uint16 priceBps;
        uint128 quantity;
        uint128 filledQuantity;
        uint64 expiresAt;
        OrderStatus status;
    }

    struct PlaceOrderParams {
        uint256 marketId;
        Side side;
        uint8 outcome;
        uint16 priceBps;
        uint128 quantity;
        uint64 expiresAt;
        address agent;
    }

    event OrderPlaced(
        uint256 indexed orderId,
        uint256 indexed marketId,
        address indexed owner,
        address agent,
        Side side,
        uint8 outcome,
        uint16 priceBps,
        uint128 quantity,
        uint64 expiresAt,
        uint256 collateralLocked
    );

    event OrderCancelled(
        uint256 indexed orderId,
        uint256 indexed marketId,
        address indexed owner,
        uint128 refundedQuantity,
        uint256 refundedCollateral
    );

    event OrdersMatched(
        uint256 indexed buyOrderId,
        uint256 indexed sellOrderId,
        uint256 indexed marketId,
        uint8 outcome,
        uint16 executionPriceBps,
        uint128 quantity
    );

    event WinningsClaimed(
        uint256 indexed marketId,
        address indexed owner,
        uint8 resolvedOutcome,
        uint128 quantity,
        uint256 grossPayout,
        uint256 feePayout
    );

    ISingularityMarketCore public immutable marketCore;
    SingularityOutcomeToken1155 public immutable outcomeToken;
    SingularityCollateralVault public immutable collateralVault;
    SingularityAgentPolicy public immutable agentPolicy;

    uint256 public nextOrderId = 1;

    mapping(uint256 => Order) public orders;
    mapping(address => uint256) public openOrderCountByOwner;

    constructor(
        address admin,
        address marketCoreAddress,
        address outcomeTokenAddress,
        address collateralVaultAddress,
        address agentPolicyAddress
    ) RoleAuth(admin) {
        if (
            marketCoreAddress == address(0)
                || outcomeTokenAddress == address(0)
                || collateralVaultAddress == address(0)
                || agentPolicyAddress == address(0)
        ) {
            revert InvalidAddress();
        }
        marketCore = ISingularityMarketCore(marketCoreAddress);
        outcomeToken = SingularityOutcomeToken1155(outcomeTokenAddress);
        collateralVault = SingularityCollateralVault(collateralVaultAddress);
        agentPolicy = SingularityAgentPolicy(agentPolicyAddress);
    }

    function placeOrder(PlaceOrderParams calldata params) external returns (uint256 orderId) {
        if (!marketCore.isTradable(params.marketId)) revert MarketNotTradable();
        if (params.outcome > 1) revert UnsupportedOutcome();
        if (params.priceBps == 0 || params.priceBps >= 10_000) revert InvalidPrice();
        if (params.quantity == 0) revert InvalidQuantity();
        if (params.expiresAt != 0 && params.expiresAt <= block.timestamp) revert OrderExpired();

        uint256 notional = (uint256(params.quantity) * uint256(params.priceBps)) / 10_000;
        if (params.agent != address(0)) {
            agentPolicy.enforceOrder(params.agent, params.quantity, notional, openOrderCountByOwner[msg.sender]);
        }

        uint256 collateralToLock = _collateralRequired(params.side, params.priceBps, params.quantity);
        collateralVault.collectFrom(msg.sender, collateralToLock, _orderReason(params.marketId, "LOCK"));

        orderId = nextOrderId;
        nextOrderId += 1;

        orders[orderId] = Order({
            id: orderId,
            marketId: params.marketId,
            owner: msg.sender,
            agent: params.agent,
            side: params.side,
            outcome: params.outcome,
            priceBps: params.priceBps,
            quantity: params.quantity,
            filledQuantity: 0,
            expiresAt: params.expiresAt,
            status: OrderStatus.Open
        });

        openOrderCountByOwner[msg.sender] += 1;

        emit OrderPlaced(
            orderId,
            params.marketId,
            msg.sender,
            params.agent,
            params.side,
            params.outcome,
            params.priceBps,
            params.quantity,
            params.expiresAt,
            collateralToLock
        );
    }

    function cancelOrder(uint256 orderId) external {
        Order storage order = orders[orderId];
        if (order.id == 0) revert InvalidOrder();
        if (order.owner != msg.sender && !hasRole(OPERATOR_ROLE, msg.sender)) revert NotOrderOwner();

        if (_isExpired(order)) {
            order.status = OrderStatus.Expired;
        }

        if (order.status != OrderStatus.Open && order.status != OrderStatus.PartiallyFilled) {
            revert OrderNotOpen();
        }

        uint128 unfilledQty = order.quantity - order.filledQuantity;
        uint256 refundCollateral = _collateralRequired(order.side, order.priceBps, unfilledQty);

        order.filledQuantity = order.quantity;
        order.status = OrderStatus.Cancelled;
        if (openOrderCountByOwner[order.owner] > 0) {
            openOrderCountByOwner[order.owner] -= 1;
        }

        collateralVault.refundTo(order.owner, refundCollateral, _orderReason(order.marketId, "CANCEL"));

        emit OrderCancelled(order.id, order.marketId, order.owner, unfilledQty, refundCollateral);
    }

    function expireOrder(uint256 orderId) external {
        Order storage order = orders[orderId];
        if (order.id == 0) revert InvalidOrder();
        if (!_isExpired(order)) revert OrderNotOpen();
        if (order.status != OrderStatus.Open && order.status != OrderStatus.PartiallyFilled) revert OrderNotOpen();

        uint128 unfilledQty = order.quantity - order.filledQuantity;
        uint256 refundCollateral = _collateralRequired(order.side, order.priceBps, unfilledQty);
        order.filledQuantity = order.quantity;
        order.status = OrderStatus.Expired;
        if (openOrderCountByOwner[order.owner] > 0) {
            openOrderCountByOwner[order.owner] -= 1;
        }

        collateralVault.refundTo(order.owner, refundCollateral, _orderReason(order.marketId, "EXPIRE"));
    }

    function matchOrders(uint256 orderAId, uint256 orderBId, uint128 quantity, uint16 executionPriceBps)
        external
        onlyRole(OPERATOR_ROLE)
    {
        if (quantity == 0) revert InvalidQuantity();
        if (executionPriceBps == 0 || executionPriceBps >= 10_000) revert InvalidPrice();

        Order storage a = orders[orderAId];
        Order storage b = orders[orderBId];
        if (a.id == 0 || b.id == 0) revert InvalidOrder();
        if (_isExpired(a) || _isExpired(b)) revert OrderExpired();
        if (!_isOpen(a) || !_isOpen(b)) revert OrderNotOpen();
        if (a.marketId != b.marketId || a.outcome != b.outcome) revert InvalidOrder();
        if (a.side == b.side) revert InvalidOrder();

        Order storage buyOrder = a.side == Side.Buy ? a : b;
        Order storage sellOrder = a.side == Side.Sell ? a : b;

        uint128 buyRemaining = buyOrder.quantity - buyOrder.filledQuantity;
        uint128 sellRemaining = sellOrder.quantity - sellOrder.filledQuantity;
        if (quantity > buyRemaining || quantity > sellRemaining) revert InvalidQuantity();

        buyOrder.filledQuantity += quantity;
        sellOrder.filledQuantity += quantity;

        _syncOrderStatus(buyOrder);
        _syncOrderStatus(sellOrder);

        (uint256 yesTokenId, uint256 noTokenId) = marketCore.getOutcomeTokenIds(buyOrder.marketId);

        if (buyOrder.outcome == 0) {
            outcomeToken.mint(buyOrder.owner, yesTokenId, quantity);
            outcomeToken.mint(sellOrder.owner, noTokenId, quantity);
        } else {
            outcomeToken.mint(buyOrder.owner, noTokenId, quantity);
            outcomeToken.mint(sellOrder.owner, yesTokenId, quantity);
        }

        emit OrdersMatched(buyOrder.id, sellOrder.id, buyOrder.marketId, buyOrder.outcome, executionPriceBps, quantity);
    }

    function claim(uint256 marketId, uint128 quantity) external {
        if (quantity == 0) revert InvalidQuantity();

        (bool resolved, uint8 resolvedOutcome) = marketCore.isResolved(marketId);
        if (!resolved || resolvedOutcome > 1) revert InvalidState();

        (uint256 yesTokenId, uint256 noTokenId) = marketCore.getOutcomeTokenIds(marketId);
        uint256 winningTokenId = resolvedOutcome == 0 ? yesTokenId : noTokenId;

        outcomeToken.burn(msg.sender, winningTokenId, quantity);
        (uint256 netAmount, uint256 feeAmount) = collateralVault.payoutTo(msg.sender, quantity);

        emit WinningsClaimed(marketId, msg.sender, resolvedOutcome, quantity, netAmount + feeAmount, feeAmount);
    }

    function _syncOrderStatus(Order storage order) internal {
        uint128 remaining = order.quantity - order.filledQuantity;
        if (remaining == 0) {
            order.status = OrderStatus.Filled;
            if (openOrderCountByOwner[order.owner] > 0) {
                openOrderCountByOwner[order.owner] -= 1;
            }
            return;
        }
        order.status = OrderStatus.PartiallyFilled;
    }

    function _isExpired(Order storage order) internal view returns (bool) {
        return order.expiresAt != 0 && block.timestamp >= order.expiresAt;
    }

    function _isOpen(Order storage order) internal view returns (bool) {
        return order.status == OrderStatus.Open || order.status == OrderStatus.PartiallyFilled;
    }

    function _collateralRequired(Side side, uint16 priceBps, uint128 quantity) internal pure returns (uint256) {
        if (side == Side.Buy) {
            return (uint256(quantity) * priceBps) / 10_000;
        }
        return (uint256(quantity) * (10_000 - priceBps)) / 10_000;
    }

    function _orderReason(uint256 marketId, string memory suffix) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked("SINGULARITY_ORDERBOOK", marketId, suffix));
    }
}

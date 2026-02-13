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


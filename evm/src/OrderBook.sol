// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

interface IMarketCoreRead {
    function markets(uint256 marketId)
        external
        view
        returns (
            bytes32 questionHash,
            uint64 closeTime,
            uint64 resolveTime,
            address resolver,
            bool resolved,
            bool outcome
        );
}

interface ICollateralVault {
    function lock(address user, uint256 amount) external;
    function settle(address from, address to, uint256 amount) external;
    function transferAvailable(address from, address to, uint256 amount) external;
}

contract OrderBook is AccessControl, Pausable, ReentrancyGuard {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant AGENT_RUNTIME_ROLE = keccak256("AGENT_RUNTIME_ROLE");

    uint256 public constant MIN_PRICE_BPS = 1;
    uint256 public constant MAX_PRICE_BPS = 9_999;
    uint256 public constant PAR_PRICE_BPS = 10_000;

    struct Order {
        address maker;
        uint256 marketId;
        bool isYes;
        uint128 priceBps;
        uint128 size;
        uint128 remaining;
        uint64 expiry;
        bool canceled;
    }

    struct Position {
        uint128 yesShares;
        uint128 noShares;
        bool claimed;
    }

    struct MarketPool {
        uint256 escrow;
        uint256 paidOut;
        uint256 matchedShares;
    }

    uint256 public orderCount;
    mapping(uint256 => Order) public orders;
    mapping(uint256 => mapping(address => Position)) public positions;
    mapping(uint256 => MarketPool) public marketPools;

    IMarketCoreRead public immutable marketCore;
    ICollateralVault public immutable collateralVault;

    error ZeroAddress();
    error InvalidPrice();
    error PriceCrossFailed();
    error InvalidSize();
    error InvalidExpiry();
    error OrderNotFound();
    error OrderExpired();
    error OrderAlreadyCanceled();
    error OrderFullyFilled();
    error NotOrderOwner();
    error FillExceedsRemaining();
    error InvalidMatchPair();
    error MarketNotResolved();
    error AlreadyClaimed();
    error NoPosition();
    error NoWinningShares();
    error InsufficientEscrow();

    event OrderPlaced(
        uint256 indexed orderId,
        address indexed maker,
        uint256 indexed marketId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 expiry
    );
    event OrderCanceled(uint256 indexed orderId, address indexed actor);
    event OrderFilled(uint256 indexed orderId, uint128 fillSize, uint128 remaining, address indexed matcher);
    event OrdersMatched(
        uint256 indexed marketId,
        uint256 indexed yesOrderId,
        uint256 indexed noOrderId,
        uint128 fillSize,
        uint128 yesPriceBps,
        uint128 noPriceBps
    );
    event Claimed(uint256 indexed marketId, address indexed user, bool outcome, uint256 payout, uint256 shares);

    constructor(address admin, address marketCoreAddress, address collateralVaultAddress) {
        if (admin == address(0) || marketCoreAddress == address(0) || collateralVaultAddress == address(0)) {
            revert ZeroAddress();
        }

        marketCore = IMarketCoreRead(marketCoreAddress);
        collateralVault = ICollateralVault(collateralVaultAddress);
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(AGENT_RUNTIME_ROLE, admin);
    }

    function placeOrder(uint256 marketId, bool isYes, uint128 priceBps, uint128 size, uint64 expiry)
        external
        whenNotPaused
        returns (uint256 orderId)
    {
        orderId = _placeOrder(msg.sender, marketId, isYes, priceBps, size, expiry);
    }

    function placeOrderFor(address maker, uint256 marketId, bool isYes, uint128 priceBps, uint128 size, uint64 expiry)
        external
        onlyRole(AGENT_RUNTIME_ROLE)
        whenNotPaused
        returns (uint256 orderId)
    {
        if (maker == address(0)) revert ZeroAddress();
        orderId = _placeOrder(maker, marketId, isYes, priceBps, size, expiry);
    }

    function matchOrders(uint256 firstOrderId, uint256 secondOrderId, uint128 fillSize)
        external
        whenNotPaused
        nonReentrant
    {
        if (firstOrderId == secondOrderId) revert InvalidMatchPair();

        Order storage first = orders[firstOrderId];
        Order storage second = orders[secondOrderId];
        if (first.maker == address(0) || second.maker == address(0)) revert OrderNotFound();
        if (first.marketId != second.marketId || first.isYes == second.isYes) revert InvalidMatchPair();

        _assertOrderFillable(first, fillSize);
        _assertOrderFillable(second, fillSize);

        Order storage yesOrder = first.isYes ? first : second;
        Order storage noOrder = first.isYes ? second : first;
        if (uint256(yesOrder.priceBps) + uint256(noOrder.priceBps) < PAR_PRICE_BPS) {
            revert PriceCrossFailed();
        }

        yesOrder.remaining -= fillSize;
        noOrder.remaining -= fillSize;

        collateralVault.lock(yesOrder.maker, fillSize);
        collateralVault.lock(noOrder.maker, fillSize);
        collateralVault.settle(yesOrder.maker, address(this), fillSize);
        collateralVault.settle(noOrder.maker, address(this), fillSize);

        positions[yesOrder.marketId][yesOrder.maker].yesShares += fillSize;
        positions[yesOrder.marketId][noOrder.maker].noShares += fillSize;

        MarketPool storage pool = marketPools[yesOrder.marketId];
        pool.escrow += uint256(fillSize) * 2;
        pool.matchedShares += fillSize;

        emit OrderFilled(firstOrderId, fillSize, first.remaining, msg.sender);
        emit OrderFilled(secondOrderId, fillSize, second.remaining, msg.sender);
        emit OrdersMatched(
            yesOrder.marketId,
            first.isYes ? firstOrderId : secondOrderId,
            first.isYes ? secondOrderId : firstOrderId,
            fillSize,
            yesOrder.priceBps,
            noOrder.priceBps
        );
    }

    function cancelOrder(uint256 orderId) external whenNotPaused {
        Order storage order = orders[orderId];
        if (order.maker == address(0)) revert OrderNotFound();
        if (order.canceled) revert OrderAlreadyCanceled();
        if (order.remaining == 0) revert OrderFullyFilled();
        if (msg.sender != order.maker && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotOrderOwner();
        }

        order.canceled = true;
        emit OrderCanceled(orderId, msg.sender);
    }

    function claim(uint256 marketId) external whenNotPaused nonReentrant returns (uint256 payout) {
        return _claim(msg.sender, marketId);
    }

    function claimFor(address user, uint256 marketId) external whenNotPaused nonReentrant returns (uint256 payout) {
        if (user == address(0)) revert ZeroAddress();
        return _claim(user, marketId);
    }

    function _claim(address user, uint256 marketId) internal returns (uint256 payout) {
        Position storage position = positions[marketId][user];
        if (position.claimed) revert AlreadyClaimed();
        if (position.yesShares == 0 && position.noShares == 0) revert NoPosition();

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        if (!resolved) revert MarketNotResolved();

        uint256 winningShares = outcome ? position.yesShares : position.noShares;
        if (winningShares == 0) revert NoWinningShares();

        payout = winningShares * 2;

        MarketPool storage pool = marketPools[marketId];
        uint256 remainingEscrow = pool.escrow - pool.paidOut;
        if (remainingEscrow < payout) revert InsufficientEscrow();
        pool.paidOut += payout;

        position.yesShares = 0;
        position.noShares = 0;
        position.claimed = true;

        collateralVault.transferAvailable(address(this), user, payout);
        emit Claimed(marketId, user, outcome, payout, winningShares);
    }

    function claimable(uint256 marketId, address user) external view returns (uint256) {
        if (user == address(0)) revert ZeroAddress();

        Position memory position = positions[marketId][user];
        if (position.claimed) return 0;
        if (position.yesShares == 0 && position.noShares == 0) return 0;

        (,,,, bool resolved, bool outcome) = marketCore.markets(marketId);
        if (!resolved) return 0;

        uint256 winningShares = outcome ? position.yesShares : position.noShares;
        return winningShares * 2;
    }

    function _placeOrder(address maker, uint256 marketId, bool isYes, uint128 priceBps, uint128 size, uint64 expiry)
        internal
        returns (uint256 orderId)
    {
        if (priceBps < MIN_PRICE_BPS || priceBps > MAX_PRICE_BPS) revert InvalidPrice();
        if (size == 0) revert InvalidSize();
        if (expiry <= block.timestamp) revert InvalidExpiry();

        orderId = ++orderCount;
        orders[orderId] = Order({
            maker: maker,
            marketId: marketId,
            isYes: isYes,
            priceBps: priceBps,
            size: size,
            remaining: size,
            expiry: expiry,
            canceled: false
        });

        emit OrderPlaced(orderId, maker, marketId, isYes, priceBps, size, expiry);
    }

    function _assertOrderFillable(Order storage order, uint128 fillSize) internal view {
        if (order.maker == address(0)) revert OrderNotFound();
        if (order.canceled) revert OrderAlreadyCanceled();
        if (order.remaining == 0) revert OrderFullyFilled();
        if (order.expiry < block.timestamp) revert OrderExpired();
        if (fillSize == 0 || fillSize > order.remaining) revert FillExceedsRemaining();
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}

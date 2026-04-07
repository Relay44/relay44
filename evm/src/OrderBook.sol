// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

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
    function unlock(address user, uint256 amount) external;
    function settle(address from, address to, uint256 amount) external;
    function transferAvailable(address from, address to, uint256 amount) external;
}

contract OrderBook is AccessControl, Pausable, ReentrancyGuard {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant AGENT_RUNTIME_ROLE = keccak256("AGENT_RUNTIME_ROLE");

    uint256 public constant MIN_PRICE_BPS = 1;
    uint256 public constant MAX_PRICE_BPS = 9_999;
    uint256 public constant PAR_PRICE_BPS = 10_000;
    uint256 public constant MAX_FEE_BPS = 1_000; // 10% max fee

    // RELAY holder discount tiers
    uint256 public constant TIER1_THRESHOLD = 1_000e18;   // 25% fee discount
    uint256 public constant TIER2_THRESHOLD = 10_000e18;  // 50% fee discount
    uint256 public constant TIER3_THRESHOLD = 100_000e18; // 75% fee discount

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
    IERC20 public immutable relayToken;

    uint256 public feeBps;         // Protocol fee in basis points
    address public feeRecipient;   // Where fees are sent
    uint256 public accruedFees;    // Fees accumulated in the vault

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
    error FeeTooHigh();
    error InvalidFeeRecipient();
    error NoFeesToWithdraw();

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
    event FeeConfigUpdated(uint256 feeBps, address feeRecipient);
    event FeesWithdrawn(address indexed recipient, uint256 amount);

    constructor(address admin, address marketCoreAddress, address collateralVaultAddress, address relayTokenAddress) {
        if (admin == address(0) || marketCoreAddress == address(0) || collateralVaultAddress == address(0)) {
            revert ZeroAddress();
        }

        marketCore = IMarketCoreRead(marketCoreAddress);
        collateralVault = ICollateralVault(collateralVaultAddress);
        relayToken = IERC20(relayTokenAddress); // address(0) disables discounts
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(AGENT_RUNTIME_ROLE, admin);
    }

    function setFeeConfig(uint256 _feeBps, address _feeRecipient) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (_feeBps > MAX_FEE_BPS) revert FeeTooHigh();
        if (_feeBps > 0 && _feeRecipient == address(0)) revert InvalidFeeRecipient();
        feeBps = _feeBps;
        feeRecipient = _feeRecipient;
        emit FeeConfigUpdated(_feeBps, _feeRecipient);
    }

    function withdrawFees() external onlyRole(DEFAULT_ADMIN_ROLE) {
        uint256 amount = accruedFees;
        if (amount == 0) revert NoFeesToWithdraw();
        accruedFees = 0;
        collateralVault.transferAvailable(address(this), feeRecipient, amount);
        emit FeesWithdrawn(feeRecipient, amount);
    }

    function getDiscountBps(address user) public view returns (uint256) {
        if (address(relayToken) == address(0)) return 0;
        uint256 balance = relayToken.balanceOf(user);
        if (balance >= TIER3_THRESHOLD) return 7_500; // 75%
        if (balance >= TIER2_THRESHOLD) return 5_000; // 50%
        if (balance >= TIER1_THRESHOLD) return 2_500; // 25%
        return 0;
    }

    function calculateFee(uint256 amount, address user) public view returns (uint256) {
        if (feeBps == 0) return 0;
        uint256 baseFee = (amount * feeBps) / 10_000;
        uint256 discount = getDiscountBps(user);
        if (discount == 0) return baseFee;
        return baseFee - (baseFee * discount) / 10_000;
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

        uint256 grossPayout = winningShares * 2;
        uint256 fee = calculateFee(grossPayout, user);
        payout = grossPayout - fee;

        MarketPool storage pool = marketPools[marketId];
        uint256 remainingEscrow = pool.escrow - pool.paidOut;
        if (remainingEscrow < grossPayout) revert InsufficientEscrow();
        pool.paidOut += grossPayout;

        position.yesShares = 0;
        position.noShares = 0;
        position.claimed = true;

        collateralVault.transferAvailable(address(this), user, payout);
        if (fee > 0) {
            accruedFees += fee;
        }
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
        uint256 grossPayout = winningShares * 2;
        return grossPayout - calculateFee(grossPayout, user);
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

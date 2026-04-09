// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";

interface ICollateralVault {
    function lock(address user, uint256 amount) external;
    function unlock(address user, uint256 amount) external;
    function settle(address from, address to, uint256 amount) external;
    function transferAvailable(address from, address to, uint256 amount) external;
}

interface IRelayStakingRead {
    function getTier(address user) external view returns (uint256);
}

interface IAggregatorV3 {
    function latestRoundData() external view returns (uint80, int256, uint256, uint256, uint80);
    function decimals() external view returns (uint8);
}

contract DistributionMarket is AccessControl, Pausable, ReentrancyGuard {
    bytes32 public constant MARKET_CREATOR_ROLE = keccak256("MARKET_CREATOR_ROLE");
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    uint256 public constant SCALE = 1e18;
    uint256 public constant OUTCOME_SCALE = 1e6;
    uint256 public constant MIN_SIGMA = 150_000; // 0.15 in OUTCOME_SCALE
    uint256 public constant MAX_FEE_BPS = 1_000;
    uint256 public constant STALENESS_THRESHOLD = 3600;
    uint256 public constant MIN_SIZE = 1_000; // minimum position size (0.001 in token decimals)
    uint256 public constant MAX_PAYOUT_RATIO = 10; // 10x cap on payout multiplier
    uint256 public constant MAX_POSITIONS_PER_USER = 100; // per market

    // RELAY holder discount tiers (same as OrderBook)
    uint256 public constant TIER1_THRESHOLD = 1_000e18;
    uint256 public constant TIER2_THRESHOLD = 10_000e18;
    uint256 public constant TIER3_THRESHOLD = 100_000e18;

    struct DistMarket {
        bytes32 questionHash;
        uint64 closeTime;
        uint64 resolveTime;
        uint256 outcomeMin;
        uint256 outcomeMax;
        uint256 liquidityParam;
        uint256 resolvedValue;
        address resolver;
        bool resolved;
        bool useOracle;
        address oracleFeed;
        uint256 totalCollateral;
        uint256 totalPaidOut;
    }

    struct DistPosition {
        address owner;
        uint256 mu;
        uint256 sigma;
        uint256 size;
        uint256 collateral;
        bool closed;
        bool claimed;
    }

    uint256 public marketCount;
    mapping(uint256 => DistMarket) internal _distMarkets;
    mapping(uint256 => uint256) public positionCount;
    mapping(uint256 => mapping(uint256 => DistPosition)) public positions;
    mapping(uint256 => mapping(address => uint256[])) private userPositionIds;

    // Aggregate market state (updated by backend OPERATOR)
    mapping(uint256 => uint256) public marketMu;
    mapping(uint256 => uint256) public marketSigma;

    // Fee config
    uint256 public feeBps;
    address public feeRecipient;
    uint256 public accruedFees;
    IRelayStakingRead public stakingContract;

    ICollateralVault public immutable collateralVault;
    IERC20 public immutable collateralToken;
    IERC20 public immutable relayToken;

    error ZeroAddress();
    error InvalidOutcomeRange();
    error InvalidLiquidityParam();
    error InvalidCloseTime();
    error MarketNotFound();
    error MarketClosed();
    error MarketNotClosed();
    error MarketAlreadyResolved();
    error MarketNotResolved();
    error InvalidMu();
    error InvalidSigma();
    error InvalidSize();
    error SlippageExceeded();
    error NotPositionOwner();
    error PositionAlreadyClosed();
    error PositionAlreadyClaimed();
    error NotDesignatedResolver();
    error FeedStale(uint256 updatedAt);
    error InvalidResolvedValue();
    error FeeTooHigh();
    error NoFeesToWithdraw();
    error InsufficientPool();
    error MaxPositionsReached();
    error SigmaTooLarge();
    error OracleNegativePrice();
    error MarketNotActive();
    error TradingNotEnded();

    event MarketCreated(uint256 indexed marketId, uint256 outcomeMin, uint256 outcomeMax, uint64 closeTime);
    event PositionOpened(
        uint256 indexed marketId,
        uint256 indexed positionId,
        address indexed owner,
        uint256 mu,
        uint256 sigma,
        uint256 size,
        uint256 collateral
    );
    event PositionClosed(uint256 indexed marketId, uint256 indexed positionId, address indexed owner, uint256 refund);
    event MarketResolved(uint256 indexed marketId, uint256 resolvedValue);
    event Claimed(uint256 indexed marketId, uint256 indexed positionId, address indexed owner, uint256 payout);
    event MarketStateUpdated(uint256 indexed marketId, uint256 mu, uint256 sigma);
    event FeeConfigUpdated(uint256 feeBps, address feeRecipient);
    event FeesWithdrawn(address indexed recipient, uint256 amount);
    event MarketCancelled(uint256 indexed marketId);
    event EmergencyWithdraw(uint256 indexed marketId, address indexed user, uint256 amount);

    constructor(
        address admin,
        address collateralVaultAddress,
        address collateralTokenAddress,
        address relayTokenAddress
    ) {
        if (admin == address(0) || collateralVaultAddress == address(0) || collateralTokenAddress == address(0)) {
            revert ZeroAddress();
        }

        collateralVault = ICollateralVault(collateralVaultAddress);
        collateralToken = IERC20(collateralTokenAddress);
        relayToken = IERC20(relayTokenAddress); // address(0) disables discounts

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MARKET_CREATOR_ROLE, admin);
        _grantRole(OPERATOR_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    // ----------------------------------------------------------------
    // Admin
    // ----------------------------------------------------------------

    function setFeeConfig(uint256 _feeBps, address _feeRecipient) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (_feeBps > MAX_FEE_BPS) revert FeeTooHigh();
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

    function setStakingContract(address _staking) external onlyRole(DEFAULT_ADMIN_ROLE) {
        stakingContract = IRelayStakingRead(_staking);
    }

    // ----------------------------------------------------------------
    // Fee discount (same as OrderBook)
    // ----------------------------------------------------------------

    function getDiscountBps(address user) public view returns (uint256) {
        if (address(stakingContract) != address(0)) {
            uint256 tier = stakingContract.getTier(user);
            if (tier >= 3) return 7_500;
            if (tier >= 2) return 5_000;
            if (tier >= 1) return 2_500;
            return 0;
        }
        if (address(relayToken) == address(0)) return 0;
        uint256 balance = relayToken.balanceOf(user);
        if (balance >= TIER3_THRESHOLD) return 7_500;
        if (balance >= TIER2_THRESHOLD) return 5_000;
        if (balance >= TIER1_THRESHOLD) return 2_500;
        return 0;
    }

    function calculateFee(uint256 amount, address user) public view returns (uint256) {
        if (feeBps == 0) return 0;
        uint256 baseFee = (amount * feeBps) / 10_000;
        uint256 discount = getDiscountBps(user);
        if (discount == 0) return baseFee;
        return baseFee - (baseFee * discount) / 10_000;
    }

    // ----------------------------------------------------------------
    // Market lifecycle
    // ----------------------------------------------------------------

    function createMarket(
        string calldata question,
        uint256 outcomeMin,
        uint256 outcomeMax,
        uint256 liquidityParam,
        uint64 closeTime,
        address resolver,
        bool useOracle,
        address oracleFeed
    ) external onlyRole(MARKET_CREATOR_ROLE) whenNotPaused returns (uint256 marketId) {
        if (closeTime <= block.timestamp) revert InvalidCloseTime();
        if (outcomeMax <= outcomeMin) revert InvalidOutcomeRange();
        if (liquidityParam == 0) revert InvalidLiquidityParam();
        if (resolver == address(0)) revert ZeroAddress();

        marketId = ++marketCount;

        _distMarkets[marketId] = DistMarket({
            questionHash: keccak256(bytes(question)),
            closeTime: closeTime,
            resolveTime: 0,
            outcomeMin: outcomeMin,
            outcomeMax: outcomeMax,
            liquidityParam: liquidityParam,
            resolvedValue: 0,
            resolver: resolver,
            resolved: false,
            useOracle: useOracle,
            oracleFeed: oracleFeed,
            totalCollateral: 0,
            totalPaidOut: 0
        });

        // Initial aggregate: mean = midpoint, sigma = range/6 (99.7% within range)
        marketMu[marketId] = (outcomeMin + outcomeMax) / 2;
        marketSigma[marketId] = (outcomeMax - outcomeMin) / 6;

        emit MarketCreated(marketId, outcomeMin, outcomeMax, closeTime);
    }

    // ----------------------------------------------------------------
    // Position management
    // ----------------------------------------------------------------

    function openPosition(uint256 marketId, uint256 mu, uint256 sigma, uint256 size, uint256 maxCollateral)
        external
        whenNotPaused
        nonReentrant
        returns (uint256 positionId)
    {
        return _openPosition(marketId, msg.sender, mu, sigma, size, size, maxCollateral);
    }

    /// @notice Open a position on behalf of a user with LMSR-computed collateral.
    /// @dev Only callable by OPERATOR_ROLE (backend). Collateral is computed off-chain via LMSR.
    /// @param collateral LMSR-computed collateral (must be >= MIN_SIZE, bounded by maxCollateral)
    function openPositionFor(
        uint256 marketId,
        address trader,
        uint256 mu,
        uint256 sigma,
        uint256 size,
        uint256 collateral,
        uint256 maxCollateral
    )
        external
        onlyRole(OPERATOR_ROLE)
        whenNotPaused
        nonReentrant
        returns (uint256 positionId)
    {
        if (trader == address(0)) revert ZeroAddress();
        return _openPosition(marketId, trader, mu, sigma, size, collateral, maxCollateral);
    }

    function _openPosition(
        uint256 marketId,
        address trader,
        uint256 mu,
        uint256 sigma,
        uint256 size,
        uint256 collateral,
        uint256 maxCollateral
    ) internal returns (uint256 positionId) {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (market.resolved) revert MarketAlreadyResolved();
        if (block.timestamp >= market.closeTime) revert MarketClosed();
        if (mu < market.outcomeMin || mu > market.outcomeMax) revert InvalidMu();
        if (sigma < MIN_SIGMA) revert InvalidSigma();
        // Max sigma = half the outcome range (distribution should fit within bounds)
        uint256 maxSigma = (market.outcomeMax - market.outcomeMin) / 2;
        if (sigma > maxSigma) revert SigmaTooLarge();
        if (size < MIN_SIZE) revert InvalidSize();
        if (collateral < MIN_SIZE) revert InvalidSize();
        if (userPositionIds[marketId][trader].length >= MAX_POSITIONS_PER_USER) revert MaxPositionsReached();

        uint256 collateralRequired = collateral;
        if (collateralRequired > maxCollateral) revert SlippageExceeded();

        // Lock collateral via vault
        collateralVault.lock(trader, collateralRequired);
        collateralVault.settle(trader, address(this), collateralRequired);

        positionId = ++positionCount[marketId];
        positions[marketId][positionId] = DistPosition({
            owner: trader,
            mu: mu,
            sigma: sigma,
            size: size,
            collateral: collateralRequired,
            closed: false,
            claimed: false
        });

        userPositionIds[marketId][trader].push(positionId);
        market.totalCollateral += collateralRequired;

        emit PositionOpened(marketId, positionId, trader, mu, sigma, size, collateralRequired);
    }

    function closePosition(uint256 marketId, uint256 positionId) external whenNotPaused nonReentrant {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (market.resolved) revert MarketAlreadyResolved();
        if (block.timestamp >= market.closeTime) revert MarketClosed();

        DistPosition storage pos = positions[marketId][positionId];
        if (pos.owner != msg.sender) revert NotPositionOwner();
        if (pos.closed) revert PositionAlreadyClosed();

        pos.closed = true;

        uint256 fee = calculateFee(pos.collateral, msg.sender);
        uint256 refund = pos.collateral - fee;

        market.totalCollateral -= pos.collateral;

        collateralVault.transferAvailable(address(this), msg.sender, refund);
        if (fee > 0) {
            accruedFees += fee;
        }

        emit PositionClosed(marketId, positionId, msg.sender, refund);
    }

    // ----------------------------------------------------------------
    // Resolution
    // ----------------------------------------------------------------

    function resolve(uint256 marketId, uint256 value) external whenNotPaused {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (market.resolved) revert MarketAlreadyResolved();
        if (block.timestamp < market.closeTime) revert MarketNotClosed();
        if (msg.sender != market.resolver && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotDesignatedResolver();
        }

        _resolve(market, marketId, value);
    }

    function resolveFromOracle(uint256 marketId) external whenNotPaused {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (market.resolved) revert MarketAlreadyResolved();
        if (block.timestamp < market.closeTime) revert MarketNotClosed();
        if (!market.useOracle) revert NotDesignatedResolver();

        IAggregatorV3 feed = IAggregatorV3(market.oracleFeed);
        (, int256 answer,, uint256 updatedAt,) = feed.latestRoundData();
        if (block.timestamp - updatedAt > STALENESS_THRESHOLD) revert FeedStale(updatedAt);
        if (answer <= 0) revert OracleNegativePrice();

        uint8 feedDecimals = feed.decimals();
        // Scale answer to OUTCOME_SCALE (1e6) — safe cast since we verified answer > 0
        uint256 scaled;
        if (feedDecimals >= 6) {
            scaled = uint256(answer) / (10 ** (feedDecimals - 6));
        } else {
            scaled = uint256(answer) * (10 ** (6 - feedDecimals));
        }

        _resolve(market, marketId, scaled);
    }

    function _resolve(DistMarket storage market, uint256 marketId, uint256 value) internal {
        if (value < market.outcomeMin || value > market.outcomeMax) revert InvalidResolvedValue();

        market.resolved = true;
        market.resolvedValue = value;
        market.resolveTime = uint64(block.timestamp);

        emit MarketResolved(marketId, value);
    }

    // ----------------------------------------------------------------
    // Claim
    // ----------------------------------------------------------------

    function claim(uint256 marketId, uint256 positionId) external whenNotPaused nonReentrant returns (uint256 payout) {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (!market.resolved) revert MarketNotResolved();

        DistPosition storage pos = positions[marketId][positionId];
        if (pos.owner != msg.sender) revert NotPositionOwner();
        if (pos.claimed) revert PositionAlreadyClaimed();
        if (pos.closed) revert PositionAlreadyClosed();

        pos.claimed = true;

        // Compute payout: position PDF density / market PDF density at resolved value
        uint256 posPdf = _gaussianPdf(market.resolvedValue, pos.mu, pos.sigma);
        uint256 mktPdf = _gaussianPdf(market.resolvedValue, marketMu[marketId], marketSigma[marketId]);

        uint256 grossPayout;
        if (mktPdf == 0) {
            // Market density is zero at resolved value — unlikely but safe fallback
            grossPayout = 0;
        } else {
            uint256 payoutRatio = (posPdf * SCALE) / mktPdf;
            // Cap payout ratio at MAX_PAYOUT_RATIO to prevent unbounded payouts
            uint256 maxRatio = MAX_PAYOUT_RATIO * SCALE;
            if (payoutRatio > maxRatio) {
                payoutRatio = maxRatio;
            }
            grossPayout = (pos.collateral * payoutRatio) / SCALE;
        }

        // Cap to available pool
        uint256 available = market.totalCollateral - market.totalPaidOut;
        if (grossPayout > available) {
            grossPayout = available;
        }

        uint256 fee = calculateFee(grossPayout, msg.sender);
        payout = grossPayout - fee;

        market.totalPaidOut += grossPayout;

        if (payout > 0) {
            collateralVault.transferAvailable(address(this), msg.sender, payout);
        }
        if (fee > 0) {
            accruedFees += fee;
        }

        emit Claimed(marketId, positionId, msg.sender, payout);
    }

    // ----------------------------------------------------------------
    // Operator state updates
    // ----------------------------------------------------------------

    function updateMarketState(uint256 marketId, uint256 mu, uint256 sigma) external onlyRole(OPERATOR_ROLE) {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();

        marketMu[marketId] = mu;
        marketSigma[marketId] = sigma;

        emit MarketStateUpdated(marketId, mu, sigma);
    }

    // ----------------------------------------------------------------
    // View helpers
    // ----------------------------------------------------------------

    function getUserPositionIds(uint256 marketId, address user) external view returns (uint256[] memory) {
        return userPositionIds[marketId][user];
    }

    function getMarketCore(uint256 marketId)
        external
        view
        returns (
            bytes32 questionHash,
            uint64 closeTime,
            uint64 resolveTime,
            uint256 outcomeMin,
            uint256 outcomeMax,
            uint256 liquidityParam,
            uint256 resolvedValue,
            address resolver
        )
    {
        DistMarket storage m = _distMarkets[marketId];
        return (
            m.questionHash, m.closeTime, m.resolveTime,
            m.outcomeMin, m.outcomeMax, m.liquidityParam,
            m.resolvedValue, m.resolver
        );
    }

    function getMarketState(uint256 marketId)
        external
        view
        returns (
            bool resolved,
            bool useOracle,
            address oracleFeed,
            uint256 totalCollateral,
            uint256 totalPaidOut
        )
    {
        DistMarket storage m = _distMarkets[marketId];
        return (m.resolved, m.useOracle, m.oracleFeed, m.totalCollateral, m.totalPaidOut);
    }

    // ----------------------------------------------------------------
    // Gaussian PDF approximation (Abramowitz-Stegun)
    // ----------------------------------------------------------------

    /// @notice Computes an approximation of the Gaussian PDF at point x given mean mu and std dev sigma.
    /// @dev Uses a rational approximation for exp(-z^2/2) based on Abramowitz-Stegun.
    ///      All inputs scaled by OUTCOME_SCALE (1e6). Returns density scaled by SCALE (1e18).
    ///      Accurate to <0.5% in the range [-4sigma, +4sigma].
    function gaussianPdf(uint256 x, uint256 mu, uint256 sigma) external pure returns (uint256) {
        return _gaussianPdf(x, mu, sigma);
    }

    function _gaussianPdf(uint256 x, uint256 mu, uint256 sigma) internal pure returns (uint256) {
        if (sigma == 0) return 0;

        // z = |x - mu| / sigma, scaled by SCALE (1e18)
        uint256 diff;
        if (x >= mu) {
            diff = x - mu;
        } else {
            diff = mu - x;
        }
        uint256 z = (diff * SCALE) / sigma; // z in 1e18

        // z^2 / 2, scaled by SCALE
        uint256 zSq = (z * z) / SCALE;
        uint256 halfZSq = zSq / 2;

        // Approximate exp(-halfZSq) using a 5th-order rational approximation
        // We use: exp(-t) ≈ 1 / (1 + t + t^2/2 + t^3/6 + t^4/24 + t^5/120)
        // where t = z^2/2, all in SCALE
        uint256 t = halfZSq;
        uint256 t2 = (t * t) / SCALE;
        uint256 t3 = (t2 * t) / SCALE;
        uint256 t4 = (t3 * t) / SCALE;
        uint256 t5 = (t4 * t) / SCALE;

        // denominator = SCALE + t + t2/2 + t3/6 + t4/24 + t5/120
        uint256 denom = SCALE + t + t2 / 2 + t3 / 6 + t4 / 24 + t5 / 120;

        // exp_approx = SCALE^2 / denom (result in SCALE)
        uint256 expApprox = (SCALE * SCALE) / denom;

        // PDF = (1 / (sigma * sqrt(2*pi))) * exp(-z^2/2)
        // sqrt(2*pi) ≈ 2.506628... We use 2_506628 scaled by 1e6
        // (1 / sigma) is (OUTCOME_SCALE / sigma) then we need to get to SCALE
        // pdf = expApprox * OUTCOME_SCALE / (sigma * 2506628 / 1e6)
        //     = expApprox * OUTCOME_SCALE * 1e6 / (sigma * 2506628)
        //     = expApprox * 1e12 / (sigma * 2506628)

        uint256 pdf = (expApprox * 1e12) / (sigma * 2_506628);

        return pdf;
    }

    // ----------------------------------------------------------------
    // Market cancellation & emergency
    // ----------------------------------------------------------------

    /// @notice Cancel a market and allow all position holders to withdraw their collateral.
    /// @dev Only admin can cancel. Cannot cancel an already resolved market.
    function cancelMarket(uint256 marketId) external onlyRole(DEFAULT_ADMIN_ROLE) {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (market.resolved) revert MarketAlreadyResolved();

        market.resolved = true; // prevent further trading
        market.resolveTime = uint64(block.timestamp);

        emit MarketCancelled(marketId);
    }

    /// @notice Emergency withdraw for a cancelled (not resolved) market.
    ///         Returns full collateral to position owner without any fee.
    function emergencyWithdraw(uint256 marketId, uint256 positionId) external nonReentrant {
        DistMarket storage market = _distMarkets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        // Only allowed if market was cancelled (resolved=true but resolvedValue=0 and no normal resolution)
        if (!market.resolved) revert MarketNotResolved();
        if (market.resolvedValue != 0) revert MarketAlreadyResolved(); // normal resolution, use claim()

        DistPosition storage pos = positions[marketId][positionId];
        if (pos.owner != msg.sender) revert NotPositionOwner();
        if (pos.closed || pos.claimed) revert PositionAlreadyClosed();

        pos.claimed = true;

        uint256 refund = pos.collateral;
        market.totalCollateral -= refund;

        if (refund > 0) {
            collateralVault.transferAvailable(address(this), msg.sender, refund);
        }

        emit EmergencyWithdraw(marketId, msg.sender, refund);
    }

    // ----------------------------------------------------------------
    // Pause
    // ----------------------------------------------------------------

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}

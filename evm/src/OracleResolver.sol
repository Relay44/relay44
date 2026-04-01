// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";

interface IAggregatorV3 {
    function latestRoundData()
        external
        view
        returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);

    function decimals() external view returns (uint8);
}

interface IMarketCore {
    function resolveMarket(uint256 marketId, bool outcome) external;

    function markets(uint256 marketId)
        external
        view
        returns (bytes32 questionHash, uint64 closeTime, uint64 resolveTime, address resolver, bool resolved, bool outcome);
}

/// @title OracleResolver
/// @notice Deployed once on Base. Set as the `resolver` on MarketCore markets that
///         use oracle-based resolution. Reads Chainlink price feeds and evaluates
///         a threshold condition to determine market outcome.
///         resolve() is permissionless — anyone can trigger resolution once
///         the market is closed and the oracle condition can be evaluated.
contract OracleResolver is AccessControl {
    bytes32 public constant CONFIGURATOR_ROLE = keccak256("CONFIGURATOR_ROLE");

    uint256 public constant STALENESS_THRESHOLD = 3600; // 1 hour

    enum FeedType {
        MANUAL,
        CHAINLINK
    }

    enum Comparison {
        GT,
        GTE,
        LT,
        LTE,
        EQ
    }

    struct OracleConfig {
        FeedType feedType;
        address feedAddress;
        Comparison comparison;
        int256 targetValue;
        bool configured;
    }

    IMarketCore public immutable marketCore;

    mapping(uint256 => OracleConfig) public oracleConfigs;

    error NotConfigured(uint256 marketId);
    error MarketNotClosed();
    error MarketAlreadyResolved();
    error FeedStale(uint256 updatedAt, uint256 threshold);
    error ManualOnly();
    error ZeroAddress();
    error AlreadyConfigured(uint256 marketId);

    event OracleConfigured(
        uint256 indexed marketId, FeedType feedType, address feedAddress, uint8 comparison, int256 targetValue
    );
    event MarketResolvedByOracle(uint256 indexed marketId, bool outcome, int256 feedAnswer, address caller);
    event MarketResolvedManually(uint256 indexed marketId, bool outcome, address caller);

    constructor(address admin, address marketCoreAddress) {
        if (admin == address(0) || marketCoreAddress == address(0)) revert ZeroAddress();
        marketCore = IMarketCore(marketCoreAddress);
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(CONFIGURATOR_ROLE, admin);
    }

    /// @notice Configure oracle settings for a market. Called after market creation.
    ///         Cannot reconfigure once set — deploy a new market instead.
    function configureOracle(
        uint256 marketId,
        FeedType feedType,
        address feedAddress,
        Comparison comparison,
        int256 targetValue
    ) external onlyRole(CONFIGURATOR_ROLE) {
        if (oracleConfigs[marketId].configured) revert AlreadyConfigured(marketId);
        if (feedType != FeedType.MANUAL && feedAddress == address(0)) revert ZeroAddress();

        oracleConfigs[marketId] = OracleConfig({
            feedType: feedType,
            feedAddress: feedAddress,
            comparison: comparison,
            targetValue: targetValue,
            configured: true
        });

        emit OracleConfigured(marketId, feedType, feedAddress, uint8(comparison), targetValue);
    }

    /// @notice Permissionless resolution for Chainlink-backed markets.
    ///         Anyone can call once the market is past closeTime and the feed is fresh.
    function resolve(uint256 marketId) external {
        OracleConfig storage cfg = oracleConfigs[marketId];
        if (!cfg.configured) revert NotConfigured(marketId);
        if (cfg.feedType == FeedType.MANUAL) revert ManualOnly();

        (, uint64 closeTime,,, bool resolved,) = marketCore.markets(marketId);
        if (block.timestamp < closeTime) revert MarketNotClosed();
        if (resolved) revert MarketAlreadyResolved();

        (, int256 answer,, uint256 updatedAt,) = IAggregatorV3(cfg.feedAddress).latestRoundData();
        if (block.timestamp - updatedAt > STALENESS_THRESHOLD) {
            revert FeedStale(updatedAt, STALENESS_THRESHOLD);
        }

        bool outcome = _evaluate(answer, cfg.comparison, cfg.targetValue);
        marketCore.resolveMarket(marketId, outcome);

        emit MarketResolvedByOracle(marketId, outcome, answer, msg.sender);
    }

    /// @notice Manual resolution fallback for MANUAL-type markets.
    ///         Requires CONFIGURATOR_ROLE.
    function resolveManual(uint256 marketId, bool outcome) external onlyRole(CONFIGURATOR_ROLE) {
        OracleConfig storage cfg = oracleConfigs[marketId];
        if (!cfg.configured) revert NotConfigured(marketId);
        if (cfg.feedType != FeedType.MANUAL) revert NotConfigured(marketId);

        (, uint64 closeTime,,, bool resolved,) = marketCore.markets(marketId);
        if (block.timestamp < closeTime) revert MarketNotClosed();
        if (resolved) revert MarketAlreadyResolved();

        marketCore.resolveMarket(marketId, outcome);

        emit MarketResolvedManually(marketId, outcome, msg.sender);
    }

    function _evaluate(int256 answer, Comparison comparison, int256 target) internal pure returns (bool) {
        if (comparison == Comparison.GT) return answer > target;
        if (comparison == Comparison.GTE) return answer >= target;
        if (comparison == Comparison.LT) return answer < target;
        if (comparison == Comparison.LTE) return answer <= target;
        return answer == target; // EQ
    }
}

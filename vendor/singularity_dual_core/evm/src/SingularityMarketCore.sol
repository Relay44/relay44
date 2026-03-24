// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";

contract SingularityMarketCore is RoleAuth {
    error MarketNotFound();
    error InvalidState();
    error InvalidOutcome();
    error InvalidTimestamp();

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant ORACLE_ROLE = keccak256("ORACLE_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    enum MarketState {
        Paused,
        Active,
        Closed,
        Resolved,
        Cancelled
    }

    struct Market {
        address creator;
        string question;
        bytes32 metadataHash;
        uint64 tradingEnd;
        uint64 resolutionDeadline;
        uint8 resolvedOutcome;
        MarketState state;
        uint256 yesTokenId;
        uint256 noTokenId;
        bool exists;
    }

    event MarketCreated(
        uint256 indexed marketId,
        address indexed creator,
        uint64 tradingEnd,
        uint64 resolutionDeadline,
        bytes32 metadataHash,
        uint256 yesTokenId,
        uint256 noTokenId
    );

    event MarketStateUpdated(uint256 indexed marketId, MarketState state, address indexed actor);

    event MarketResolved(
        uint256 indexed marketId,
        uint8 indexed outcome,
        address indexed resolver,
        bytes32 evidenceHash,
        string oracleSource
    );

    mapping(uint256 => Market) public markets;
    uint256 public nextMarketId = 1;

    constructor(address admin) RoleAuth(admin) {}

    function createMarket(
        address creator,
        string calldata question,
        bytes32 metadataHash,
        uint64 tradingEnd,
        uint64 resolutionDeadline
    ) external onlyRole(OPERATOR_ROLE) returns (uint256 marketId) {
        if (creator == address(0)) revert InvalidAddress();
        if (bytes(question).length == 0) revert InvalidState();
        if (tradingEnd <= block.timestamp) revert InvalidTimestamp();
        if (resolutionDeadline <= tradingEnd) revert InvalidTimestamp();

        marketId = nextMarketId;
        nextMarketId += 1;

        uint256 yesTokenId = marketId * 2;
        uint256 noTokenId = yesTokenId + 1;

        markets[marketId] = Market({
            creator: creator,
            question: question,
            metadataHash: metadataHash,
            tradingEnd: tradingEnd,
            resolutionDeadline: resolutionDeadline,
            resolvedOutcome: 2,
            state: MarketState.Active,
            yesTokenId: yesTokenId,
            noTokenId: noTokenId,
            exists: true
        });

        emit MarketCreated(
            marketId,
            creator,
            tradingEnd,
            resolutionDeadline,
            metadataHash,
            yesTokenId,
            noTokenId
        );
    }

    function pauseMarket(uint256 marketId) external onlyRole(PAUSER_ROLE) {
        Market storage market = _requireMarket(marketId);
        if (market.state == MarketState.Resolved || market.state == MarketState.Cancelled) revert InvalidState();
        market.state = MarketState.Paused;
        emit MarketStateUpdated(marketId, market.state, msg.sender);
    }

    function resumeMarket(uint256 marketId) external onlyRole(OPERATOR_ROLE) {
        Market storage market = _requireMarket(marketId);
        if (market.state != MarketState.Paused) revert InvalidState();
        market.state = MarketState.Active;
        emit MarketStateUpdated(marketId, market.state, msg.sender);
    }

    function closeMarket(uint256 marketId) external onlyRole(OPERATOR_ROLE) {
        Market storage market = _requireMarket(marketId);
        if (market.state == MarketState.Resolved || market.state == MarketState.Cancelled) revert InvalidState();
        market.state = MarketState.Closed;
        emit MarketStateUpdated(marketId, market.state, msg.sender);
    }

    function cancelMarket(uint256 marketId) external onlyRole(OPERATOR_ROLE) {
        Market storage market = _requireMarket(marketId);
        if (market.state == MarketState.Resolved) revert InvalidState();
        market.state = MarketState.Cancelled;
        emit MarketStateUpdated(marketId, market.state, msg.sender);
    }

    function resolveMarket(
        uint256 marketId,
        uint8 outcome,
        address resolver,
        bytes32 evidenceHash,
        string calldata oracleSource
    ) external onlyRole(ORACLE_ROLE) {
        if (outcome > 1) revert InvalidOutcome();
        if (resolver == address(0)) revert InvalidAddress();

        Market storage market = _requireMarket(marketId);
        if (market.state == MarketState.Resolved || market.state == MarketState.Cancelled) revert InvalidState();

        market.state = MarketState.Resolved;
        market.resolvedOutcome = outcome;

        emit MarketResolved(marketId, outcome, resolver, evidenceHash, oracleSource);
        emit MarketStateUpdated(marketId, market.state, msg.sender);
    }

    function isTradable(uint256 marketId) external view returns (bool) {
        Market storage market = markets[marketId];
        if (!market.exists) return false;
        if (market.state != MarketState.Active) return false;
        return block.timestamp < market.tradingEnd;
    }

    function getOutcomeTokenIds(uint256 marketId) external view returns (uint256 yesTokenId, uint256 noTokenId) {
        Market storage market = _requireMarketView(marketId);
        return (market.yesTokenId, market.noTokenId);
    }

    function isResolved(uint256 marketId) external view returns (bool resolved, uint8 resolvedOutcome) {
        Market storage market = _requireMarketView(marketId);
        return (market.state == MarketState.Resolved, market.resolvedOutcome);
    }

    function marketState(uint256 marketId) external view returns (MarketState) {
        Market storage market = _requireMarketView(marketId);
        return market.state;
    }

    function _requireMarket(uint256 marketId) internal view returns (Market storage market) {
        market = markets[marketId];
        if (!market.exists) revert MarketNotFound();
    }

    function _requireMarketView(uint256 marketId) internal view returns (Market storage market) {
        market = markets[marketId];
        if (!market.exists) revert MarketNotFound();
    }
}

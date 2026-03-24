// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface ISingularityMarketCore {
    function isTradable(uint256 marketId) external view returns (bool);
    function getOutcomeTokenIds(uint256 marketId) external view returns (uint256 yesTokenId, uint256 noTokenId);
    function isResolved(uint256 marketId) external view returns (bool resolved, uint8 resolvedOutcome);
}

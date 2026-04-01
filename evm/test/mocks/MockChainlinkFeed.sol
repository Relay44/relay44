// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

/// @notice Minimal mock of a Chainlink AggregatorV3 price feed for testing.
contract MockChainlinkFeed {
    int256 public price;
    uint8 public decimals_;
    uint256 public updatedAt;
    uint80 public roundId;

    constructor(int256 _price, uint8 _decimals) {
        price = _price;
        decimals_ = _decimals;
        updatedAt = block.timestamp;
        roundId = 1;
    }

    function setPrice(int256 _price) external {
        price = _price;
        updatedAt = block.timestamp;
        roundId++;
    }

    function setUpdatedAt(uint256 _updatedAt) external {
        updatedAt = _updatedAt;
    }

    function decimals() external view returns (uint8) {
        return decimals_;
    }

    function latestRoundData()
        external
        view
        returns (uint80, int256, uint256, uint256, uint80)
    {
        return (roundId, price, block.timestamp, updatedAt, roundId);
    }
}

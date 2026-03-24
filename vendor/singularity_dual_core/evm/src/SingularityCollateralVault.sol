// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { IERC20Minimal } from "./interfaces/IERC20Minimal.sol";
import { RoleAuth } from "./shared/RoleAuth.sol";

contract SingularityCollateralVault is RoleAuth {
    error TransferFailed();
    error InvalidTreasury();

    event TreasuryUpdated(address indexed previousTreasury, address indexed newTreasury);
    event CollateralCollected(address indexed from, uint256 amount, bytes32 indexed reason);
    event CollateralRefunded(address indexed to, uint256 amount, bytes32 indexed reason);
    event CollateralPayout(address indexed to, uint256 grossAmount, uint256 netAmount, uint256 feeAmount);

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    uint16 public constant SETTLEMENT_FEE_BPS = 50;

    IERC20Minimal public immutable collateralToken;
    address public treasury;
    uint256 public totalFeesCollected;

    constructor(address admin, address collateralTokenAddress, address treasuryAddress) RoleAuth(admin) {
        if (collateralTokenAddress == address(0)) revert InvalidAddress();
        if (treasuryAddress == address(0)) revert InvalidTreasury();
        collateralToken = IERC20Minimal(collateralTokenAddress);
        treasury = treasuryAddress;
    }

    function setTreasury(address newTreasury) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (newTreasury == address(0)) revert InvalidTreasury();
        address previous = treasury;
        treasury = newTreasury;
        emit TreasuryUpdated(previous, newTreasury);
    }

    function collectFrom(address from, uint256 amount, bytes32 reason) external onlyRole(OPERATOR_ROLE) {
        if (amount == 0) return;
        if (!collateralToken.transferFrom(from, address(this), amount)) revert TransferFailed();
        emit CollateralCollected(from, amount, reason);
    }

    function refundTo(address to, uint256 amount, bytes32 reason) external onlyRole(OPERATOR_ROLE) {
        if (amount == 0) return;
        if (!collateralToken.transfer(to, amount)) revert TransferFailed();
        emit CollateralRefunded(to, amount, reason);
    }

    function payoutTo(address to, uint256 grossAmount)
        external
        onlyRole(OPERATOR_ROLE)
        returns (uint256 netAmount, uint256 feeAmount)
    {
        if (grossAmount == 0) return (0, 0);

        feeAmount = (grossAmount * SETTLEMENT_FEE_BPS) / 10_000;
        netAmount = grossAmount - feeAmount;

        if (netAmount > 0 && !collateralToken.transfer(to, netAmount)) revert TransferFailed();
        if (feeAmount > 0 && !collateralToken.transfer(treasury, feeAmount)) revert TransferFailed();

        totalFeesCollected += feeAmount;
        emit CollateralPayout(to, grossAmount, netAmount, feeAmount);
    }

    function availableCollateral() external view returns (uint256) {
        return collateralToken.balanceOf(address(this));
    }
}

// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

contract CollateralVault is AccessControl, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    IERC20 public immutable collateral;

    mapping(address => uint256) public availableBalance;
    mapping(address => uint256) public lockedBalance;

    error ZeroAddress();
    error InvalidAmount();
    error InsufficientAvailable();
    error InsufficientLocked();

    event Deposited(address indexed user, uint256 amount);
    event Withdrawn(address indexed user, uint256 amount);
    event Locked(address indexed user, uint256 amount);
    event Unlocked(address indexed user, uint256 amount);
    event Settled(address indexed from, address indexed to, uint256 amount);
    event AvailableTransferred(address indexed from, address indexed to, uint256 amount);

    constructor(address admin, address collateralToken) {
        if (admin == address(0) || collateralToken == address(0)) revert ZeroAddress();

        collateral = IERC20(collateralToken);

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(OPERATOR_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    function deposit(uint256 amount) external nonReentrant whenNotPaused {
        if (amount == 0) revert InvalidAmount();

        collateral.safeTransferFrom(msg.sender, address(this), amount);
        availableBalance[msg.sender] += amount;

        emit Deposited(msg.sender, amount);
    }

    function withdraw(uint256 amount) external nonReentrant whenNotPaused {
        if (amount == 0) revert InvalidAmount();
        if (availableBalance[msg.sender] < amount) revert InsufficientAvailable();

        availableBalance[msg.sender] -= amount;
        collateral.safeTransfer(msg.sender, amount);

        emit Withdrawn(msg.sender, amount);
    }

    function lock(address user, uint256 amount) external onlyRole(OPERATOR_ROLE) whenNotPaused {
        if (user == address(0)) revert ZeroAddress();
        if (amount == 0) revert InvalidAmount();
        if (availableBalance[user] < amount) revert InsufficientAvailable();

        availableBalance[user] -= amount;
        lockedBalance[user] += amount;

        emit Locked(user, amount);
    }

    function unlock(address user, uint256 amount) external onlyRole(OPERATOR_ROLE) whenNotPaused {
        if (user == address(0)) revert ZeroAddress();
        if (amount == 0) revert InvalidAmount();
        if (lockedBalance[user] < amount) revert InsufficientLocked();

        lockedBalance[user] -= amount;
        availableBalance[user] += amount;

        emit Unlocked(user, amount);
    }

    function settle(address from, address to, uint256 amount) external onlyRole(OPERATOR_ROLE) whenNotPaused {
        if (from == address(0) || to == address(0)) revert ZeroAddress();
        if (amount == 0) revert InvalidAmount();
        if (lockedBalance[from] < amount) revert InsufficientLocked();

        lockedBalance[from] -= amount;
        availableBalance[to] += amount;

        emit Settled(from, to, amount);
    }

    function transferAvailable(address from, address to, uint256 amount)
        external
        onlyRole(OPERATOR_ROLE)
        whenNotPaused
    {
        if (from == address(0) || to == address(0)) revert ZeroAddress();
        if (amount == 0) revert InvalidAmount();
        if (availableBalance[from] < amount) revert InsufficientAvailable();

        availableBalance[from] -= amount;
        availableBalance[to] += amount;

        emit AvailableTransferred(from, to, amount);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}

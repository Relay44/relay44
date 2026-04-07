// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title RelayStaking - Stake RELAY for tier benefits and reward distribution
/// @notice Users lock RELAY to gain platform benefits (fee discounts, agent tiers, governance weight).
///         Rewards are distributed per-epoch by the RewardDistributor.
contract RelayStaking is AccessControl, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant DISTRIBUTOR_ROLE = keccak256("DISTRIBUTOR_ROLE");

    uint256 public constant MIN_LOCK_DURATION = 7 days;
    uint256 public constant MAX_LOCK_DURATION = 365 days;

    struct Stake {
        uint256 amount;
        uint64 lockedAt;
        uint64 unlockAt;
        uint256 rewardDebt; // For per-share reward accounting
    }

    IERC20 public immutable relayToken;

    mapping(address => Stake) public stakes;
    uint256 public totalStaked;

    // Reward accounting (token-per-share model)
    uint256 public accRewardPerShare; // Scaled by 1e18
    uint256 public pendingRewards;

    error InvalidAmount();
    error InvalidDuration();
    error StakeStillLocked();
    error NoStake();
    error AlreadyStaked();

    event Staked(address indexed user, uint256 amount, uint64 unlockAt);
    event Unstaked(address indexed user, uint256 amount);
    event RewardsClaimed(address indexed user, uint256 amount);
    event RewardsDeposited(uint256 amount, uint256 newAccRewardPerShare);
    event StakeExtended(address indexed user, uint64 newUnlockAt);

    constructor(address admin, address relayTokenAddress) {
        if (admin == address(0) || relayTokenAddress == address(0)) revert InvalidAmount();

        relayToken = IERC20(relayTokenAddress);
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    /// @notice Stake RELAY tokens with a lock duration
    function stake(uint256 amount, uint64 lockDuration) external nonReentrant whenNotPaused {
        if (amount == 0) revert InvalidAmount();
        if (lockDuration < MIN_LOCK_DURATION || lockDuration > MAX_LOCK_DURATION) revert InvalidDuration();
        if (stakes[msg.sender].amount > 0) revert AlreadyStaked();

        relayToken.safeTransferFrom(msg.sender, address(this), amount);

        uint64 unlockAt = uint64(block.timestamp) + lockDuration;
        stakes[msg.sender] = Stake({
            amount: amount,
            lockedAt: uint64(block.timestamp),
            unlockAt: unlockAt,
            rewardDebt: (amount * accRewardPerShare) / 1e18
        });
        totalStaked += amount;

        emit Staked(msg.sender, amount, unlockAt);
    }

    /// @notice Unstake after lock period expires
    function unstake() external nonReentrant whenNotPaused {
        Stake storage s = stakes[msg.sender];
        if (s.amount == 0) revert NoStake();
        if (block.timestamp < s.unlockAt) revert StakeStillLocked();

        uint256 amount = s.amount;
        uint256 reward = _pendingReward(msg.sender);

        totalStaked -= amount;
        delete stakes[msg.sender];

        relayToken.safeTransfer(msg.sender, amount);
        if (reward > 0) {
            relayToken.safeTransfer(msg.sender, reward);
            emit RewardsClaimed(msg.sender, reward);
        }

        emit Unstaked(msg.sender, amount);
    }

    /// @notice Claim accumulated rewards without unstaking
    function claimRewards() external nonReentrant whenNotPaused {
        Stake storage s = stakes[msg.sender];
        if (s.amount == 0) revert NoStake();

        uint256 reward = _pendingReward(msg.sender);
        if (reward == 0) revert InvalidAmount();

        s.rewardDebt = (s.amount * accRewardPerShare) / 1e18;
        relayToken.safeTransfer(msg.sender, reward);

        emit RewardsClaimed(msg.sender, reward);
    }

    /// @notice Extend lock duration (cannot shorten)
    function extendLock(uint64 newUnlockAt) external whenNotPaused {
        Stake storage s = stakes[msg.sender];
        if (s.amount == 0) revert NoStake();
        if (newUnlockAt <= s.unlockAt) revert InvalidDuration();
        if (newUnlockAt > uint64(block.timestamp) + uint64(MAX_LOCK_DURATION)) revert InvalidDuration();

        s.unlockAt = newUnlockAt;
        emit StakeExtended(msg.sender, newUnlockAt);
    }

    /// @notice Deposit rewards for distribution to stakers. Called by RewardDistributor.
    function depositRewards(uint256 amount) external onlyRole(DISTRIBUTOR_ROLE) {
        if (amount == 0) revert InvalidAmount();
        if (totalStaked == 0) {
            pendingRewards += amount;
            return;
        }

        relayToken.safeTransferFrom(msg.sender, address(this), amount);

        uint256 totalToDistribute = amount + pendingRewards;
        pendingRewards = 0;
        accRewardPerShare += (totalToDistribute * 1e18) / totalStaked;

        emit RewardsDeposited(totalToDistribute, accRewardPerShare);
    }

    // --- Views ---

    function pendingRewardOf(address user) external view returns (uint256) {
        return _pendingReward(user);
    }

    function stakeOf(address user) external view returns (uint256 amount, uint64 unlockAt) {
        Stake memory s = stakes[user];
        return (s.amount, s.unlockAt);
    }

    function getTier(address user) external view returns (uint256) {
        uint256 amount = stakes[user].amount;
        if (amount >= 100_000e18) return 3; // Diamond
        if (amount >= 10_000e18) return 2;  // Gold
        if (amount >= 1_000e18) return 1;   // Silver
        return 0; // Bronze
    }

    // --- Internal ---

    function _pendingReward(address user) internal view returns (uint256) {
        Stake memory s = stakes[user];
        if (s.amount == 0) return 0;
        return (s.amount * accRewardPerShare) / 1e18 - s.rewardDebt;
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}

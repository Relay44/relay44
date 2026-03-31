// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

interface IR44Staking {
    function depositRewards(uint256 amount) external;
}

/// @title RewardDistributor - Routes Clanker LP fee revenue to platform participants
/// @notice Receives R44 from Clanker fee recipients and distributes across:
///         - Staking rewards
///         - Agent performance rewards (claimable by top agents)
///         - Market creator rewards
///         - Protocol treasury
contract RewardDistributor is AccessControl, ReentrancyGuard {
    using SafeERC20 for IERC20;

    bytes32 public constant KEEPER_ROLE = keccak256("KEEPER_ROLE");

    IERC20 public immutable r44Token;
    IR44Staking public stakingPool;
    address public treasury;

    // Reward allocation in BPS (must sum to 10_000)
    uint256 public stakingShareBps;
    uint256 public agentShareBps;
    uint256 public creatorShareBps;
    uint256 public treasuryShareBps;

    // Epoch tracking
    uint256 public currentEpoch;
    uint256 public epochDuration;
    uint256 public lastDistributionAt;

    // Agent rewards: per-epoch claimable amounts set by keeper
    mapping(uint256 => mapping(address => uint256)) public agentRewards; // epoch => agent => amount
    mapping(uint256 => mapping(address => bool)) public agentRewardClaimed;
    mapping(uint256 => uint256) public epochAgentPool; // epoch => total allocated

    // Creator rewards: per-epoch claimable amounts set by keeper
    mapping(uint256 => mapping(address => uint256)) public creatorRewards;
    mapping(uint256 => mapping(address => bool)) public creatorRewardClaimed;
    mapping(uint256 => uint256) public epochCreatorPool;

    error InvalidShares();
    error EpochNotReady();
    error AlreadyClaimed();
    error NothingToClaim();
    error InvalidEpoch();
    error AllocationExceedsPool();

    event EpochDistributed(uint256 indexed epoch, uint256 total, uint256 staking, uint256 agents, uint256 creators, uint256 treasuryAmount);
    event AgentRewardsSet(uint256 indexed epoch, address[] agents, uint256[] amounts);
    event CreatorRewardsSet(uint256 indexed epoch, address[] creators, uint256[] amounts);
    event AgentRewardClaimed(uint256 indexed epoch, address indexed agent, uint256 amount);
    event CreatorRewardClaimed(uint256 indexed epoch, address indexed creator, uint256 amount);
    event SharesUpdated(uint256 staking, uint256 agents, uint256 creators, uint256 treasury);

    constructor(
        address admin,
        address r44TokenAddress,
        address _treasury,
        uint256 _epochDuration
    ) {
        r44Token = IERC20(r44TokenAddress);
        treasury = _treasury;
        epochDuration = _epochDuration;
        lastDistributionAt = block.timestamp;

        // Default shares: matches Clanker fee recipient plan
        stakingShareBps = 2_000;  // 20% to stakers
        agentShareBps = 4_000;    // 40% to agents
        creatorShareBps = 3_000;  // 30% to market creators
        treasuryShareBps = 1_000; // 10% to treasury

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(KEEPER_ROLE, admin);
    }

    function setStakingPool(address _stakingPool) external onlyRole(DEFAULT_ADMIN_ROLE) {
        stakingPool = IR44Staking(_stakingPool);
    }

    function setTreasury(address _treasury) external onlyRole(DEFAULT_ADMIN_ROLE) {
        treasury = _treasury;
    }

    function setShares(uint256 _staking, uint256 _agents, uint256 _creators, uint256 _treasury)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
    {
        if (_staking + _agents + _creators + _treasury != 10_000) revert InvalidShares();
        stakingShareBps = _staking;
        agentShareBps = _agents;
        creatorShareBps = _creators;
        treasuryShareBps = _treasury;
        emit SharesUpdated(_staking, _agents, _creators, _treasury);
    }

    /// @notice Distribute accumulated R44 balance across recipients for current epoch
    function distribute() external onlyRole(KEEPER_ROLE) nonReentrant {
        if (block.timestamp < lastDistributionAt + epochDuration) revert EpochNotReady();

        uint256 balance = r44Token.balanceOf(address(this));
        if (balance == 0) revert NothingToClaim();

        currentEpoch++;
        lastDistributionAt = block.timestamp;

        uint256 stakingAmount = (balance * stakingShareBps) / 10_000;
        uint256 agentAmount = (balance * agentShareBps) / 10_000;
        uint256 creatorAmount = (balance * creatorShareBps) / 10_000;
        uint256 treasuryAmount = balance - stakingAmount - agentAmount - creatorAmount;

        // Send staking rewards
        if (stakingAmount > 0 && address(stakingPool) != address(0)) {
            r44Token.safeIncreaseAllowance(address(stakingPool), stakingAmount);
            stakingPool.depositRewards(stakingAmount);
        }

        // Hold agent + creator amounts for claiming
        epochAgentPool[currentEpoch] = agentAmount;
        epochCreatorPool[currentEpoch] = creatorAmount;

        // Send treasury share
        if (treasuryAmount > 0 && treasury != address(0)) {
            r44Token.safeTransfer(treasury, treasuryAmount);
        }

        emit EpochDistributed(currentEpoch, balance, stakingAmount, agentAmount, creatorAmount, treasuryAmount);
    }

    /// @notice Set agent reward allocations for an epoch. Keeper calls after computing rankings.
    function setAgentRewards(uint256 epoch, address[] calldata agents, uint256[] calldata amounts)
        external
        onlyRole(KEEPER_ROLE)
    {
        if (epoch == 0 || epoch > currentEpoch) revert InvalidEpoch();
        if (agents.length != amounts.length) revert InvalidShares();

        uint256 total;
        for (uint256 i = 0; i < agents.length; i++) {
            agentRewards[epoch][agents[i]] = amounts[i];
            total += amounts[i];
        }
        if (total > epochAgentPool[epoch]) revert AllocationExceedsPool();

        emit AgentRewardsSet(epoch, agents, amounts);
    }

    /// @notice Set creator reward allocations for an epoch
    function setCreatorRewards(uint256 epoch, address[] calldata creators, uint256[] calldata amounts)
        external
        onlyRole(KEEPER_ROLE)
    {
        if (epoch == 0 || epoch > currentEpoch) revert InvalidEpoch();
        if (creators.length != amounts.length) revert InvalidShares();

        uint256 total;
        for (uint256 i = 0; i < creators.length; i++) {
            creatorRewards[epoch][creators[i]] = amounts[i];
            total += amounts[i];
        }
        if (total > epochCreatorPool[epoch]) revert AllocationExceedsPool();

        emit CreatorRewardsSet(epoch, creators, amounts);
    }

    /// @notice Claim agent reward for a specific epoch
    function claimAgentReward(uint256 epoch) external nonReentrant {
        if (agentRewardClaimed[epoch][msg.sender]) revert AlreadyClaimed();
        uint256 amount = agentRewards[epoch][msg.sender];
        if (amount == 0) revert NothingToClaim();

        agentRewardClaimed[epoch][msg.sender] = true;
        r44Token.safeTransfer(msg.sender, amount);

        emit AgentRewardClaimed(epoch, msg.sender, amount);
    }

    /// @notice Claim creator reward for a specific epoch
    function claimCreatorReward(uint256 epoch) external nonReentrant {
        if (creatorRewardClaimed[epoch][msg.sender]) revert AlreadyClaimed();
        uint256 amount = creatorRewards[epoch][msg.sender];
        if (amount == 0) revert NothingToClaim();

        creatorRewardClaimed[epoch][msg.sender] = true;
        r44Token.safeTransfer(msg.sender, amount);

        emit CreatorRewardClaimed(epoch, msg.sender, amount);
    }
}

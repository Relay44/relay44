// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title ParlayEscrow
/// @notice Escrow contract for multi-leg prediction market parlays.
/// Users deposit USDC for a parlay bet. An operator resolves each leg.
/// Payout = stake * product(leg_odds) if all legs win, else 0.
contract ParlayEscrow is AccessControl, ReentrancyGuard {
    using SafeERC20 for IERC20;

    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    IERC20 public immutable collateral;
    uint256 public protocolFeeBps;

    struct Leg {
        uint256 marketId;
        bool outcomeYes;
        uint256 oddsBps;    // e.g. 15000 = 1.5x
        bool resolved;
        bool won;
    }

    struct Parlay {
        address bettor;
        uint256 stake;
        uint256 legCount;
        uint256 resolvedCount;
        bool settled;
        bool allWon;
        uint256 payout;
    }

    uint256 public nextParlayId;
    mapping(uint256 => Parlay) public parlays;
    mapping(uint256 => mapping(uint256 => Leg)) public parlayLegs;

    uint256 public constant MIN_ODDS_BPS = 10000; // 1x minimum
    uint256 public constant MAX_ODDS_BPS = 1000000; // 100x maximum
    uint256 public constant MAX_PAYOUT_MULTIPLIER = 10000; // 10000x cap on total payout/stake

    error InvalidLegCount();
    error InvalidStake();
    error InvalidOdds();
    error AlreadySettled();
    error NotFullyResolved();
    error LegAlreadyResolved();

    event ParlayCreated(uint256 indexed parlayId, address indexed bettor, uint256 stake, uint256 legCount);
    event LegResolved(uint256 indexed parlayId, uint256 legIndex, uint256 marketId, bool won);
    event ParlaySettled(uint256 indexed parlayId, address indexed bettor, uint256 payout, bool won);

    constructor(address _collateral, uint256 _protocolFeeBps) {
        collateral = IERC20(_collateral);
        protocolFeeBps = _protocolFeeBps;
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(OPERATOR_ROLE, msg.sender);
    }

    /// @notice Place a parlay bet with multiple legs.
    /// @param marketIds Array of market IDs for each leg.
    /// @param outcomesYes Array of booleans (true = bet on YES, false = bet on NO).
    /// @param oddsBps Array of odds in basis points (10000 = 1x, 15000 = 1.5x).
    /// @param stake Amount of USDC to wager.
    function createParlay(
        uint256[] calldata marketIds,
        bool[] calldata outcomesYes,
        uint256[] calldata oddsBps,
        uint256 stake
    ) external nonReentrant returns (uint256 parlayId) {
        uint256 legCount = marketIds.length;
        if (legCount < 2 || legCount > 10) revert InvalidLegCount();
        if (outcomesYes.length != legCount || oddsBps.length != legCount) revert InvalidLegCount();
        if (stake == 0) revert InvalidStake();

        collateral.safeTransferFrom(msg.sender, address(this), stake);

        parlayId = nextParlayId++;
        parlays[parlayId] = Parlay({
            bettor: msg.sender,
            stake: stake,
            legCount: legCount,
            resolvedCount: 0,
            settled: false,
            allWon: true,
            payout: 0
        });

        for (uint256 i = 0; i < legCount; i++) {
            if (oddsBps[i] < MIN_ODDS_BPS || oddsBps[i] > MAX_ODDS_BPS) revert InvalidOdds();
            parlayLegs[parlayId][i] = Leg({
                marketId: marketIds[i],
                outcomeYes: outcomesYes[i],
                oddsBps: oddsBps[i],
                resolved: false,
                won: false
            });
        }

        emit ParlayCreated(parlayId, msg.sender, stake, legCount);
    }

    /// @notice Resolve a single leg of a parlay.
    function resolveLeg(
        uint256 parlayId,
        uint256 legIndex,
        bool won
    ) external onlyRole(OPERATOR_ROLE) {
        Parlay storage p = parlays[parlayId];
        if (p.settled) revert AlreadySettled();

        Leg storage leg = parlayLegs[parlayId][legIndex];
        if (leg.resolved) revert LegAlreadyResolved();

        leg.resolved = true;
        leg.won = won;
        p.resolvedCount++;

        if (!won) {
            p.allWon = false;
        }

        emit LegResolved(parlayId, legIndex, leg.marketId, won);
    }

    /// @notice Settle a fully resolved parlay. Pays out if all legs won.
    function settle(uint256 parlayId) external nonReentrant {
        Parlay storage p = parlays[parlayId];
        if (p.settled) revert AlreadySettled();
        if (p.resolvedCount < p.legCount) revert NotFullyResolved();

        p.settled = true;

        if (p.allWon) {
            uint256 payout = p.stake;
            for (uint256 i = 0; i < p.legCount; i++) {
                payout = (payout * parlayLegs[parlayId][i].oddsBps) / 10000;
            }

            // Cap payout at MAX_PAYOUT_MULTIPLIER * stake to bound risk.
            uint256 maxPayout = p.stake * MAX_PAYOUT_MULTIPLIER;
            if (payout > maxPayout) {
                payout = maxPayout;
            }

            uint256 fee = (payout * protocolFeeBps) / 10000;
            uint256 netPayout = payout - fee;
            p.payout = netPayout;

            uint256 balance = collateral.balanceOf(address(this));
            if (netPayout > balance) {
                netPayout = balance;
            }
            collateral.safeTransfer(p.bettor, netPayout);

            emit ParlaySettled(parlayId, p.bettor, netPayout, true);
        } else {
            p.payout = 0;
            emit ParlaySettled(parlayId, p.bettor, 0, false);
        }
    }

    /// @notice Update protocol fee (admin only).
    function setProtocolFeeBps(uint256 _feeBps) external onlyRole(DEFAULT_ADMIN_ROLE) {
        protocolFeeBps = _feeBps;
    }

    /// @notice Withdraw accumulated fees (admin only).
    function withdrawFees(address to, uint256 amount) external onlyRole(DEFAULT_ADMIN_ROLE) {
        collateral.safeTransfer(to, amount);
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";
import { SingularityMarketCore } from "./SingularityMarketCore.sol";

contract SingularityOracleCommittee is RoleAuth {
    error InvalidOutcome();
    error AlreadyVoted();
    error NotCommitteeMember();

    bytes32 public constant COMMITTEE_ADMIN_ROLE = keccak256("COMMITTEE_ADMIN_ROLE");

    struct Tally {
        uint32 yesVotes;
        uint32 noVotes;
        uint32 cancelVotes;
        bool finalized;
    }

    event CommitteeMemberUpdated(address indexed member, bool enabled);
    event VoteCast(uint256 indexed marketId, address indexed voter, uint8 indexed outcomeVote, bytes32 evidenceHash);
    event VoteFinalized(uint256 indexed marketId, uint8 indexed resolvedOutcome, string oracleSource);

    SingularityMarketCore public immutable marketCore;
    uint32 public quorum;

    mapping(address => bool) public committeeMembers;
    mapping(uint256 => Tally) public tallies;
    mapping(uint256 => mapping(address => bool)) public hasVoted;

    constructor(address admin, address marketCoreAddress, uint32 quorumThreshold) RoleAuth(admin) {
        if (marketCoreAddress == address(0)) revert InvalidAddress();
        if (quorumThreshold == 0) revert InvalidOutcome();
        marketCore = SingularityMarketCore(marketCoreAddress);
        quorum = quorumThreshold;
        _grantRole(COMMITTEE_ADMIN_ROLE, admin);
    }

    function setCommitteeMember(address member, bool enabled) external onlyRole(COMMITTEE_ADMIN_ROLE) {
        if (member == address(0)) revert InvalidAddress();
        committeeMembers[member] = enabled;
        emit CommitteeMemberUpdated(member, enabled);
    }

    function setQuorum(uint32 newQuorum) external onlyRole(COMMITTEE_ADMIN_ROLE) {
        if (newQuorum == 0) revert InvalidOutcome();
        quorum = newQuorum;
    }

    function castVote(uint256 marketId, uint8 outcomeVote, bytes32 evidenceHash, string calldata oracleSource) external {
        if (!committeeMembers[msg.sender]) revert NotCommitteeMember();
        if (outcomeVote > 2) revert InvalidOutcome();
        if (hasVoted[marketId][msg.sender]) revert AlreadyVoted();

        Tally storage tally = tallies[marketId];
        if (tally.finalized) revert InvalidOutcome();

        hasVoted[marketId][msg.sender] = true;
        if (outcomeVote == 0) {
            tally.yesVotes += 1;
        } else if (outcomeVote == 1) {
            tally.noVotes += 1;
        } else {
            tally.cancelVotes += 1;
        }

        emit VoteCast(marketId, msg.sender, outcomeVote, evidenceHash);

        uint32 maxVotes = _max(tally.yesVotes, tally.noVotes, tally.cancelVotes);
        if (maxVotes < quorum) return;

        tally.finalized = true;

        if (tally.cancelVotes >= tally.yesVotes && tally.cancelVotes >= tally.noVotes) {
            marketCore.cancelMarket(marketId);
            emit VoteFinalized(marketId, 2, oracleSource);
            return;
        }

        uint8 outcome = tally.yesVotes >= tally.noVotes ? 0 : 1;
        marketCore.resolveMarket(marketId, outcome, msg.sender, evidenceHash, oracleSource);
        emit VoteFinalized(marketId, outcome, oracleSource);
    }

    function _max(uint32 a, uint32 b, uint32 c) internal pure returns (uint32) {
        uint32 m = a;
        if (b > m) m = b;
        if (c > m) m = c;
        return m;
    }
}

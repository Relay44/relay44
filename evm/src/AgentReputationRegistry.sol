// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";

interface IAgentIdentityRead {
    function ownerOf(uint256 tokenId) external view returns (address);
    function getApproved(uint256 tokenId) external view returns (address);
    function isApprovedForAll(address owner, address operator) external view returns (bool);
}

contract AgentReputationRegistry is AccessControl, Pausable {
    bytes32 public constant ORACLE_ROLE = keccak256("ORACLE_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    struct AgentMetrics {
        int128 roiBps;
        uint128 totalVolume;
        uint64 tradeCount;
        uint64 winCount;
        uint64 lossCount;
        uint16 maxDrawdownBps;
        uint64 updatedAt;
    }

    struct Feedback {
        int128 value;
        uint8 valueDecimals;
        uint64 createdAt;
        bool revoked;
        string category;
        string comment;
        string endpoint;
        string feedbackURI;
        bytes32 feedbackHash;
    }

    struct FeedbackView {
        uint256 agentId;
        address client;
        uint64 index;
        int128 value;
        uint8 valueDecimals;
        uint64 createdAt;
        bool revoked;
        string category;
        string comment;
        string endpoint;
        string feedbackURI;
        bytes32 feedbackHash;
    }

    struct FeedbackInput {
        int128 value;
        uint8 valueDecimals;
        string category;
        string comment;
        string endpoint;
        string feedbackURI;
        bytes32 feedbackHash;
    }

    IAgentIdentityRead public immutable identityRegistry;

    mapping(uint256 => AgentMetrics) public metrics;
    mapping(uint256 => address[]) private feedbackClients;
    mapping(uint256 => mapping(address => bool)) private hasClientFeedback;
    mapping(uint256 => mapping(address => uint64)) public feedbackCount;
    mapping(uint256 => mapping(address => mapping(uint64 => Feedback))) private feedbackByClient;

    error ZeroAddress();
    error AgentNotFound();
    error InvalidValueDecimals();
    error FeedbackNotFound();
    error FeedbackAlreadyRevoked();
    error SelfOrOperatorFeedbackForbidden();

    event FeedbackGiven(uint256 indexed agentId, address indexed client, uint64 indexed index, int128 value);
    event FeedbackRevoked(uint256 indexed agentId, address indexed client, uint64 indexed index);
    event MetricsUpdated(
        uint256 indexed agentId,
        int128 roiBps,
        uint128 totalVolume,
        uint64 tradeCount,
        uint64 winCount,
        uint64 lossCount,
        uint16 maxDrawdownBps
    );

    constructor(address admin, address identityRegistryAddress) {
        if (admin == address(0) || identityRegistryAddress == address(0)) revert ZeroAddress();

        identityRegistry = IAgentIdentityRead(identityRegistryAddress);
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ORACLE_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    function giveFeedback(uint256 agentId, FeedbackInput calldata input) external whenNotPaused {
        if (input.valueDecimals > 18) revert InvalidValueDecimals();
        _assertFeedbackAuthorAllowed(agentId, msg.sender);

        if (!hasClientFeedback[agentId][msg.sender]) {
            hasClientFeedback[agentId][msg.sender] = true;
            feedbackClients[agentId].push(msg.sender);
        }

        uint64 index = ++feedbackCount[agentId][msg.sender];
        _storeFeedback(agentId, msg.sender, index, input);

        emit FeedbackGiven(agentId, msg.sender, index, input.value);
    }

    function revokeFeedback(uint256 agentId, uint64 index) external whenNotPaused {
        _requireAgentOwner(agentId);
        if (index == 0 || index > feedbackCount[agentId][msg.sender]) revert FeedbackNotFound();

        Feedback storage feedback = feedbackByClient[agentId][msg.sender][index];
        if (feedback.revoked) revert FeedbackAlreadyRevoked();
        feedback.revoked = true;

        emit FeedbackRevoked(agentId, msg.sender, index);
    }

    function getFeedback(uint256 agentId, address client, uint64 index) external view returns (Feedback memory) {
        _requireAgentOwner(agentId);
        if (index == 0 || index > feedbackCount[agentId][client]) revert FeedbackNotFound();
        return feedbackByClient[agentId][client][index];
    }

    function getFeedbackClients(uint256 agentId) external view returns (address[] memory) {
        _requireAgentOwner(agentId);
        return feedbackClients[agentId];
    }

    function listFeedback(uint256 agentId, bool includeRevoked, uint64 maxItems)
        external
        view
        returns (FeedbackView[] memory result)
    {
        _requireAgentOwner(agentId);
        uint64 cap = maxItems == 0 ? type(uint64).max : maxItems;

        uint256 total;
        address[] storage clients = feedbackClients[agentId];
        for (uint256 i = 0; i < clients.length; i++) {
            address client = clients[i];
            uint64 count = feedbackCount[agentId][client];
            for (uint64 j = 1; j <= count; j++) {
                Feedback storage feedback = feedbackByClient[agentId][client][j];
                if (!includeRevoked && feedback.revoked) {
                    continue;
                }
                total++;
                if (total >= cap) break;
            }
            if (total >= cap) break;
        }

        result = new FeedbackView[](total);
        uint256 outIdx;
        for (uint256 i = 0; i < clients.length && outIdx < total; i++) {
            address client = clients[i];
            uint64 count = feedbackCount[agentId][client];
            for (uint64 j = 1; j <= count && outIdx < total; j++) {
                Feedback storage feedback = feedbackByClient[agentId][client][j];
                if (!includeRevoked && feedback.revoked) continue;

                result[outIdx] = FeedbackView({
                    agentId: agentId,
                    client: client,
                    index: j,
                    value: feedback.value,
                    valueDecimals: feedback.valueDecimals,
                    createdAt: feedback.createdAt,
                    revoked: feedback.revoked,
                    category: feedback.category,
                    comment: feedback.comment,
                    endpoint: feedback.endpoint,
                    feedbackURI: feedback.feedbackURI,
                    feedbackHash: feedback.feedbackHash
                });
                outIdx++;
            }
        }
    }

    function updateMetrics(
        uint256 agentId,
        int128 roiBps,
        uint128 totalVolume,
        uint64 tradeCount,
        uint64 winCount,
        uint64 lossCount,
        uint16 maxDrawdownBps
    ) external onlyRole(ORACLE_ROLE) whenNotPaused {
        _requireAgentOwner(agentId);

        metrics[agentId] = AgentMetrics({
            roiBps: roiBps,
            totalVolume: totalVolume,
            tradeCount: tradeCount,
            winCount: winCount,
            lossCount: lossCount,
            maxDrawdownBps: maxDrawdownBps,
            updatedAt: uint64(block.timestamp)
        });

        emit MetricsUpdated(agentId, roiBps, totalVolume, tradeCount, winCount, lossCount, maxDrawdownBps);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function _requireAgentOwner(uint256 agentId) internal view returns (address owner) {
        try identityRegistry.ownerOf(agentId) returns (address foundOwner) {
            return foundOwner;
        } catch {
            revert AgentNotFound();
        }
    }

    function _assertFeedbackAuthorAllowed(uint256 agentId, address author) internal view {
        address owner = _requireAgentOwner(agentId);
        address approved = identityRegistry.getApproved(agentId);
        if (author == owner || author == approved || identityRegistry.isApprovedForAll(owner, author)) {
            revert SelfOrOperatorFeedbackForbidden();
        }
    }

    function _storeFeedback(uint256 agentId, address client, uint64 index, FeedbackInput calldata input) internal {
        Feedback storage feedback = feedbackByClient[agentId][client][index];
        feedback.value = input.value;
        feedback.valueDecimals = input.valueDecimals;
        feedback.createdAt = uint64(block.timestamp);
        feedback.revoked = false;
        feedback.category = input.category;
        feedback.comment = input.comment;
        feedback.endpoint = input.endpoint;
        feedback.feedbackURI = input.feedbackURI;
        feedback.feedbackHash = input.feedbackHash;
    }
}

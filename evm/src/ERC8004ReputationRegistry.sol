// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";

interface IERC8004IdentityRead {
    function identityOf(address wallet) external view returns (uint256);
    function ownerOfIdentity(uint256 identityId) external view returns (address);
}

contract ERC8004ReputationRegistry is AccessControl, Pausable {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant ATTESTER_ROLE = keccak256("ATTESTER_ROLE");

    struct Reputation {
        uint64 eventCount;
        uint64 successCount;
        uint128 notionalMicrousdc;
        uint32 scoreBps;
        uint32 confidenceBps;
        uint64 updatedAt;
    }

    struct LegacyFeedbackEntry {
        uint64 id;
        address reviewer;
        int32 ratingBps;
        string review;
        uint64 createdAt;
    }

    struct Feedback {
        int128 value;
        uint8 valueDecimals;
        bytes32 tag1;
        bytes32 tag2;
        bytes32 endpoint;
        string feedbackURI;
        bytes32 feedbackHash;
        uint64 timestamp;
        bool isRevoked;
    }

    struct Response {
        address responder;
        string responseURI;
        bytes32 responseHash;
        uint64 timestamp;
    }

    struct FeedbackBatch {
        address[] clients;
        uint64[] indices;
        int128[] values;
        uint8[] valueDecimals;
        bytes32[] tag1s;
        bytes32[] tag2s;
        bool[] revoked;
    }

    IERC8004IdentityRead public immutable identityRegistry;

    mapping(address => Reputation) private _reputation;
    mapping(address => LegacyFeedbackEntry[]) private _legacyFeedback;

    mapping(uint256 => mapping(address => mapping(uint64 => Feedback))) private _feedback;
    mapping(uint256 => mapping(address => uint64)) private _feedbackCount;
    mapping(uint256 => address[]) private _agentClients;
    mapping(uint256 => mapping(address => bool)) private _hasSubmittedFeedback;
    mapping(uint256 => mapping(address => mapping(uint64 => Response[]))) private _responses;

    error ZeroAddress();
    error IdentityMissing();
    error InvalidConfidenceWeight();
    error InvalidRating();
    error SelfFeedbackForbidden();
    error FeedbackNotFound();
    error AgentNotFound();
    error FeedbackAlreadyRevoked();
    error EmptyClientList();

    event OutcomeSubmitted(address indexed wallet, bool success, uint128 notionalMicrousdc, uint16 confidenceWeightBps);
    event ReputationUpdated(
        address indexed wallet,
        uint32 scoreBps,
        uint32 confidenceBps,
        uint64 eventCount,
        uint128 notionalMicrousdc
    );
    event FeedbackSubmitted(address indexed wallet, uint64 indexed feedbackId, address reviewer, int32 ratingBps);
    event FeedbackRevoked(address indexed wallet, uint64 indexed feedbackId);
    event FeedbackRevoked(uint256 indexed agentId, address indexed clientAddress, uint64 indexed feedbackIndex);

    event NewFeedback(
        uint256 indexed agentId,
        address indexed clientAddress,
        uint64 feedbackIndex,
        int128 value,
        uint8 valueDecimals,
        bytes32 indexed indexedTag1,
        bytes32 tag1,
        bytes32 tag2,
        bytes32 endpoint,
        string feedbackURI,
        bytes32 feedbackHash
    );
    event ResponseAppended(
        uint256 indexed agentId,
        address indexed clientAddress,
        uint64 feedbackIndex,
        address indexed responder,
        string responseURI,
        bytes32 responseHash
    );

    constructor(address admin, address identityRegistryAddress) {
        if (admin == address(0) || identityRegistryAddress == address(0)) revert ZeroAddress();
        identityRegistry = IERC8004IdentityRead(identityRegistryAddress);

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(ATTESTER_ROLE, admin);
    }

    function submitOutcome(address wallet, bool success, uint128 notionalMicrousdc, uint16 confidenceWeightBps)
        external
        onlyRole(ATTESTER_ROLE)
        whenNotPaused
    {
        if (wallet == address(0)) revert ZeroAddress();
        if (identityRegistry.identityOf(wallet) == 0) revert IdentityMissing();
        if (confidenceWeightBps > 10_000) revert InvalidConfidenceWeight();

        Reputation storage row = _reputation[wallet];
        row.eventCount += 1;
        if (success) {
            row.successCount += 1;
        }
        row.notionalMicrousdc += notionalMicrousdc;
        row.scoreBps = uint32((uint256(row.successCount) * 10_000) / uint256(row.eventCount));

        uint256 confidence = uint256(row.eventCount) * 250 + uint256(confidenceWeightBps);
        if (confidence > 10_000) {
            confidence = 10_000;
        }
        row.confidenceBps = uint32(confidence);
        row.updatedAt = uint64(block.timestamp);

        emit OutcomeSubmitted(wallet, success, notionalMicrousdc, confidenceWeightBps);
        emit ReputationUpdated(wallet, row.scoreBps, row.confidenceBps, row.eventCount, row.notionalMicrousdc);
    }

    function submitReputation(address wallet, uint32 scoreBps, uint32 confidenceBps, uint128 notionalMicrousdc)
        external
        onlyRole(ATTESTER_ROLE)
        whenNotPaused
    {
        if (wallet == address(0)) revert ZeroAddress();
        if (identityRegistry.identityOf(wallet) == 0) revert IdentityMissing();
        if (scoreBps > 10_000 || confidenceBps > 10_000) revert InvalidConfidenceWeight();

        Reputation storage row = _reputation[wallet];
        row.eventCount += 1;
        row.notionalMicrousdc += notionalMicrousdc;
        row.scoreBps = scoreBps;
        row.confidenceBps = confidenceBps;
        row.updatedAt = uint64(block.timestamp);

        emit ReputationUpdated(wallet, row.scoreBps, row.confidenceBps, row.eventCount, row.notionalMicrousdc);
    }

    function submitFeedback(address wallet, int32 ratingBps, string calldata review)
        external
        whenNotPaused
        returns (uint64)
    {
        if (wallet == address(0)) revert ZeroAddress();
        if (identityRegistry.identityOf(wallet) == 0) revert IdentityMissing();
        if (msg.sender == wallet) revert SelfFeedbackForbidden();
        if (ratingBps < -10_000 || ratingBps > 10_000) revert InvalidRating();

        LegacyFeedbackEntry[] storage rows = _legacyFeedback[wallet];
        uint64 feedbackId = uint64(rows.length + 1);
        rows.push(
            LegacyFeedbackEntry({
                id: feedbackId,
                reviewer: msg.sender,
                ratingBps: ratingBps,
                review: review,
                createdAt: uint64(block.timestamp)
            })
        );

        emit FeedbackSubmitted(wallet, feedbackId, msg.sender, ratingBps);
        return feedbackId;
    }

    function revokeFeedback(address wallet, uint64 feedbackId) external onlyRole(ATTESTER_ROLE) whenNotPaused {
        LegacyFeedbackEntry[] storage rows = _legacyFeedback[wallet];
        if (feedbackId == 0 || feedbackId > rows.length) revert FeedbackNotFound();

        uint64 index = feedbackId - 1;
        if (rows.length == 1) {
            rows.pop();
        } else {
            rows[index] = rows[rows.length - 1];
            rows[index].id = feedbackId;
            rows.pop();
        }

        emit FeedbackRevoked(wallet, feedbackId);
    }

    function reputationOf(address wallet)
        external
        view
        returns (uint32 scoreBps, uint32 confidenceBps, uint64 eventCount, uint128 notionalMicrousdc)
    {
        Reputation storage row = _reputation[wallet];
        return (row.scoreBps, row.confidenceBps, row.eventCount, row.notionalMicrousdc);
    }

    function getReputation(address wallet)
        external
        view
        returns (uint32 scoreBps, uint32 confidenceBps, uint64 eventCount, uint128 notionalMicrousdc, uint64 updatedAt)
    {
        Reputation storage row = _reputation[wallet];
        return (row.scoreBps, row.confidenceBps, row.eventCount, row.notionalMicrousdc, row.updatedAt);
    }

    function feedbackCount(address wallet) external view returns (uint64) {
        return uint64(_legacyFeedback[wallet].length);
    }

    function feedbackAt(address wallet, uint64 index)
        external
        view
        returns (uint64 id, address reviewer, int32 ratingBps, string memory review, uint64 createdAt)
    {
        LegacyFeedbackEntry storage row = _legacyFeedback[wallet][index];
        return (row.id, row.reviewer, row.ratingBps, row.review, row.createdAt);
    }

    function giveFeedback(
        uint256 agentId,
        int128 value,
        uint8 valueDecimals,
        bytes32 tag1,
        bytes32 tag2,
        bytes32 endpoint,
        string calldata feedbackURI,
        bytes32 feedbackHash
    ) external whenNotPaused {
        if (!_agentExists(agentId)) revert AgentNotFound();

        address owner = identityRegistry.ownerOfIdentity(agentId);
        if (msg.sender == owner) revert SelfFeedbackForbidden();

        uint64 feedbackIndex = _feedbackCount[agentId][msg.sender];
        _feedback[agentId][msg.sender][feedbackIndex] = Feedback({
            value: value,
            valueDecimals: valueDecimals,
            tag1: tag1,
            tag2: tag2,
            endpoint: endpoint,
            feedbackURI: feedbackURI,
            feedbackHash: feedbackHash,
            timestamp: uint64(block.timestamp),
            isRevoked: false
        });
        _feedbackCount[agentId][msg.sender] = feedbackIndex + 1;

        if (!_hasSubmittedFeedback[agentId][msg.sender]) {
            _agentClients[agentId].push(msg.sender);
            _hasSubmittedFeedback[agentId][msg.sender] = true;
        }

        emit NewFeedback(
            agentId,
            msg.sender,
            feedbackIndex,
            value,
            valueDecimals,
            tag1,
            tag1,
            tag2,
            endpoint,
            feedbackURI,
            feedbackHash
        );
    }

    function revokeFeedback(uint256 agentId, uint64 feedbackIndex) external whenNotPaused {
        Feedback storage row = _feedback[agentId][msg.sender][feedbackIndex];
        if (row.timestamp == 0) revert FeedbackNotFound();
        if (row.isRevoked) revert FeedbackAlreadyRevoked();

        row.isRevoked = true;

        emit FeedbackRevoked(agentId, msg.sender, feedbackIndex);
    }

    function appendResponse(
        uint256 agentId,
        address clientAddress,
        uint64 feedbackIndex,
        string calldata responseURI,
        bytes32 responseHash
    ) external whenNotPaused {
        if (!_agentExists(agentId)) revert AgentNotFound();

        Feedback storage row = _feedback[agentId][clientAddress][feedbackIndex];
        if (row.timestamp == 0) revert FeedbackNotFound();

        _responses[agentId][clientAddress][feedbackIndex].push(
            Response({
                responder: msg.sender,
                responseURI: responseURI,
                responseHash: responseHash,
                timestamp: uint64(block.timestamp)
            })
        );

        emit ResponseAppended(agentId, clientAddress, feedbackIndex, msg.sender, responseURI, responseHash);
    }

    function getSummary(
        uint256 agentId,
        address[] calldata clientAddresses,
        bytes32 tag1,
        bytes32 tag2
    ) external view returns (uint64 count, int128 summaryValue, uint8 decimals) {
        if (clientAddresses.length == 0) revert EmptyClientList();

        int256 sum = 0;
        uint8 maxDecimals = 0;

        for (uint256 i = 0; i < clientAddresses.length; i++) {
            uint64 rowCount = _feedbackCount[agentId][clientAddresses[i]];
            for (uint64 j = 0; j < rowCount; j++) {
                Feedback storage row = _feedback[agentId][clientAddresses[i]][j];
                if (!_matchesFeedbackFilter(row, tag1, tag2, false)) {
                    continue;
                }
                sum += int256(row.value);
                count++;
                if (row.valueDecimals > maxDecimals) {
                    maxDecimals = row.valueDecimals;
                }
            }
        }

        summaryValue = int128(sum);
        decimals = maxDecimals;
    }

    function readFeedback(uint256 agentId, address clientAddress, uint64 feedbackIndex)
        external
        view
        returns (int128 value, uint8 valueDecimals, bytes32 tag1, bytes32 tag2, bool isRevoked)
    {
        Feedback storage row = _feedback[agentId][clientAddress][feedbackIndex];
        if (row.timestamp == 0) revert FeedbackNotFound();

        return (row.value, row.valueDecimals, row.tag1, row.tag2, row.isRevoked);
    }

    function readAllFeedback(
        uint256 agentId,
        address[] calldata clientAddresses,
        bytes32 tag1,
        bytes32 tag2,
        bool includeRevoked
    )
        external
        view
        returns (
            address[] memory,
            uint64[] memory,
            int128[] memory,
            uint8[] memory,
            bytes32[] memory,
            bytes32[] memory,
            bool[] memory
        )
    {
        if (clientAddresses.length == 0) revert EmptyClientList();
        FeedbackBatch memory batch = _buildFeedbackBatch(agentId, clientAddresses, tag1, tag2, includeRevoked);

        return (
            batch.clients, batch.indices, batch.values, batch.valueDecimals, batch.tag1s, batch.tag2s, batch.revoked
        );
    }

    function getLastIndex(uint256 agentId, address clientAddress) external view returns (uint64) {
        uint64 count = _feedbackCount[agentId][clientAddress];
        if (count == 0) {
            return 0;
        }
        return count - 1;
    }

    function getClients(uint256 agentId) external view returns (address[] memory) {
        return _agentClients[agentId];
    }

    function getFeedbackFull(uint256 agentId, address clientAddress, uint64 feedbackIndex)
        external
        view
        returns (Feedback memory)
    {
        Feedback storage row = _feedback[agentId][clientAddress][feedbackIndex];
        if (row.timestamp == 0) revert FeedbackNotFound();
        return row;
    }

    function getResponseCount(uint256 agentId, address clientAddress, uint64 feedbackIndex)
        external
        view
        returns (uint256)
    {
        return _responses[agentId][clientAddress][feedbackIndex].length;
    }

    function getResponse(uint256 agentId, address clientAddress, uint64 feedbackIndex, uint256 responseIndex)
        external
        view
        returns (address responder, string memory responseURI, bytes32 responseHash, uint64 timestamp)
    {
        Response storage row = _responses[agentId][clientAddress][feedbackIndex][responseIndex];
        return (row.responder, row.responseURI, row.responseHash, row.timestamp);
    }

    function _countMatchingFeedback(
        uint256 agentId,
        address[] calldata clientAddresses,
        bytes32 tag1,
        bytes32 tag2,
        bool includeRevoked
    ) internal view returns (uint256 totalCount) {
        for (uint256 i = 0; i < clientAddresses.length; i++) {
            uint64 rowCount = _feedbackCount[agentId][clientAddresses[i]];
            for (uint64 j = 0; j < rowCount; j++) {
                Feedback storage row = _feedback[agentId][clientAddresses[i]][j];
                if (_matchesFeedbackFilter(row, tag1, tag2, includeRevoked)) {
                    totalCount++;
                }
            }
        }
    }

    function _matchesFeedbackFilter(
        Feedback storage row,
        bytes32 tag1,
        bytes32 tag2,
        bool includeRevoked
    ) internal view returns (bool) {
        if (!includeRevoked && row.isRevoked) return false;
        if (tag1 != bytes32(0) && row.tag1 != tag1) return false;
        if (tag2 != bytes32(0) && row.tag2 != tag2) return false;
        return true;
    }

    function _buildFeedbackBatch(
        uint256 agentId,
        address[] calldata clientAddresses,
        bytes32 tag1,
        bytes32 tag2,
        bool includeRevoked
    ) internal view returns (FeedbackBatch memory batch) {
        uint256 totalCount = _countMatchingFeedback(agentId, clientAddresses, tag1, tag2, includeRevoked);
        batch.clients = new address[](totalCount);
        batch.indices = new uint64[](totalCount);
        batch.values = new int128[](totalCount);
        batch.valueDecimals = new uint8[](totalCount);
        batch.tag1s = new bytes32[](totalCount);
        batch.tag2s = new bytes32[](totalCount);
        batch.revoked = new bool[](totalCount);

        uint256 idx = 0;
        for (uint256 i = 0; i < clientAddresses.length; i++) {
            for (uint64 j = 0; j < _feedbackCount[agentId][clientAddresses[i]]; j++) {
                Feedback storage row = _feedback[agentId][clientAddresses[i]][j];
                if (!_matchesFeedbackFilter(row, tag1, tag2, includeRevoked)) {
                    continue;
                }
                batch.clients[idx] = clientAddresses[i];
                batch.indices[idx] = j;
                batch.values[idx] = row.value;
                batch.valueDecimals[idx] = row.valueDecimals;
                batch.tag1s[idx] = row.tag1;
                batch.tag2s[idx] = row.tag2;
                batch.revoked[idx] = row.isRevoked;
                idx++;
            }
        }
    }

    function _agentExists(uint256 agentId) internal view returns (bool) {
        try identityRegistry.ownerOfIdentity(agentId) returns (address owner) {
            return owner != address(0);
        } catch {
            return false;
        }
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}

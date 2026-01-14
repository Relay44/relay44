// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";

interface IERC8004IdentityValidationRead {
    function ownerOfIdentity(uint256 identityId) external view returns (address);
}

contract ERC8004ValidationRegistry is AccessControl, Pausable {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant VALIDATOR_MANAGER_ROLE = keccak256("VALIDATOR_MANAGER_ROLE");

    struct ValidationRecord {
        address validatorAddress;
        uint256 agentId;
        uint8 response;
        bytes32 responseHash;
        bytes32 tag;
        uint64 timestamp;
        bool responded;
    }

    IERC8004IdentityValidationRead public identityRegistry;

    mapping(bytes32 => ValidationRecord) private _validations;
    mapping(uint256 => bytes32[]) private _agentValidations;
    mapping(address => bytes32[]) private _validatorRequests;
    mapping(address => bool) public isValidator;
    address[] private _validators;

    error ZeroAddress();
    error AgentNotFound();
    error ValidationNotFound();
    error DuplicateValidationRequest();
    error NotValidator();
    error AlreadyResponded();
    error InvalidResponse();

    event ValidationRequest(
        address indexed validatorAddress,
        uint256 indexed agentId,
        string requestURI,
        bytes32 indexed requestHash
    );
    event ValidationResponse(
        address indexed validatorAddress,
        uint256 indexed agentId,
        bytes32 indexed requestHash,
        uint8 response,
        string responseURI,
        bytes32 responseHash,
        bytes32 tag
    );
    event ValidatorAdded(address indexed validator);
    event ValidatorRemoved(address indexed validator);

    constructor(address admin, address identityRegistryAddress) {
        if (admin == address(0) || identityRegistryAddress == address(0)) revert ZeroAddress();
        identityRegistry = IERC8004IdentityValidationRead(identityRegistryAddress);

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(VALIDATOR_MANAGER_ROLE, admin);

        isValidator[admin] = true;
        _validators.push(admin);
        emit ValidatorAdded(admin);
    }

    function validationRequest(address validatorAddress, uint256 agentId, string calldata requestURI, bytes32 requestHash)
        external
        whenNotPaused
    {
        if (validatorAddress == address(0)) revert ZeroAddress();
        if (!isValidator[validatorAddress]) revert NotValidator();
        if (!_agentExists(agentId)) revert AgentNotFound();

        bytes32 effectiveHash = requestHash == bytes32(0) ? keccak256(abi.encodePacked(requestURI)) : requestHash;
        if (_validations[effectiveHash].timestamp != 0) revert DuplicateValidationRequest();

        _validations[effectiveHash] = ValidationRecord({
            validatorAddress: validatorAddress,
            agentId: agentId,
            response: 0,
            responseHash: bytes32(0),
            tag: bytes32(0),
            timestamp: uint64(block.timestamp),
            responded: false
        });

        _agentValidations[agentId].push(effectiveHash);
        _validatorRequests[validatorAddress].push(effectiveHash);

        emit ValidationRequest(validatorAddress, agentId, requestURI, effectiveHash);
    }

    function validationResponse(
        bytes32 requestHash,
        uint8 response,
        string calldata responseURI,
        bytes32 responseHash,
        bytes32 tag
    ) public whenNotPaused {
        ValidationRecord storage record = _validations[requestHash];
        if (record.timestamp == 0) revert ValidationNotFound();
        if (record.validatorAddress != msg.sender || !isValidator[msg.sender]) revert NotValidator();
        if (record.responded) revert AlreadyResponded();
        if (response > 100) revert InvalidResponse();

        record.response = response;
        record.responseHash = responseHash;
        record.tag = tag;
        record.timestamp = uint64(block.timestamp);
        record.responded = true;

        emit ValidationResponse(msg.sender, record.agentId, requestHash, response, responseURI, responseHash, tag);
    }

    function validationResponseFromTier(bytes32 requestHash, uint8 tier, string calldata responseURI, bytes32 responseHash)
        external
        whenNotPaused
    {
        validationResponse(
            requestHash, tierToResponse(tier), responseURI, responseHash, keccak256("validation_tier")
        );
    }

    function getValidationStatus(bytes32 requestHash)
        external
        view
        returns (
            address validatorAddress,
            uint256 agentId,
            uint8 response,
            bytes32 responseHash,
            bytes32 tag,
            uint64 lastUpdate
        )
    {
        ValidationRecord storage record = _validations[requestHash];
        if (record.timestamp == 0) revert ValidationNotFound();

        return (
            record.validatorAddress, record.agentId, record.response, record.responseHash, record.tag, record.timestamp
        );
    }

    function getSummary(uint256 agentId, address[] calldata validatorAddresses, bytes32 tag)
        external
        view
        returns (uint64 count, uint8 averageResponse)
    {
        bytes32[] storage requests = _agentValidations[agentId];
        uint256 totalCount = 0;
        uint256 totalResponse = 0;

        for (uint256 i = 0; i < requests.length; i++) {
            ValidationRecord storage record = _validations[requests[i]];
            if (!record.responded) continue;

            if (tag != bytes32(0) && record.tag != tag) continue;
            if (validatorAddresses.length > 0 && !_isListedValidator(record.validatorAddress, validatorAddresses)) {
                continue;
            }

            totalCount++;
            totalResponse += record.response;
        }

        count = uint64(totalCount);
        if (totalCount > 0) {
            averageResponse = uint8(totalResponse / totalCount);
        }
    }

    function getAgentValidations(uint256 agentId) external view returns (bytes32[] memory requestHashes) {
        return _agentValidations[agentId];
    }

    function getValidatorRequests(address validatorAddress) external view returns (bytes32[] memory requestHashes) {
        return _validatorRequests[validatorAddress];
    }

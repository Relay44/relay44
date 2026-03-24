// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {ERC721} from "openzeppelin-contracts/contracts/token/ERC721/ERC721.sol";
import {ERC721URIStorage} from "openzeppelin-contracts/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";

contract AgentIdentityRegistry is ERC721URIStorage, AccessControl, Pausable {
    bytes32 public constant REGISTRAR_ROLE = keccak256("REGISTRAR_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    uint256 public agentIdCounter;

    mapping(uint256 => mapping(bytes32 => bytes)) private metadataStore;

    error ZeroAddress();
    error AgentNotFound();
    error NotAuthorized();

    event AgentRegistered(uint256 indexed agentId, address indexed owner, address indexed registrar, string agentURI);
    event AgentMetadataSet(uint256 indexed agentId, address indexed writer, string metadataKey, bytes metadataValue);

    constructor(address admin) ERC721("Relay44 Agent Identity", "NMAI") {
        if (admin == address(0)) revert ZeroAddress();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(REGISTRAR_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    function register(string calldata agentURI) external whenNotPaused returns (uint256 agentId) {
        return _register(msg.sender, agentURI, msg.sender);
    }

    function registerFor(address owner, string calldata agentURI)
        external
        onlyRole(REGISTRAR_ROLE)
        whenNotPaused
        returns (uint256 agentId)
    {
        if (owner == address(0)) revert ZeroAddress();
        return _register(owner, agentURI, msg.sender);
    }

    function getMetadata(uint256 agentId, string calldata metadataKey) external view returns (bytes memory) {
        if (_ownerOf(agentId) == address(0)) revert AgentNotFound();
        return metadataStore[agentId][keccak256(bytes(metadataKey))];
    }

    function setMetadata(uint256 agentId, string calldata metadataKey, bytes calldata metadataValue)
        external
        whenNotPaused
    {
        if (_ownerOf(agentId) == address(0)) revert AgentNotFound();
        if (!_canWrite(agentId, msg.sender)) revert NotAuthorized();

        metadataStore[agentId][keccak256(bytes(metadataKey))] = metadataValue;
        emit AgentMetadataSet(agentId, msg.sender, metadataKey, metadataValue);
    }

    function setAgentURI(uint256 agentId, string calldata agentURI) external whenNotPaused {
        if (_ownerOf(agentId) == address(0)) revert AgentNotFound();
        if (!_canWrite(agentId, msg.sender)) revert NotAuthorized();

        _setTokenURI(agentId, agentURI);
    }

    function exists(uint256 agentId) external view returns (bool) {
        return _ownerOf(agentId) != address(0);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function _register(address owner, string calldata agentURI, address registrar) internal returns (uint256 agentId) {
        if (owner == address(0)) revert ZeroAddress();

        agentId = ++agentIdCounter;
        _mint(owner, agentId);
        _setTokenURI(agentId, agentURI);

        emit AgentRegistered(agentId, owner, registrar, agentURI);
    }

    function _canWrite(uint256 agentId, address actor) internal view returns (bool) {
        address owner = ownerOf(agentId);
        return actor == owner || actor == getApproved(agentId) || isApprovedForAll(owner, actor)
            || hasRole(REGISTRAR_ROLE, actor) || hasRole(DEFAULT_ADMIN_ROLE, actor);
    }

    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(ERC721URIStorage, AccessControl)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}

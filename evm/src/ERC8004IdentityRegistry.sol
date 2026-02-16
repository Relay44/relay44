// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {ERC721} from "openzeppelin-contracts/contracts/token/ERC721/ERC721.sol";
import {ERC721URIStorage} from "openzeppelin-contracts/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {Strings} from "openzeppelin-contracts/contracts/utils/Strings.sol";
import {ECDSA} from "openzeppelin-contracts/contracts/utils/cryptography/ECDSA.sol";
import {EIP712} from "openzeppelin-contracts/contracts/utils/cryptography/EIP712.sol";

contract ERC8004IdentityRegistry is ERC721URIStorage, AccessControl, Pausable, EIP712 {
    using Strings for uint256;

    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant ISSUER_ROLE = keccak256("ISSUER_ROLE");

    bytes32 public constant AGENT_WALLET_KEY = keccak256("agentWallet");
    bytes32 public constant SET_WALLET_TYPEHASH =
        keccak256("SetAgentWallet(uint256 agentId,address newWallet,uint256 deadline)");

    uint8 public constant MAX_TIER = 100;

    struct IdentityProfile {
        uint256 identityId;
        uint8 tier;
        bool active;
        uint64 createdAt;
        uint64 updatedAt;
    }

    struct MetadataEntry {
        string key;
        bytes value;
    }

    uint256 public identityCount;
    mapping(address => IdentityProfile) private _profiles;
    mapping(uint256 => address) private _ownersByIdentity;
    mapping(uint256 => mapping(bytes32 => bytes)) private _metadata;
    mapping(uint256 => address) private _agentWallets;
    mapping(uint256 => uint64) public registeredAt;

    error ZeroAddress();
    error InvalidTier();
    error AlreadyRegistered();
    error NotRegistered();
    error IdentityNonTransferable();
    error NotAuthorized();
    error NotAgentOwner();
    error InvalidSignature();
    error SignatureExpired();

    event IdentityRegistered(address indexed wallet, uint256 indexed identityId, uint8 tier);
    event TierUpdated(address indexed wallet, uint8 tier);
    event IdentityActivated(address indexed wallet, bool active);
    event IdentityURIUpdated(address indexed wallet, uint256 indexed identityId, string uri);
    event IdentityRevoked(address indexed wallet, uint256 indexed identityId);

    event Registered(uint256 indexed agentId, string agentURI, address indexed owner);
    event URIUpdated(uint256 indexed agentId, string newURI, address indexed updatedBy);
    event MetadataSet(uint256 indexed agentId, string indexed indexedMetadataKey, string metadataKey, bytes metadataValue);
    event AgentWalletSet(uint256 indexed agentId, address indexed wallet);
    event AgentWalletUnset(uint256 indexed agentId);

    constructor(address admin)
        ERC721("Relay44 Agent Identity", "NRMD-AI")
        EIP712("Relay44 Agent Identity", "1")
    {
        if (admin == address(0)) revert ZeroAddress();
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
        _grantRole(ISSUER_ROLE, admin);
    }

    function register(address wallet, uint8 tier) external onlyRole(ISSUER_ROLE) whenNotPaused returns (uint256) {
        return _register(wallet, tier, "", false);
    }

    function register(string calldata agentURI, MetadataEntry[] calldata metadata)
        external
        whenNotPaused
        returns (uint256 agentId)
    {
        agentId = _register(msg.sender, 0, agentURI, true);

        for (uint256 i = 0; i < metadata.length; i++) {
            bytes32 keyHash = keccak256(bytes(metadata[i].key));
            if (keyHash == AGENT_WALLET_KEY) revert NotAuthorized();
            _metadata[agentId][keyHash] = metadata[i].value;
            emit MetadataSet(agentId, metadata[i].key, metadata[i].key, metadata[i].value);
        }
    }

    function register(string calldata agentURI) external whenNotPaused returns (uint256) {
        return _register(msg.sender, 0, agentURI, true);
    }

    function register() external whenNotPaused returns (uint256) {
        return _register(msg.sender, 0, "", true);
    }

    function registerIdentity(string calldata identityURI) external whenNotPaused returns (uint256) {
        return _register(msg.sender, 0, identityURI, true);
    }

    function registerIdentityFor(address wallet, string calldata identityURI, uint8 tier, bool active)
        external
        onlyRole(ISSUER_ROLE)
        whenNotPaused
        returns (uint256)
    {
        uint256 identityId = _register(wallet, tier, identityURI, true);
        if (!active) {
            IdentityProfile storage entry = _profiles[wallet];
            entry.active = false;
            entry.updatedAt = uint64(block.timestamp);
            emit IdentityActivated(wallet, false);
        }
        return identityId;
    }

    function updateIdentityURI(uint256 identityId, string calldata identityURI) public whenNotPaused {
        address wallet = _ownersByIdentity[identityId];
        if (wallet == address(0)) revert NotRegistered();
        if (msg.sender != wallet && !hasRole(ISSUER_ROLE, msg.sender)) revert NotAuthorized();

        _setTokenURI(identityId, identityURI);
        _profiles[wallet].updatedAt = uint64(block.timestamp);

        emit IdentityURIUpdated(wallet, identityId, identityURI);
        emit URIUpdated(identityId, identityURI, msg.sender);
    }

    function setAgentURI(uint256 identityId, string calldata newURI) external {
        updateIdentityURI(identityId, newURI);
    }

    function setMetadata(uint256 identityId, string calldata metadataKey, bytes calldata metadataValue)
        external
        whenNotPaused
    {
        address owner = _ownersByIdentity[identityId];
        if (owner == address(0)) revert NotRegistered();
        if (msg.sender != owner && !hasRole(ISSUER_ROLE, msg.sender)) revert NotAuthorized();

        bytes32 keyHash = keccak256(bytes(metadataKey));
        if (keyHash == AGENT_WALLET_KEY) revert NotAuthorized();

        _metadata[identityId][keyHash] = metadataValue;
        _profiles[owner].updatedAt = uint64(block.timestamp);

        emit MetadataSet(identityId, metadataKey, metadataKey, metadataValue);
    }

    function getMetadata(uint256 identityId, string calldata metadataKey) external view returns (bytes memory) {
        if (_ownersByIdentity[identityId] == address(0)) revert NotRegistered();
        return _metadata[identityId][keccak256(bytes(metadataKey))];
    }

    function setAgentWallet(uint256 identityId, address newWallet, uint256 deadline, bytes calldata signature)
        external
        whenNotPaused
    {
        address owner = _ownersByIdentity[identityId];
        if (owner == address(0)) revert NotRegistered();
        if (newWallet == address(0)) revert ZeroAddress();
        if (block.timestamp > deadline) revert SignatureExpired();
        if (msg.sender != owner && !hasRole(ISSUER_ROLE, msg.sender)) revert NotAgentOwner();

        bytes32 structHash = keccak256(abi.encode(SET_WALLET_TYPEHASH, identityId, newWallet, deadline));
        bytes32 digest = _hashTypedDataV4(structHash);
        address signer = ECDSA.recover(digest, signature);
        if (signer != newWallet) revert InvalidSignature();

        _agentWallets[identityId] = newWallet;
        _metadata[identityId][AGENT_WALLET_KEY] = abi.encodePacked(newWallet);
        _profiles[owner].updatedAt = uint64(block.timestamp);

        emit AgentWalletSet(identityId, newWallet);
    }

    function getAgentWallet(uint256 identityId) external view returns (address wallet) {
        if (_ownersByIdentity[identityId] == address(0)) revert NotRegistered();

        wallet = _agentWallets[identityId];
        if (wallet == address(0)) {
            wallet = _ownersByIdentity[identityId];
        }
    }

    function unsetAgentWallet(uint256 identityId) external whenNotPaused {
        address owner = _ownersByIdentity[identityId];
        if (owner == address(0)) revert NotRegistered();
        if (msg.sender != owner && !hasRole(ISSUER_ROLE, msg.sender)) revert NotAgentOwner();

        delete _agentWallets[identityId];
        delete _metadata[identityId][AGENT_WALLET_KEY];
        _profiles[owner].updatedAt = uint64(block.timestamp);

        emit AgentWalletUnset(identityId);
    }

    function revokeIdentity(uint256 identityId) external onlyRole(ISSUER_ROLE) whenNotPaused {
        address wallet = _ownersByIdentity[identityId];
        if (wallet == address(0)) revert NotRegistered();

        IdentityProfile storage entry = _profiles[wallet];
        entry.active = false;
        entry.updatedAt = uint64(block.timestamp);

        emit IdentityActivated(wallet, false);
        emit IdentityRevoked(wallet, identityId);
    }

    function isRegistered(address wallet) external view returns (bool) {
        return _profiles[wallet].identityId != 0;
    }

    function getAgentId(address wallet) external view returns (uint256) {
        return _profiles[wallet].identityId;
    }

    function registerable(address wallet) external view returns (bool) {
        return wallet != address(0) && _profiles[wallet].identityId == 0;
    }

    function setTier(address wallet, uint8 tier) external onlyRole(ISSUER_ROLE) whenNotPaused {
        if (tier > MAX_TIER) revert InvalidTier();

        IdentityProfile storage entry = _profiles[wallet];
        if (entry.identityId == 0) revert NotRegistered();

        entry.tier = tier;
        entry.updatedAt = uint64(block.timestamp);

        emit TierUpdated(wallet, tier);
    }

    function setActive(address wallet, bool active) external onlyRole(ISSUER_ROLE) whenNotPaused {
        IdentityProfile storage entry = _profiles[wallet];
        if (entry.identityId == 0) revert NotRegistered();

        entry.active = active;
        entry.updatedAt = uint64(block.timestamp);

        emit IdentityActivated(wallet, active);
    }

    function profile(address wallet)
        external
        view
        returns (uint256 identityId, uint8 tier, bool active, uint64 updatedAt)
    {
        IdentityProfile storage entry = _profiles[wallet];
        return (entry.identityId, entry.tier, entry.active, entry.updatedAt);
    }

    function identityOf(address wallet) external view returns (uint256) {
        return _profiles[wallet].identityId;
    }

    function ownerOfIdentity(uint256 identityId) external view returns (address) {
        return _ownersByIdentity[identityId];
    }

    function totalSupply() external view returns (uint256) {
        return identityCount;
    }

    function exists(uint256 identityId) external view returns (bool) {
        return _ownersByIdentity[identityId] != address(0);
    }

    function getGlobalId(uint256 identityId) external view returns (string memory) {
        if (_ownersByIdentity[identityId] == address(0)) revert NotRegistered();

        return string(
            abi.encodePacked(
                "eip155:",
                uint256(block.chainid).toString(),
                ":",
                Strings.toHexString(uint160(address(this)), 20),
                ":",
                identityId.toString()
            )
        );
    }

    function _register(address wallet, uint8 tier, string memory identityURI, bool emitRegisteredEvent)
        internal
        returns (uint256 identityId)
    {
        if (wallet == address(0)) revert ZeroAddress();
        if (tier > MAX_TIER) revert InvalidTier();
        if (_profiles[wallet].identityId != 0) revert AlreadyRegistered();

        identityId = ++identityCount;
        uint64 nowTs = uint64(block.timestamp);
        _profiles[wallet] = IdentityProfile({
            identityId: identityId,
            tier: tier,
            active: true,
            createdAt: nowTs,
            updatedAt: nowTs
        });
        _ownersByIdentity[identityId] = wallet;
        registeredAt[identityId] = nowTs;

        _mint(wallet, identityId);

        if (bytes(identityURI).length > 0) {
            _setTokenURI(identityId, identityURI);
            emit IdentityURIUpdated(wallet, identityId, identityURI);
            emit URIUpdated(identityId, identityURI, msg.sender);
        }

        emit IdentityRegistered(wallet, identityId, tier);
        if (emitRegisteredEvent) {
            emit Registered(identityId, identityURI, wallet);
        }
    }

    function _update(address to, uint256 tokenId, address auth) internal override(ERC721) returns (address) {
        address from = _ownerOf(tokenId);
        if (from != address(0) && to != address(0)) revert IdentityNonTransferable();
        return super._update(to, tokenId, auth);
    }

    function supportsInterface(bytes4 interfaceId) public view override(ERC721URIStorage, AccessControl) returns (bool) {
        return super.supportsInterface(interfaceId);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
}


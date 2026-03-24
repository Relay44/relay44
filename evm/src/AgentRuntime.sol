// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "openzeppelin-contracts/contracts/utils/ReentrancyGuard.sol";

interface IOrderBookAgent {
    function placeOrderFor(address maker, uint256 marketId, bool isYes, uint128 priceBps, uint128 size, uint64 expiry)
        external
        returns (uint256 orderId);
}

interface IAgentIdentityRegistry {
    function registerFor(address owner, string calldata agentURI) external returns (uint256 agentId);
}

contract AgentRuntime is AccessControl, Pausable, ReentrancyGuard {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    uint256 public constant MIN_PRICE_BPS = 1;
    uint256 public constant MAX_PRICE_BPS = 9_999;

    struct Agent {
        address owner;
        uint256 marketId;
        bool isYes;
        uint128 priceBps;
        uint128 size;
        uint64 cadence;
        uint64 expiryWindow;
        uint64 lastExecutedAt;
        bool active;
        string strategy;
    }

    uint256 public agentCount;
    mapping(uint256 => Agent) public agents;
    mapping(uint256 => uint256) public agentIdentityId;

    IOrderBookAgent public immutable orderBook;
    IAgentIdentityRegistry public identityRegistry;

    error ZeroAddress();
    error NotOwner();
    error InvalidConfig();
    error AgentNotFound();
    error AgentInactive();
    error ExecutionTooEarly();
    error IdentityRegistryNotConfigured();
    error IdentityAlreadyRegistered();

    event AgentCreated(
        uint256 indexed agentId,
        address indexed owner,
        uint256 indexed marketId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string strategy
    );
    event AgentUpdated(
        uint256 indexed agentId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string strategy
    );
    event AgentDeactivated(uint256 indexed agentId);
    event AgentExecuted(
        uint256 indexed agentId, uint256 indexed orderId, address indexed executor, uint64 executedAt, uint64 expiry
    );
    event IdentityRegistrySet(address indexed identityRegistry);
    event AgentIdentityLinked(uint256 indexed agentId, uint256 indexed identityId, address indexed owner);

    constructor(address admin, address orderBookAddress) {
        if (admin == address(0) || orderBookAddress == address(0)) revert ZeroAddress();

        orderBook = IOrderBookAgent(orderBookAddress);

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    function createAgent(
        uint256 marketId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string calldata strategy
    ) external whenNotPaused returns (uint256 agentId) {
        _validateConfig(priceBps, size, cadence, expiryWindow);

        agentId = ++agentCount;
        agents[agentId] = Agent({
            owner: msg.sender,
            marketId: marketId,
            isYes: isYes,
            priceBps: priceBps,
            size: size,
            cadence: cadence,
            expiryWindow: expiryWindow,
            lastExecutedAt: 0,
            active: true,
            strategy: strategy
        });

        emit AgentCreated(agentId, msg.sender, marketId, isYes, priceBps, size, cadence, expiryWindow, strategy);
    }

    function updateAgent(
        uint256 agentId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string calldata strategy
    ) external whenNotPaused {
        Agent storage agent = agents[agentId];
        if (agent.owner == address(0)) revert AgentNotFound();
        if (agent.owner != msg.sender) revert NotOwner();

        _validateConfig(priceBps, size, cadence, expiryWindow);

        agent.isYes = isYes;
        agent.priceBps = priceBps;
        agent.size = size;
        agent.cadence = cadence;
        agent.expiryWindow = expiryWindow;
        agent.strategy = strategy;
        agent.active = true;

        emit AgentUpdated(agentId, isYes, priceBps, size, cadence, expiryWindow, strategy);
    }

    function deactivateAgent(uint256 agentId) external {
        Agent storage agent = agents[agentId];
        if (agent.owner == address(0)) revert AgentNotFound();
        if (agent.owner != msg.sender && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotOwner();
        }

        agent.active = false;
        emit AgentDeactivated(agentId);
    }

    function setIdentityRegistry(address registry) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (registry == address(0)) revert ZeroAddress();
        identityRegistry = IAgentIdentityRegistry(registry);
        emit IdentityRegistrySet(registry);
    }

    function registerAgentIdentity(uint256 agentId, string calldata agentURI)
        external
        whenNotPaused
        returns (uint256 identityId)
    {
        Agent storage agent = agents[agentId];
        if (agent.owner == address(0)) revert AgentNotFound();
        if (agent.owner != msg.sender) revert NotOwner();
        if (address(identityRegistry) == address(0)) revert IdentityRegistryNotConfigured();
        if (agentIdentityId[agentId] != 0) revert IdentityAlreadyRegistered();

        identityId = identityRegistry.registerFor(agent.owner, agentURI);
        agentIdentityId[agentId] = identityId;

        emit AgentIdentityLinked(agentId, identityId, agent.owner);
    }

    function executeAgent(uint256 agentId) external nonReentrant whenNotPaused returns (uint256 orderId) {
        Agent storage agent = agents[agentId];
        if (agent.owner == address(0)) revert AgentNotFound();
        if (!agent.active) revert AgentInactive();

        uint64 nowTs = uint64(block.timestamp);
        uint64 nextExecution = agent.lastExecutedAt + agent.cadence;
        if (agent.lastExecutedAt != 0 && nowTs < nextExecution) {
            revert ExecutionTooEarly();
        }

        uint64 expiry = nowTs + agent.expiryWindow;
        orderId = orderBook.placeOrderFor(agent.owner, agent.marketId, agent.isYes, agent.priceBps, agent.size, expiry);

        agent.lastExecutedAt = nowTs;
        emit AgentExecuted(agentId, orderId, msg.sender, nowTs, expiry);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function _validateConfig(uint128 priceBps, uint128 size, uint64 cadence, uint64 expiryWindow) internal pure {
        if (priceBps < MIN_PRICE_BPS || priceBps > MAX_PRICE_BPS) revert InvalidConfig();
        if (size == 0 || cadence == 0 || expiryWindow == 0) revert InvalidConfig();
    }
}

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

interface IRelayBurnable {
    function burnFrom(address account, uint256 amount) external;
    function balanceOf(address account) external view returns (uint256);
}

contract AgentRuntime is AccessControl, Pausable, ReentrancyGuard {
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    uint256 public constant MIN_PRICE_BPS = 1;
    uint256 public constant MAX_PRICE_BPS = 9_999;

    struct AgentConfig {
        uint256 marketId;
        bool isYes;
        uint128 priceBps;
        uint128 size;
        uint64 cadence;
        uint64 expiryWindow;
    }

    struct AgentUpdateConfig {
        uint256 agentId;
        bool isYes;
        uint128 priceBps;
        uint128 size;
        uint64 cadence;
        uint64 expiryWindow;
    }

    struct Agent {
        address owner;
        address manager;
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
    mapping(address => mapping(address => bool)) public managerApprovals;

    IOrderBookAgent public immutable orderBook;
    IAgentIdentityRegistry public identityRegistry;
    IRelayBurnable public relayToken;
    uint256 public executionFee; // RELAY burned per agent execution

    error ZeroAddress();
    error NotOwner();
    error NotAuthorized();
    error InvalidConfig();
    error AgentNotFound();
    error AgentInactive();
    error ExecutionTooEarly();
    error IdentityRegistryNotConfigured();
    error IdentityAlreadyRegistered();
    error ManagerNotApproved();
    error InsufficientRelayForExecution();

    event AgentCreated(
        uint256 indexed agentId,
        address indexed owner,
        uint256 indexed marketId,
        address manager,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string strategy
    );
    event AgentUpdated(
        uint256 indexed agentId,
        address indexed manager,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string strategy
    );
    event AgentDeactivated(uint256 indexed agentId);
    event AgentManagerSet(uint256 indexed agentId, address indexed owner, address indexed manager);
    event AgentExecuted(
        uint256 indexed agentId, uint256 indexed orderId, address indexed executor, uint64 executedAt, uint64 expiry
    );
    event IdentityRegistrySet(address indexed identityRegistry);
    event AgentIdentityLinked(uint256 indexed agentId, uint256 indexed identityId, address indexed owner);
    event ManagerApprovalSet(address indexed owner, address indexed manager, bool approved);
    event ExecutionFeeUpdated(uint256 newFee);
    event RelayTokenSet(address indexed token);
    event ExecutionFeeBurned(uint256 indexed agentId, address indexed owner, uint256 amount);

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
        agentId = _createAgent(msg.sender, address(0), marketId, isYes, priceBps, size, cadence, expiryWindow, strategy);
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
        _applyUpdate(agent, isYes, priceBps, size, cadence, expiryWindow, strategy);
        emit AgentUpdated(agentId, agent.manager, isYes, priceBps, size, cadence, expiryWindow, strategy);
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

    function setManagerApproval(address manager, bool approved) external whenNotPaused {
        if (manager == address(0)) revert ZeroAddress();
        managerApprovals[msg.sender][manager] = approved;
        emit ManagerApprovalSet(msg.sender, manager, approved);
    }

    function createAgentsFor(address owner, address manager, AgentConfig[] calldata configs, string calldata strategy)
        external
        whenNotPaused
        returns (uint256[] memory agentIds)
    {
        _requireCreateAuthorization(owner, manager);
        uint256 length = configs.length;
        if (length == 0) revert InvalidConfig();

        agentIds = new uint256[](length);
        for (uint256 index = 0; index < length; ++index) {
            AgentConfig calldata config = configs[index];
            agentIds[index] = _createAgent(
                owner,
                manager,
                config.marketId,
                config.isYes,
                config.priceBps,
                config.size,
                config.cadence,
                config.expiryWindow,
                strategy
            );
        }
    }

    function updateAgents(AgentUpdateConfig[] calldata updates, string calldata strategy) external whenNotPaused {
        uint256 length = updates.length;
        if (length == 0) revert InvalidConfig();

        for (uint256 index = 0; index < length; ++index) {
            AgentUpdateConfig calldata config = updates[index];
            Agent storage agent = agents[config.agentId];
            if (agent.owner == address(0)) revert AgentNotFound();
            _requireManagerAuthorization(agent.owner, agent.manager);
            _applyUpdate(
                agent, config.isYes, config.priceBps, config.size, config.cadence, config.expiryWindow, strategy
            );
            emit AgentUpdated(
                config.agentId,
                agent.manager,
                config.isYes,
                config.priceBps,
                config.size,
                config.cadence,
                config.expiryWindow,
                strategy
            );
        }
    }

    function deactivateAgents(uint256[] calldata agentIds) external {
        uint256 length = agentIds.length;
        if (length == 0) revert InvalidConfig();

        for (uint256 index = 0; index < length; ++index) {
            Agent storage agent = agents[agentIds[index]];
            if (agent.owner == address(0)) revert AgentNotFound();
            _requireManagerAuthorization(agent.owner, agent.manager);
            agent.active = false;
            emit AgentDeactivated(agentIds[index]);
        }
    }

    function setAgentManager(uint256 agentId, address manager) external whenNotPaused {
        Agent storage agent = agents[agentId];
        if (agent.owner == address(0)) revert AgentNotFound();
        if (msg.sender != agent.owner && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotOwner();
        }
        if (
            manager != address(0) && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender) && !managerApprovals[agent.owner][manager]
        ) {
            revert ManagerNotApproved();
        }

        agent.manager = manager;
        emit AgentManagerSet(agentId, agent.owner, manager);
    }

    function setIdentityRegistry(address registry) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (registry == address(0)) revert ZeroAddress();
        identityRegistry = IAgentIdentityRegistry(registry);
        emit IdentityRegistrySet(registry);
    }

    function setRelayToken(address token) external onlyRole(DEFAULT_ADMIN_ROLE) {
        relayToken = IRelayBurnable(token); // address(0) disables burn
        emit RelayTokenSet(token);
    }

    function setExecutionFee(uint256 fee) external onlyRole(DEFAULT_ADMIN_ROLE) {
        executionFee = fee;
        emit ExecutionFeeUpdated(fee);
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

        // Burn RELAY execution fee from agent owner
        if (executionFee > 0 && address(relayToken) != address(0)) {
            if (relayToken.balanceOf(agent.owner) < executionFee) {
                revert InsufficientRelayForExecution();
            }
            relayToken.burnFrom(agent.owner, executionFee);
            emit ExecutionFeeBurned(agentId, agent.owner, executionFee);
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

    function _createAgent(
        address owner,
        address manager,
        uint256 marketId,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string memory strategy
    ) internal returns (uint256 agentId) {
        if (owner == address(0)) revert ZeroAddress();
        _validateConfig(priceBps, size, cadence, expiryWindow);

        agentId = ++agentCount;
        agents[agentId] = Agent({
            owner: owner,
            manager: manager,
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

        emit AgentCreated(agentId, owner, marketId, manager, isYes, priceBps, size, cadence, expiryWindow, strategy);
    }

    function _applyUpdate(
        Agent storage agent,
        bool isYes,
        uint128 priceBps,
        uint128 size,
        uint64 cadence,
        uint64 expiryWindow,
        string memory strategy
    ) internal {
        _validateConfig(priceBps, size, cadence, expiryWindow);

        agent.isYes = isYes;
        agent.priceBps = priceBps;
        agent.size = size;
        agent.cadence = cadence;
        agent.expiryWindow = expiryWindow;
        agent.strategy = strategy;
        agent.active = true;
    }

    function _requireCreateAuthorization(address owner, address manager) internal view {
        if (owner == address(0) || manager == address(0)) revert ZeroAddress();

        if (hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) return;
        if (!managerApprovals[owner][manager]) revert ManagerNotApproved();

        if (msg.sender == owner || msg.sender == manager) {
            return;
        }

        revert NotAuthorized();
    }

    function _requireManagerAuthorization(address owner, address manager) internal view {
        if (hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) return;
        if (msg.sender == owner) return;
        if (manager != address(0) && msg.sender == manager && managerApprovals[owner][manager]) {
            return;
        }
        revert NotAuthorized();
    }
}

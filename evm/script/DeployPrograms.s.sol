// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {AgentReputationRegistry} from "../src/AgentReputationRegistry.sol";
import {ERC8004IdentityRegistry} from "../src/ERC8004IdentityRegistry.sol";
import {ERC8004ReputationRegistry} from "../src/ERC8004ReputationRegistry.sol";
import {ERC8004ValidationRegistry} from "../src/ERC8004ValidationRegistry.sol";

interface IAccessControlLike {
    function grantRole(bytes32 role, address account) external;
    function revokeRole(bytes32 role, address account) external;
    function hasRole(bytes32 role, address account) external view returns (bool);
}

contract DeployProgramsScript is Script {
    error MissingAdmin();
    error MissingCollateralToken();
    error ZeroAddress(string field);

    struct RoleConfig {
        address admin;
        address bootstrapAdmin;
        address pauser;
        address resolver;
        address marketCreator;
        address operator;
        address runtimeOperator;
        address reputationOracle;
        address erc8004Issuer;
        address erc8004Attester;
        address erc8004ValidationManager;
        address erc8004Validator;
        address collateralToken;
    }

    function run() external {
        RoleConfig memory cfg = _loadConfig();

        vm.startBroadcast();
        _deploy(cfg);
        vm.stopBroadcast();
    }

    function _loadConfig() internal view returns (RoleConfig memory cfg) {
        cfg.admin = _envAddressOr("BASE_ADMIN", address(0));
        if (cfg.admin == address(0)) revert MissingAdmin();

        cfg.bootstrapAdmin = _envAddressOr("BOOTSTRAP_ADMIN", cfg.admin);
        if (cfg.bootstrapAdmin == address(0)) revert ZeroAddress("BOOTSTRAP_ADMIN");

        cfg.pauser = _envAddressOr("BASE_PAUSER", cfg.admin);
        cfg.resolver = _envAddressOr("BASE_RESOLVER", cfg.admin);
        cfg.marketCreator = _envAddressOr("BASE_MARKET_CREATOR", cfg.admin);
        cfg.operator = _envAddressOr("BASE_OPERATOR", cfg.admin);
        cfg.runtimeOperator = _envAddressOr("BASE_AGENT_RUNTIME_OPERATOR", address(0));
        cfg.reputationOracle = _envAddressOr("BASE_REPUTATION_ORACLE", cfg.operator);
        cfg.erc8004Issuer = _envAddressOr("BASE_IDENTITY_ISSUER", cfg.admin);
        cfg.erc8004Attester = _envAddressOr("BASE_REPUTATION_ATTESTER", cfg.reputationOracle);
        cfg.erc8004ValidationManager = _envAddressOr("BASE_VALIDATION_MANAGER", cfg.admin);
        cfg.erc8004Validator = _envAddressOr("BASE_VALIDATION_VALIDATOR", cfg.erc8004Attester);
        cfg.collateralToken = _resolveCollateralToken();
        if (cfg.collateralToken == address(0)) revert MissingCollateralToken();
    }

    function _deploy(RoleConfig memory cfg) internal {
        MarketCore marketCore = new MarketCore(cfg.bootstrapAdmin);
        CollateralVault collateralVault = new CollateralVault(cfg.bootstrapAdmin, cfg.collateralToken);
        OrderBook orderBook = new OrderBook(
            cfg.bootstrapAdmin,
            address(marketCore),
            address(collateralVault),
            vm.envOr("R44_TOKEN_ADDRESS", address(0))
        );
        AgentRuntime agentRuntime = new AgentRuntime(cfg.bootstrapAdmin, address(orderBook));
        AgentIdentityRegistry identityRegistry = new AgentIdentityRegistry(cfg.bootstrapAdmin);
        AgentReputationRegistry reputationRegistry =
            new AgentReputationRegistry(cfg.bootstrapAdmin, address(identityRegistry));
        ERC8004IdentityRegistry erc8004IdentityRegistry = new ERC8004IdentityRegistry(cfg.bootstrapAdmin);
        ERC8004ReputationRegistry erc8004ReputationRegistry =
            new ERC8004ReputationRegistry(cfg.bootstrapAdmin, address(erc8004IdentityRegistry));
        ERC8004ValidationRegistry erc8004ValidationRegistry =
            new ERC8004ValidationRegistry(cfg.bootstrapAdmin, address(erc8004IdentityRegistry));

        _configureMarketCore(marketCore, cfg.bootstrapAdmin, cfg.admin, cfg.marketCreator, cfg.resolver, cfg.pauser);
        _configureCollateralVault(collateralVault, cfg.bootstrapAdmin, cfg.admin, cfg.operator, cfg.pauser, address(orderBook));
        _configureOrderBook(orderBook, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, address(agentRuntime), cfg.runtimeOperator);
        _configureIdentityRegistry(identityRegistry, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, address(agentRuntime));
        _configureReputationRegistry(reputationRegistry, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, cfg.reputationOracle);
        _configureErc8004IdentityRegistry(erc8004IdentityRegistry, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, cfg.erc8004Issuer);
        _configureErc8004ReputationRegistry(erc8004ReputationRegistry, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, cfg.erc8004Attester);
        _configureErc8004ValidationRegistry(
            erc8004ValidationRegistry, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, cfg.erc8004ValidationManager, cfg.erc8004Validator
        );
        _configureAgentRuntime(agentRuntime, cfg.bootstrapAdmin, cfg.admin, cfg.pauser, address(identityRegistry));

        console2.log("chainId:", block.chainid);
        console2.log("admin:", cfg.admin);
        console2.log("bootstrapAdmin:", cfg.bootstrapAdmin);
        console2.log("MarketCore:", address(marketCore));
        console2.log("CollateralVault:", address(collateralVault));
        console2.log("OrderBook:", address(orderBook));
        console2.log("AgentRuntime:", address(agentRuntime));
        console2.log("AgentIdentityRegistry:", address(identityRegistry));
        console2.log("AgentReputationRegistry:", address(reputationRegistry));
        console2.log("ERC8004IdentityRegistry:", address(erc8004IdentityRegistry));
        console2.log("ERC8004ReputationRegistry:", address(erc8004ReputationRegistry));
        console2.log("ERC8004ValidationRegistry:", address(erc8004ValidationRegistry));
    }

    function _configureMarketCore(
        MarketCore marketCore,
        address bootstrapAdmin,
        address admin,
        address marketCreator,
        address resolver,
        address pauser
    ) internal {
        bytes32 defaultAdminRole = marketCore.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(marketCore)), marketCore.MARKET_CREATOR_ROLE(), marketCreator);
        _grantRoleIfMissing(IAccessControlLike(address(marketCore)), marketCore.RESOLVER_ROLE(), resolver);
        _grantRoleIfMissing(IAccessControlLike(address(marketCore)), marketCore.PAUSER_ROLE(), pauser);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(marketCore)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(marketCore)), marketCore.MARKET_CREATOR_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(marketCore)), marketCore.RESOLVER_ROLE(), bootstrapAdmin);
            _revokeRoleIfPresent(IAccessControlLike(address(marketCore)), marketCore.PAUSER_ROLE(), bootstrapAdmin);
            _revokeRoleIfPresent(IAccessControlLike(address(marketCore)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureOrderBook(
        OrderBook orderBook,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address agentRuntime,
        address runtimeOperator
    ) internal {
        bytes32 defaultAdminRole = orderBook.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(orderBook)), orderBook.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(IAccessControlLike(address(orderBook)), orderBook.AGENT_RUNTIME_ROLE(), agentRuntime);
        if (runtimeOperator != address(0)) {
            _grantRoleIfMissing(IAccessControlLike(address(orderBook)), orderBook.AGENT_RUNTIME_ROLE(), runtimeOperator);
        }

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(orderBook)), defaultAdminRole, admin);
            _revokeRoleIfPresent(IAccessControlLike(address(orderBook)), orderBook.PAUSER_ROLE(), bootstrapAdmin);
            _revokeRoleIfPresent(IAccessControlLike(address(orderBook)), orderBook.AGENT_RUNTIME_ROLE(), bootstrapAdmin);
            _revokeRoleIfPresent(IAccessControlLike(address(orderBook)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureCollateralVault(
        CollateralVault collateralVault,
        address bootstrapAdmin,
        address admin,
        address operator,
        address pauser,
        address orderBook
    ) internal {
        bytes32 defaultAdminRole = collateralVault.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(collateralVault)), collateralVault.OPERATOR_ROLE(), operator);
        _grantRoleIfMissing(IAccessControlLike(address(collateralVault)), collateralVault.OPERATOR_ROLE(), orderBook);
        _grantRoleIfMissing(IAccessControlLike(address(collateralVault)), collateralVault.PAUSER_ROLE(), pauser);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(collateralVault)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(collateralVault)), collateralVault.OPERATOR_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(collateralVault)), collateralVault.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(collateralVault)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureAgentRuntime(
        AgentRuntime agentRuntime,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address identityRegistry
    ) internal {
        bytes32 defaultAdminRole = agentRuntime.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(agentRuntime)), agentRuntime.PAUSER_ROLE(), pauser);
        agentRuntime.setIdentityRegistry(identityRegistry);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(agentRuntime)), defaultAdminRole, admin);
            _revokeRoleIfPresent(IAccessControlLike(address(agentRuntime)), agentRuntime.PAUSER_ROLE(), bootstrapAdmin);
            _revokeRoleIfPresent(IAccessControlLike(address(agentRuntime)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureIdentityRegistry(
        AgentIdentityRegistry identityRegistry,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address registrar
    ) internal {
        bytes32 defaultAdminRole = identityRegistry.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), identityRegistry.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), identityRegistry.REGISTRAR_ROLE(), registrar);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(identityRegistry)), identityRegistry.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(identityRegistry)), identityRegistry.REGISTRAR_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(identityRegistry)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureReputationRegistry(
        AgentReputationRegistry reputationRegistry,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address oracle
    ) internal {
        bytes32 defaultAdminRole = reputationRegistry.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(reputationRegistry)), reputationRegistry.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(IAccessControlLike(address(reputationRegistry)), reputationRegistry.ORACLE_ROLE(), oracle);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(reputationRegistry)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(reputationRegistry)), reputationRegistry.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(reputationRegistry)), reputationRegistry.ORACLE_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(reputationRegistry)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureErc8004IdentityRegistry(
        ERC8004IdentityRegistry identityRegistry,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address issuer
    ) internal {
        bytes32 defaultAdminRole = identityRegistry.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), identityRegistry.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), identityRegistry.ISSUER_ROLE(), issuer);

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(identityRegistry)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(identityRegistry)), identityRegistry.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(identityRegistry)), identityRegistry.ISSUER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(identityRegistry)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureErc8004ReputationRegistry(
        ERC8004ReputationRegistry reputationRegistry,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address attester
    ) internal {
        bytes32 defaultAdminRole = reputationRegistry.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(reputationRegistry)), reputationRegistry.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(
            IAccessControlLike(address(reputationRegistry)), reputationRegistry.ATTESTER_ROLE(), attester
        );

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(reputationRegistry)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(reputationRegistry)), reputationRegistry.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(reputationRegistry)), reputationRegistry.ATTESTER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(reputationRegistry)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _configureErc8004ValidationRegistry(
        ERC8004ValidationRegistry validationRegistry,
        address bootstrapAdmin,
        address admin,
        address pauser,
        address validatorManager,
        address validator
    ) internal {
        bytes32 defaultAdminRole = validationRegistry.DEFAULT_ADMIN_ROLE();

        _grantRoleIfMissing(IAccessControlLike(address(validationRegistry)), validationRegistry.PAUSER_ROLE(), pauser);
        _grantRoleIfMissing(
            IAccessControlLike(address(validationRegistry)),
            validationRegistry.VALIDATOR_MANAGER_ROLE(),
            validatorManager
        );
        if (!validationRegistry.isValidator(validator)) {
            validationRegistry.addValidator(validator);
        }

        if (bootstrapAdmin != admin) {
            _grantRoleIfMissing(IAccessControlLike(address(validationRegistry)), defaultAdminRole, admin);
            _revokeRoleIfPresent(
                IAccessControlLike(address(validationRegistry)), validationRegistry.PAUSER_ROLE(), bootstrapAdmin
            );
            _revokeRoleIfPresent(
                IAccessControlLike(address(validationRegistry)),
                validationRegistry.VALIDATOR_MANAGER_ROLE(),
                bootstrapAdmin
            );
            _revokeRoleIfPresent(IAccessControlLike(address(validationRegistry)), defaultAdminRole, bootstrapAdmin);
        }
    }

    function _grantRoleIfMissing(IAccessControlLike target, bytes32 role, address account) internal {
        if (account == address(0)) revert ZeroAddress("role-account");
        if (!target.hasRole(role, account)) {
            target.grantRole(role, account);
        }
    }

    function _revokeRoleIfPresent(IAccessControlLike target, bytes32 role, address account) internal {
        if (target.hasRole(role, account)) {
            target.revokeRole(role, account);
        }
    }

    function _envAddressOr(string memory key, address fallbackValue) internal view returns (address) {
        try vm.envAddress(key) returns (address value) {
            return value;
        } catch {
            return fallbackValue;
        }
    }

    function _resolveCollateralToken() internal view returns (address) {
        address collateralToken = _envAddressOr("COLLATERAL_TOKEN_ADDRESS", address(0));
        if (collateralToken != address(0)) return collateralToken;

        if (block.chainid == 8453) {
            return _envAddressOr("COLLATERAL_TOKEN_BASE_MAINNET", address(0));
        }
        if (block.chainid == 84532) {
            return _envAddressOr("COLLATERAL_TOKEN_BASE_SEPOLIA", address(0));
        }

        return address(0);
    }
}

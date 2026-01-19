// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {R44Token} from "../src/R44Token.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {AgentReputationRegistry} from "../src/AgentReputationRegistry.sol";
import {ERC8004IdentityRegistry} from "../src/ERC8004IdentityRegistry.sol";
import {ERC8004ReputationRegistry} from "../src/ERC8004ReputationRegistry.sol";
import {ERC8004ValidationRegistry} from "../src/ERC8004ValidationRegistry.sol";

contract DeployCoreScript is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address treasury = vm.envAddress("BASE_TREASURY");
        uint256 cap = vm.envUint("R44_CAP_WEI");
        uint256 initialSupply = vm.envUint("R44_INITIAL_SUPPLY_WEI");

        vm.startBroadcast();

        R44Token token = new R44Token("Relay44", "R44", cap, admin, treasury, initialSupply);
        MarketCore marketCore = new MarketCore(admin);

        address collateralToken = vm.envOr("COLLATERAL_TOKEN_ADDRESS", address(0));
        if (collateralToken == address(0)) {
            collateralToken = address(token);
        }

        CollateralVault collateralVault = new CollateralVault(admin, collateralToken);
        OrderBook orderBook = new OrderBook(admin, address(marketCore), address(collateralVault));
        AgentRuntime agentRuntime = new AgentRuntime(admin, address(orderBook));
        AgentIdentityRegistry identityRegistry = new AgentIdentityRegistry(admin);
        AgentReputationRegistry reputationRegistry = new AgentReputationRegistry(admin, address(identityRegistry));
        ERC8004IdentityRegistry erc8004IdentityRegistry = new ERC8004IdentityRegistry(admin);
        ERC8004ReputationRegistry erc8004ReputationRegistry =
            new ERC8004ReputationRegistry(admin, address(erc8004IdentityRegistry));
        ERC8004ValidationRegistry erc8004ValidationRegistry =
            new ERC8004ValidationRegistry(admin, address(erc8004IdentityRegistry));

        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        orderBook.grantRole(orderBook.AGENT_RUNTIME_ROLE(), address(agentRuntime));
        identityRegistry.grantRole(identityRegistry.REGISTRAR_ROLE(), address(agentRuntime));
        reputationRegistry.grantRole(reputationRegistry.ORACLE_ROLE(), admin);
        erc8004IdentityRegistry.grantRole(erc8004IdentityRegistry.ISSUER_ROLE(), admin);
        erc8004ReputationRegistry.grantRole(erc8004ReputationRegistry.ATTESTER_ROLE(), admin);
        erc8004ValidationRegistry.addValidator(admin);
        agentRuntime.setIdentityRegistry(address(identityRegistry));

        vm.stopBroadcast();

        console2.log("R44Token:", address(token));
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
}

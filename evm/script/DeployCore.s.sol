// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {RelayToken} from "../src/RelayToken.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OrderBook} from "../src/OrderBook.sol";
import {CollateralVault} from "../src/CollateralVault.sol";
import {AgentRuntime} from "../src/AgentRuntime.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {AgentReputationRegistry} from "../src/AgentReputationRegistry.sol";
import {ERC8004IdentityRegistry} from "../src/ERC8004IdentityRegistry.sol";
import {ERC8004ReputationRegistry} from "../src/ERC8004ReputationRegistry.sol";
import {ERC8004ValidationRegistry} from "../src/ERC8004ValidationRegistry.sol";
import {RelayStaking} from "../src/RelayStaking.sol";
import {RewardDistributor} from "../src/RewardDistributor.sol";

contract DeployCoreScript is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address treasury = vm.envAddress("BASE_TREASURY");
        uint256 cap = vm.envUint("RELAY_CAP_WEI");
        uint256 initialSupply = vm.envUint("RELAY_INITIAL_SUPPLY_WEI");

        vm.startBroadcast();

        RelayToken token = new RelayToken("Relay", "RELAY", cap, admin, treasury, initialSupply);
        address tokenAddr = address(token);

        MarketCore marketCore = new MarketCore(admin);

        address collateralToken = vm.envOr("COLLATERAL_TOKEN_ADDRESS", address(0));
        if (collateralToken == address(0)) collateralToken = tokenAddr;

        CollateralVault collateralVault = new CollateralVault(admin, collateralToken);
        OrderBook orderBook = new OrderBook(admin, address(marketCore), address(collateralVault), tokenAddr);
        AgentRuntime agentRuntime = new AgentRuntime(admin, address(orderBook));

        _deployIdentity(admin, address(agentRuntime));
        _deployTokenomics(admin, tokenAddr, treasury, address(agentRuntime), address(marketCore), address(orderBook));

        // Core roles
        collateralVault.grantRole(collateralVault.OPERATOR_ROLE(), address(orderBook));
        orderBook.grantRole(orderBook.AGENT_RUNTIME_ROLE(), address(agentRuntime));
        orderBook.setFeeConfig(100, treasury);

        vm.stopBroadcast();

        console2.log("RelayToken:", tokenAddr);
        console2.log("MarketCore:", address(marketCore));
        console2.log("CollateralVault:", address(collateralVault));
        console2.log("OrderBook:", address(orderBook));
        console2.log("AgentRuntime:", address(agentRuntime));
    }

    function _deployIdentity(address admin, address agentRuntimeAddr) internal {
        AgentIdentityRegistry identityRegistry = new AgentIdentityRegistry(admin);
        AgentReputationRegistry reputationRegistry = new AgentReputationRegistry(admin, address(identityRegistry));
        ERC8004IdentityRegistry erc8004Id = new ERC8004IdentityRegistry(admin);
        ERC8004ReputationRegistry erc8004Rep = new ERC8004ReputationRegistry(admin, address(erc8004Id));
        ERC8004ValidationRegistry erc8004Val = new ERC8004ValidationRegistry(admin, address(erc8004Id));

        identityRegistry.grantRole(identityRegistry.REGISTRAR_ROLE(), agentRuntimeAddr);
        reputationRegistry.grantRole(reputationRegistry.ORACLE_ROLE(), admin);
        erc8004Id.grantRole(erc8004Id.ISSUER_ROLE(), admin);
        erc8004Rep.grantRole(erc8004Rep.ATTESTER_ROLE(), admin);
        erc8004Val.addValidator(admin);

        AgentRuntime(agentRuntimeAddr).setIdentityRegistry(address(identityRegistry));

        console2.log("AgentIdentityRegistry:", address(identityRegistry));
        console2.log("AgentReputationRegistry:", address(reputationRegistry));
        console2.log("ERC8004IdentityRegistry:", address(erc8004Id));
        console2.log("ERC8004ReputationRegistry:", address(erc8004Rep));
        console2.log("ERC8004ValidationRegistry:", address(erc8004Val));
    }

    function _deployTokenomics(
        address admin,
        address tokenAddr,
        address treasury,
        address agentRuntimeAddr,
        address marketCoreAddr,
        address orderBookAddr
    ) internal {
        RelayStaking staking = new RelayStaking(admin, tokenAddr);
        uint256 epochDuration = vm.envOr("EPOCH_DURATION", uint256(7 days));
        RewardDistributor distributor = new RewardDistributor(admin, tokenAddr, treasury, epochDuration);

        RelayToken(tokenAddr).grantRole(RelayToken(tokenAddr).BURNER_ROLE(), agentRuntimeAddr);
        AgentRuntime(agentRuntimeAddr).setRelayToken(tokenAddr);
        MarketCore(marketCoreAddr).setRelayToken(tokenAddr);
        staking.grantRole(staking.DISTRIBUTOR_ROLE(), address(distributor));
        distributor.setStakingPool(address(staking));

        console2.log("RelayStaking:", address(staking));
        console2.log("RewardDistributor:", address(distributor));
    }
}

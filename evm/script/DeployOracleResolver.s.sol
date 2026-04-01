// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {MarketCore} from "../src/MarketCore.sol";
import {OracleResolver} from "../src/OracleResolver.sol";

contract DeployOracleResolverScript is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address marketCoreAddress = vm.envAddress("MARKET_CORE_ADDRESS");
        address keeperAddress = vm.envAddress("ORACLE_KEEPER_ADDRESS");

        vm.startBroadcast();

        OracleResolver resolver = new OracleResolver(admin, marketCoreAddress);

        // Grant RESOLVER_ROLE on MarketCore so OracleResolver can call resolveMarket
        MarketCore(marketCoreAddress).grantRole(
            MarketCore(marketCoreAddress).RESOLVER_ROLE(), address(resolver)
        );

        // Grant CONFIGURATOR_ROLE to keeper wallet
        resolver.grantRole(resolver.CONFIGURATOR_ROLE(), keeperAddress);

        vm.stopBroadcast();

        console2.log("OracleResolver:", address(resolver));
        console2.log("Keeper:", keeperAddress);
    }
}

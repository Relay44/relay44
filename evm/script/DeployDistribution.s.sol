// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {DistributionMarket} from "../src/DistributionMarket.sol";
import {CollateralVault} from "../src/CollateralVault.sol";

/// @notice Deploy DistributionMarket and grant it OPERATOR_ROLE on CollateralVault.
///
/// Required env vars:
///   BASE_ADMIN            — admin address (gets DEFAULT_ADMIN_ROLE, MARKET_CREATOR_ROLE, OPERATOR_ROLE)
///   COLLATERAL_VAULT      — existing CollateralVault address
///   COLLATERAL_TOKEN      — primary collateral token (e.g. USDC) address
///   RELAY_TOKEN           — RELAY token address
///   BACKEND_OPERATOR      — backend wallet that calls updateMarketState (gets OPERATOR_ROLE)
///
/// Optional:
///   ORACLE_RESOLVER       — OracleResolver address for resolveFromOracle()
///
/// Usage:
///   forge script script/DeployDistribution.s.sol --rpc-url $RPC_URL --broadcast --verify
contract DeployDistributionScript is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address vaultAddr = vm.envAddress("COLLATERAL_VAULT");
        address collateralToken = vm.envAddress("COLLATERAL_TOKEN");
        address relayToken = vm.envAddress("RELAY_TOKEN");
        address backend = vm.envAddress("BACKEND_OPERATOR");

        vm.startBroadcast();

        DistributionMarket distMarket = new DistributionMarket(admin, vaultAddr, collateralToken, relayToken);

        // Grant roles
        distMarket.grantRole(distMarket.MARKET_CREATOR_ROLE(), admin);
        distMarket.grantRole(distMarket.OPERATOR_ROLE(), backend);

        // Grant DistributionMarket OPERATOR_ROLE on CollateralVault for escrow ops
        CollateralVault vault = CollateralVault(vaultAddr);
        vault.grantRole(vault.OPERATOR_ROLE(), address(distMarket));

        vm.stopBroadcast();

        console2.log("DistributionMarket:", address(distMarket));
        console2.log("  admin:", admin);
        console2.log("  backend operator:", backend);
        console2.log("  CollateralVault:", vaultAddr);
    }
}

// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {ERC8004IdentityRegistry} from "../src/ERC8004IdentityRegistry.sol";
import {ERC8004ReputationRegistry} from "../src/ERC8004ReputationRegistry.sol";
import {ERC8004ValidationRegistry} from "../src/ERC8004ValidationRegistry.sol";

contract DeployERC8004Script is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");

        vm.startBroadcast();

        ERC8004IdentityRegistry identity = new ERC8004IdentityRegistry(admin);
        ERC8004ReputationRegistry reputation = new ERC8004ReputationRegistry(admin, address(identity));
        ERC8004ValidationRegistry validation = new ERC8004ValidationRegistry(admin, address(identity));

        identity.grantRole(identity.ISSUER_ROLE(), admin);
        reputation.grantRole(reputation.ATTESTER_ROLE(), admin);
        validation.addValidator(admin);

        vm.stopBroadcast();

        console2.log("ERC8004IdentityRegistry:", address(identity));
        console2.log("ERC8004ReputationRegistry:", address(reputation));
        console2.log("ERC8004ValidationRegistry:", address(validation));
    }
}

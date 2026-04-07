// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {RelayStaking} from "../src/RelayStaking.sol";
import {RewardDistributor} from "../src/RewardDistributor.sol";

contract DeployTokenomicsScript is Script {
    function run() external {
        address admin = vm.envAddress("BASE_ADMIN");
        address treasury = vm.envAddress("BASE_TREASURY");
        address relayToken = vm.envAddress("RELAY_TOKEN_ADDRESS");
        uint256 epochDuration = vm.envOr("EPOCH_DURATION", uint256(7 days));

        vm.startBroadcast();

        RelayStaking staking = new RelayStaking(admin, relayToken);
        RewardDistributor distributor = new RewardDistributor(admin, relayToken, treasury, epochDuration);

        staking.grantRole(staking.DISTRIBUTOR_ROLE(), address(distributor));
        distributor.setStakingPool(address(staking));

        vm.stopBroadcast();

        console2.log("RelayStaking:", address(staking));
        console2.log("RewardDistributor:", address(distributor));
        console2.log("RelayToken:", relayToken);
        console2.log("Admin:", admin);
        console2.log("Treasury:", treasury);
    }
}

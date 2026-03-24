// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {TimelockController} from "openzeppelin-contracts/contracts/governance/TimelockController.sol";

contract DeployTimelockScript is Script {
    error ZeroAddress(string field);

    function run() external {
        address admin = _envAddressOr("TIMELOCK_ADMIN", _envAddressOr("BASE_ADMIN", address(0)));
        address proposer = _envAddressOr("TIMELOCK_PROPOSER", admin);
        address executor = _envAddressOr("TIMELOCK_EXECUTOR", admin);
        uint256 minDelay = _envUintOr("TIMELOCK_MIN_DELAY", 12 hours);

        if (admin == address(0)) revert ZeroAddress("TIMELOCK_ADMIN");
        if (proposer == address(0)) revert ZeroAddress("TIMELOCK_PROPOSER");
        if (executor == address(0)) revert ZeroAddress("TIMELOCK_EXECUTOR");

        address[] memory proposers = new address[](1);
        proposers[0] = proposer;
        address[] memory executors = new address[](1);
        executors[0] = executor;

        vm.startBroadcast();
        TimelockController timelock = new TimelockController(minDelay, proposers, executors, admin);
        vm.stopBroadcast();

        console2.log("timelock:", address(timelock));
        console2.log("minDelay:", minDelay);
        console2.log("proposer:", proposer);
        console2.log("executor:", executor);
        console2.log("admin:", admin);
    }

    function _envAddressOr(string memory key, address fallbackValue) internal view returns (address) {
        try vm.envAddress(key) returns (address value) {
            return value;
        } catch {
            return fallbackValue;
        }
    }

    function _envUintOr(string memory key, uint256 fallbackValue) internal view returns (uint256) {
        try vm.envUint(key) returns (uint256 value) {
            return value;
        } catch {
            return fallbackValue;
        }
    }
}

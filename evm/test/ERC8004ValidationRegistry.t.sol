// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {ERC8004IdentityRegistry} from "../src/ERC8004IdentityRegistry.sol";
import {ERC8004ValidationRegistry} from "../src/ERC8004ValidationRegistry.sol";

contract ERC8004ValidationRegistryTest is Test {
    address internal admin = makeAddr("admin");
    address internal issuer = makeAddr("issuer");
    address internal alice = makeAddr("alice");
    address internal validator = makeAddr("validator");
    address internal requester = makeAddr("requester");

    ERC8004IdentityRegistry internal identityRegistry;
    ERC8004ValidationRegistry internal validationRegistry;

    function setUp() external {
        identityRegistry = new ERC8004IdentityRegistry(admin);
        validationRegistry = new ERC8004ValidationRegistry(admin, address(identityRegistry));

        vm.startPrank(admin);
        identityRegistry.grantRole(identityRegistry.ISSUER_ROLE(), issuer);
        validationRegistry.addValidator(validator);
        vm.stopPrank();

        vm.prank(issuer);
        identityRegistry.register(alice, 40);
    }

    function test_validationRequestAndResponse() external {
        bytes32 requestHash = keccak256("request-1");

        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://validation/1", requestHash);

        bytes32[] memory requests = validationRegistry.getAgentValidations(1);
        assertEq(requests.length, 1);
        assertEq(requests[0], requestHash);

        vm.prank(validator);
        validationRegistry.validationResponse(requestHash, 88, "ipfs://validation/response/1", keccak256("r1"), keccak256("quality"));

        (address assignedValidator, uint256 agentId, uint8 response, bytes32 responseHash, bytes32 tag, uint64 lastUpdate) =
            validationRegistry.getValidationStatus(requestHash);

        assertEq(assignedValidator, validator);
        assertEq(agentId, 1);
        assertEq(response, 88);
        assertEq(responseHash, keccak256("r1"));
        assertEq(tag, keccak256("quality"));
        assertGt(lastUpdate, 0);
    }

    function test_validationRequestDerivesHashWhenZero() external {
        string memory uri = "ipfs://validation/derived";
        bytes32 derivedHash = keccak256(bytes(uri));

        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, uri, bytes32(0));

        bytes32[] memory requests = validationRegistry.getAgentValidations(1);
        assertEq(requests.length, 1);
        assertEq(requests[0], derivedHash);
    }

    function test_validationSummaryWithTagFilter() external {
        bytes32 requestHash1 = keccak256("request-a");
        bytes32 requestHash2 = keccak256("request-b");
        bytes32 qualityTag = keccak256("quality");
        bytes32 speedTag = keccak256("speed");

        vm.startPrank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://a", requestHash1);
        validationRegistry.validationRequest(validator, 1, "ipfs://b", requestHash2);
        vm.stopPrank();

        vm.startPrank(validator);
        validationRegistry.validationResponse(requestHash1, 90, "ipfs://ra", keccak256("ra"), qualityTag);
        validationRegistry.validationResponse(requestHash2, 70, "ipfs://rb", keccak256("rb"), speedTag);
        vm.stopPrank();

        address[] memory validators = new address[](1);
        validators[0] = validator;

        (uint64 countAll, uint8 avgAll) = validationRegistry.getSummary(1, validators, bytes32(0));
        assertEq(countAll, 2);
        assertEq(avgAll, 80);

        (uint64 countQuality, uint8 avgQuality) = validationRegistry.getSummary(1, validators, qualityTag);
        assertEq(countQuality, 1);
        assertEq(avgQuality, 90);
    }

    function test_validationResponseFromTier() external {
        bytes32 requestHash = keccak256("request-tier");

        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://tier", requestHash);

        vm.prank(validator);
        validationRegistry.validationResponseFromTier(requestHash, 3, "ipfs://tier-response", keccak256("tier-response"));

        (, , uint8 response, , bytes32 tag,) = validationRegistry.getValidationStatus(requestHash);
        assertEq(response, 80);
        assertEq(tag, keccak256("validation_tier"));
        assertEq(validationRegistry.responseToTier(response), 3);
    }

    function test_validationRejectsDuplicateRequest() external {
        bytes32 requestHash = keccak256("request-dup");

        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://dup", requestHash);

        vm.expectRevert(ERC8004ValidationRegistry.DuplicateValidationRequest.selector);
        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://dup", requestHash);
    }

    function test_validationRejectsUnassignedResponder() external {
        bytes32 requestHash = keccak256("request-unassigned");
        address intruder = makeAddr("intruder");

        vm.prank(requester);
        validationRegistry.validationRequest(validator, 1, "ipfs://intruder", requestHash);

        vm.expectRevert(ERC8004ValidationRegistry.NotValidator.selector);
        vm.prank(intruder);
        validationRegistry.validationResponse(requestHash, 80, "ipfs://bad", keccak256("bad"), bytes32(0));
    }
}


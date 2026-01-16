// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {AgentIdentityRegistry} from "../src/AgentIdentityRegistry.sol";
import {AgentReputationRegistry} from "../src/AgentReputationRegistry.sol";

contract AgentReputationRegistryTest is Test {
    address internal admin = makeAddr("admin");
    address internal owner = makeAddr("owner");
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");
    address internal oracle = makeAddr("oracle");

    AgentIdentityRegistry internal identityRegistry;
    AgentReputationRegistry internal reputationRegistry;
    uint256 internal agentId;

    function setUp() external {
        identityRegistry = new AgentIdentityRegistry(admin);
        reputationRegistry = new AgentReputationRegistry(admin, address(identityRegistry));

        vm.startPrank(admin);
        identityRegistry.grantRole(identityRegistry.REGISTRAR_ROLE(), admin);
        reputationRegistry.grantRole(reputationRegistry.ORACLE_ROLE(), oracle);
        agentId = identityRegistry.registerFor(owner, "ipfs://agent/owner");
        vm.stopPrank();
    }

    function test_giveFeedbackAndList() external {
        vm.prank(alice);
        reputationRegistry.giveFeedback(
            agentId,
            AgentReputationRegistry.FeedbackInput({
                value: int128(int256(1250)),
                valueDecimals: 2,
                category: "pnl",
                comment: "good execution",
                endpoint: "relay44://session/1",
                feedbackURI: "ipfs://feedback/1",
                feedbackHash: keccak256("feedback-1")
            })
        );

        vm.prank(bob);
        reputationRegistry.giveFeedback(
            agentId,
            AgentReputationRegistry.FeedbackInput({
                value: int128(int256(-350)),
                valueDecimals: 2,
                category: "risk",
                comment: "late execution",
                endpoint: "relay44://session/2",
                feedbackURI: "ipfs://feedback/2",
                feedbackHash: keccak256("feedback-2")
            })
        );

        AgentReputationRegistry.FeedbackView[] memory feedback = reputationRegistry.listFeedback(agentId, false, 10);
        assertEq(feedback.length, 2);
        assertEq(feedback[0].client, alice);
        assertEq(feedback[1].client, bob);
    }

    function test_preventSelfFeedback() external {
        vm.prank(owner);
        vm.expectRevert(AgentReputationRegistry.SelfOrOperatorFeedbackForbidden.selector);
        reputationRegistry.giveFeedback(
            agentId,
            AgentReputationRegistry.FeedbackInput({
                value: 100,
                valueDecimals: 0,
                category: "pnl",
                comment: "self",
                endpoint: "",
                feedbackURI: "",
                feedbackHash: bytes32(0)
            })
        );
    }

    function test_revokeFeedback() external {
        vm.prank(alice);
        reputationRegistry.giveFeedback(
            agentId,
            AgentReputationRegistry.FeedbackInput({
                value: 100,
                valueDecimals: 0,
                category: "pnl",
                comment: "msg",
                endpoint: "",
                feedbackURI: "",
                feedbackHash: bytes32(0)
            })
        );


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

        vm.prank(alice);
        reputationRegistry.revokeFeedback(agentId, 1);

        AgentReputationRegistry.FeedbackView[] memory visible = reputationRegistry.listFeedback(agentId, false, 10);
        assertEq(visible.length, 0);

        AgentReputationRegistry.FeedbackView[] memory allFeedback = reputationRegistry.listFeedback(agentId, true, 10);
        assertEq(allFeedback.length, 1);
        assertEq(allFeedback[0].revoked, true);
    }

    function test_updateMetrics() external {
        vm.prank(oracle);
        reputationRegistry.updateMetrics(agentId, 1450, 250_000e6, 150, 92, 58, 1800);

        (
            int128 roiBps,
            uint128 totalVolume,
            uint64 tradeCount,
            uint64 winCount,
            uint64 lossCount,
            uint16 maxDrawdownBps,
            uint64 updatedAt
        ) = reputationRegistry.metrics(agentId);

        assertEq(roiBps, 1450);
        assertEq(totalVolume, 250_000e6);
        assertEq(tradeCount, 150);
        assertEq(winCount, 92);
        assertEq(lossCount, 58);
        assertEq(maxDrawdownBps, 1800);
        assertGt(updatedAt, 0);
    }

    function test_onlyOracleCanUpdateMetrics() external {
        vm.prank(alice);
        vm.expectRevert();
        reputationRegistry.updateMetrics(agentId, 500, 1000, 10, 6, 4, 900);
    }
}

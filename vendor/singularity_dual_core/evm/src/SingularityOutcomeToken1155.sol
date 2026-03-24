// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import { RoleAuth } from "./shared/RoleAuth.sol";

contract SingularityOutcomeToken1155 is RoleAuth {
    error InsufficientBalance();

    event TransferSingle(
        address indexed operator,
        address indexed from,
        address indexed to,
        uint256 tokenId,
        uint256 amount
    );

    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");

    mapping(uint256 => mapping(address => uint256)) public balanceOf;

    constructor(address admin) RoleAuth(admin) {}

    function mint(address to, uint256 tokenId, uint256 amount) external onlyRole(MINTER_ROLE) {
        if (to == address(0)) revert InvalidAddress();
        if (amount == 0) return;

        balanceOf[tokenId][to] += amount;
        emit TransferSingle(msg.sender, address(0), to, tokenId, amount);
    }

    function burn(address from, uint256 tokenId, uint256 amount) external onlyRole(MINTER_ROLE) {
        if (from == address(0)) revert InvalidAddress();
        uint256 current = balanceOf[tokenId][from];
        if (current < amount) revert InsufficientBalance();
        balanceOf[tokenId][from] = current - amount;
        emit TransferSingle(msg.sender, from, address(0), tokenId, amount);
    }

    function safeTransferFrom(address from, address to, uint256 tokenId, uint256 amount) external {
        if (from != msg.sender) revert Unauthorized();
        if (to == address(0)) revert InvalidAddress();

        uint256 current = balanceOf[tokenId][from];
        if (current < amount) revert InsufficientBalance();
        balanceOf[tokenId][from] = current - amount;
        balanceOf[tokenId][to] += amount;
        emit TransferSingle(msg.sender, from, to, tokenId, amount);
    }
}

// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {AccessControl} from "openzeppelin-contracts/contracts/access/AccessControl.sol";
import {Pausable} from "openzeppelin-contracts/contracts/utils/Pausable.sol";
import {IERC20} from "openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";

contract MarketCore is AccessControl, Pausable {
    using SafeERC20 for IERC20;

    bytes32 public constant MARKET_CREATOR_ROLE = keccak256("MARKET_CREATOR_ROLE");
    bytes32 public constant RESOLVER_ROLE = keccak256("RESOLVER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    uint256 private constant MAX_TEXT_LENGTH = 2048;

    struct Market {
        bytes32 questionHash;
        uint64 closeTime;
        uint64 resolveTime;
        address resolver;
        bool resolved;
        bool outcome;
    }

    struct MarketMetadata {
        string question;
        string description;
        string category;
        string resolutionSource;
    }

    uint256 public marketCount;
    mapping(uint256 => Market) public markets;
    mapping(uint256 => address) public marketCreators;
    mapping(uint256 => MarketMetadata) private marketMetadata;

    IERC20 public relayToken;
    uint256 public creationDeposit;    // RELAY required to create a market
    address public slashRecipient;     // Where slashed deposits go
    mapping(uint256 => bool) public depositRefunded; // Track refund status

    error ZeroAddress();
    error InvalidCloseTime();
    error MarketNotFound();
    error MarketNotClosed();
    error MarketAlreadyResolved();
    error NotDesignatedResolver();
    error NotMarketCreator();
    error UnauthorizedResolver();
    error EmptyQuestion();
    error TextTooLong();
    error DepositAlreadyRefunded();
    error MarketNotResolvedYet();

    event MarketCreated(uint256 indexed marketId, bytes32 indexed questionHash, uint64 closeTime, address resolver);
    event MarketResolved(uint256 indexed marketId, bool outcome, uint64 resolveTime, address resolver);
    event MarketMetadataSet(
        uint256 indexed marketId, string question, string description, string category, string resolutionSource
    );
    event CreationDepositUpdated(uint256 deposit, address slashRecipient);
    event DepositRefunded(uint256 indexed marketId, address indexed creator, uint256 amount);
    event DepositSlashed(uint256 indexed marketId, address indexed creator, uint256 amount);

    constructor(address admin) {
        if (admin == address(0)) revert ZeroAddress();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MARKET_CREATOR_ROLE, admin);
        _grantRole(RESOLVER_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);
    }

    function setRelayToken(address token) external onlyRole(DEFAULT_ADMIN_ROLE) {
        relayToken = IERC20(token);
    }

    function setCreationDeposit(uint256 deposit, address _slashRecipient) external onlyRole(DEFAULT_ADMIN_ROLE) {
        creationDeposit = deposit;
        slashRecipient = _slashRecipient;
        emit CreationDepositUpdated(deposit, _slashRecipient);
    }

    /// @notice Refund deposit after clean resolution. Callable by market creator.
    function refundDeposit(uint256 marketId) external whenNotPaused {
        if (depositRefunded[marketId]) revert DepositAlreadyRefunded();
        Market storage market = markets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (!market.resolved) revert MarketNotResolvedYet();

        address creator = marketCreators[marketId];
        if (msg.sender != creator && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) revert NotMarketCreator();

        depositRefunded[marketId] = true;
        if (creationDeposit > 0 && address(relayToken) != address(0)) {
            relayToken.safeTransfer(creator, creationDeposit);
        }
        emit DepositRefunded(marketId, creator, creationDeposit);
    }

    /// @notice Slash deposit for invalid/spam market. Admin only.
    function slashDeposit(uint256 marketId) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (depositRefunded[marketId]) revert DepositAlreadyRefunded();
        address creator = marketCreators[marketId];
        if (creator == address(0)) revert MarketNotFound();

        depositRefunded[marketId] = true;
        if (creationDeposit > 0 && address(relayToken) != address(0) && slashRecipient != address(0)) {
            relayToken.safeTransfer(slashRecipient, creationDeposit);
        }
        emit DepositSlashed(marketId, creator, creationDeposit);
    }

    function createMarket(bytes32 questionHash, uint64 closeTime, address resolver)
        external
        whenNotPaused
        returns (uint256 marketId)
    {
        marketId = _createMarket(msg.sender, questionHash, closeTime, resolver);
    }

    function createMarketRich(
        string calldata question,
        string calldata description,
        string calldata category,
        string calldata resolutionSource,
        uint64 closeTime,
        address resolver
    ) external whenNotPaused returns (uint256 marketId) {
        if (bytes(question).length == 0) revert EmptyQuestion();

        bytes32 questionHash = keccak256(bytes(question));
        marketId = _createMarket(msg.sender, questionHash, closeTime, resolver);
        _setMarketMetadata(marketId, question, description, category, resolutionSource);
    }

    function setMarketMetadata(
        uint256 marketId,
        string calldata question,
        string calldata description,
        string calldata category,
        string calldata resolutionSource
    ) external whenNotPaused {
        if (markets[marketId].resolver == address(0)) revert MarketNotFound();
        if (msg.sender != marketCreators[marketId] && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotMarketCreator();
        }
        _setMarketMetadata(marketId, question, description, category, resolutionSource);
    }

    function getMarketMetadata(uint256 marketId)
        external
        view
        returns (
            string memory question,
            string memory description,
            string memory category,
            string memory resolutionSource
        )
    {
        if (markets[marketId].resolver == address(0)) revert MarketNotFound();
        MarketMetadata storage metadata = marketMetadata[marketId];
        return (metadata.question, metadata.description, metadata.category, metadata.resolutionSource);
    }

    function resolveMarket(uint256 marketId, bool outcome) external whenNotPaused {
        Market storage market = markets[marketId];
        if (market.resolver == address(0)) revert MarketNotFound();
        if (block.timestamp < market.closeTime) revert MarketNotClosed();
        if (market.resolved) revert MarketAlreadyResolved();
        if (msg.sender != market.resolver && !hasRole(DEFAULT_ADMIN_ROLE, msg.sender)) {
            revert NotDesignatedResolver();
        }

        market.resolved = true;
        market.outcome = outcome;
        market.resolveTime = uint64(block.timestamp);

        emit MarketResolved(marketId, outcome, market.resolveTime, msg.sender);
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function _createMarket(address creator, bytes32 questionHash, uint64 closeTime, address resolver)
        internal
        returns (uint256 marketId)
    {
        if (resolver == address(0)) revert ZeroAddress();
        if (closeTime <= block.timestamp) revert InvalidCloseTime();
        if (!hasRole(DEFAULT_ADMIN_ROLE, creator) && resolver != creator) revert UnauthorizedResolver();

        // Collect RELAY deposit if configured
        if (creationDeposit > 0 && address(relayToken) != address(0)) {
            relayToken.safeTransferFrom(creator, address(this), creationDeposit);
        }

        marketId = ++marketCount;
        marketCreators[marketId] = creator;
        markets[marketId] = Market({
            questionHash: questionHash,
            closeTime: closeTime,
            resolveTime: 0,
            resolver: resolver,
            resolved: false,
            outcome: false
        });

        emit MarketCreated(marketId, questionHash, closeTime, resolver);
    }

    function _setMarketMetadata(
        uint256 marketId,
        string calldata question,
        string calldata description,
        string calldata category,
        string calldata resolutionSource
    ) internal {
        if (bytes(question).length == 0) revert EmptyQuestion();
        if (
            bytes(question).length > MAX_TEXT_LENGTH || bytes(description).length > MAX_TEXT_LENGTH
                || bytes(category).length > MAX_TEXT_LENGTH || bytes(resolutionSource).length > MAX_TEXT_LENGTH
        ) {
            revert TextTooLong();
        }

        marketMetadata[marketId] = MarketMetadata({
            question: question, description: description, category: category, resolutionSource: resolutionSource
        });

        emit MarketMetadataSet(marketId, question, description, category, resolutionSource);
    }
}

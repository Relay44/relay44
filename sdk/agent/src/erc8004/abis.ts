export const ERC8004_IDENTITY_REGISTRY_ABI = [
  'function register(string agentURI, tuple(string key, bytes value)[] metadata) returns (uint256)',
  'function register(string agentURI) returns (uint256)',
  'function register() returns (uint256)',
  'function setAgentURI(uint256 agentId, string newURI)',
  'function tokenURI(uint256 tokenId) view returns (string)',
  'function setMetadata(uint256 agentId, string metadataKey, bytes metadataValue)',
  'function getMetadata(uint256 agentId, string metadataKey) view returns (bytes)',
  'function setAgentWallet(uint256 agentId, address newWallet, uint256 deadline, bytes signature)',
  'function getAgentWallet(uint256 agentId) view returns (address)',
  'function unsetAgentWallet(uint256 agentId)',
  'function getGlobalId(uint256 agentId) view returns (string)',
  'function totalSupply() view returns (uint256)',
  'function exists(uint256 agentId) view returns (bool)',
  'function registeredAt(uint256 agentId) view returns (uint64)',
] as const;

export const ERC8004_REPUTATION_REGISTRY_ABI = [
  'function giveFeedback(uint256 agentId, int128 value, uint8 valueDecimals, bytes32 tag1, bytes32 tag2, bytes32 endpoint, string feedbackURI, bytes32 feedbackHash)',
  'function revokeFeedback(uint256 agentId, uint64 feedbackIndex)',
  'function appendResponse(uint256 agentId, address clientAddress, uint64 feedbackIndex, string responseURI, bytes32 responseHash)',
  'function getSummary(uint256 agentId, address[] clientAddresses, bytes32 tag1, bytes32 tag2) view returns (uint64 count, int128 summaryValue, uint8 decimals)',
  'function readFeedback(uint256 agentId, address clientAddress, uint64 feedbackIndex) view returns (int128 value, uint8 valueDecimals, bytes32 tag1, bytes32 tag2, bool isRevoked)',
  'function getClients(uint256 agentId) view returns (address[])',
] as const;

export const ERC8004_VALIDATION_REGISTRY_ABI = [
  'function validationRequest(address validatorAddress, uint256 agentId, string requestURI, bytes32 requestHash)',
  'function validationResponse(bytes32 requestHash, uint8 response, string responseURI, bytes32 responseHash, bytes32 tag)',
  'function validationResponseFromTier(bytes32 requestHash, uint8 tier, string responseURI, bytes32 responseHash)',
  'function getValidationStatus(bytes32 requestHash) view returns (address validatorAddress, uint256 agentId, uint8 response, bytes32 responseHash, bytes32 tag, uint64 lastUpdate)',
  'function getSummary(uint256 agentId, address[] validatorAddresses, bytes32 tag) view returns (uint64 count, uint8 averageResponse)',
  'function getAgentValidations(uint256 agentId) view returns (bytes32[])',
  'function getValidatorRequests(address validatorAddress) view returns (bytes32[])',
  'function tierToResponse(uint8 tier) pure returns (uint8)',
  'function responseToTier(uint8 response) pure returns (uint8)',
  'function isValidator(address) view returns (bool)',
  'function getValidators() view returns (address[])',
] as const;

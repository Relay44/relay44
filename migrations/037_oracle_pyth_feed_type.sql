-- Widen feed_address to accommodate Pyth 32-byte price feed IDs (66 chars with 0x prefix).
-- Existing chainlink addresses (42 chars) are unaffected.

ALTER TABLE oracle_market_configs
    ALTER COLUMN feed_address TYPE VARCHAR(66);

COMMENT ON COLUMN oracle_market_configs.feed_type IS 'chainlink, pyth, or manual';
COMMENT ON COLUMN oracle_market_configs.feed_address IS 'Chainlink contract address (0x, 42 chars) or Pyth price feed ID (0x, 66 chars)';

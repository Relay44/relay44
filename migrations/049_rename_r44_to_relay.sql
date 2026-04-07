-- Rename r44_* tables and indexes to relay_* for $RELAY token rebrand

ALTER TABLE r44_stakes RENAME TO relay_stakes;
ALTER INDEX idx_r44_stakes_wallet RENAME TO idx_relay_stakes_wallet;
ALTER INDEX idx_r44_stakes_tier RENAME TO idx_relay_stakes_tier;

ALTER TABLE r44_reward_epochs RENAME TO relay_reward_epochs;

ALTER TABLE r44_reward_allocations RENAME TO relay_reward_allocations;
ALTER INDEX idx_r44_reward_alloc_wallet RENAME TO idx_relay_reward_alloc_wallet;
ALTER INDEX idx_r44_reward_alloc_epoch RENAME TO idx_relay_reward_alloc_epoch;
ALTER INDEX idx_r44_reward_alloc_unclaimed RENAME TO idx_relay_reward_alloc_unclaimed;

ALTER TABLE r44_execution_burns RENAME TO relay_execution_burns;
ALTER INDEX idx_r44_burns_wallet RENAME TO idx_relay_burns_wallet;
ALTER INDEX idx_r44_burns_agent RENAME TO idx_relay_burns_agent;

ALTER TABLE r44_market_deposits RENAME TO relay_market_deposits;
ALTER INDEX idx_r44_deposits_wallet RENAME TO idx_relay_deposits_wallet;
ALTER INDEX idx_r44_deposits_status RENAME TO idx_relay_deposits_status;

ALTER TABLE r44_fee_collections RENAME COLUMN r44_tier TO relay_tier;
ALTER TABLE r44_fee_collections RENAME TO relay_fee_collections;
ALTER INDEX idx_r44_fees_market RENAME TO idx_relay_fees_market;
ALTER INDEX idx_r44_fees_wallet RENAME TO idx_relay_fees_wallet;

ALTER TABLE r44_fee_withdrawals RENAME TO relay_fee_withdrawals;

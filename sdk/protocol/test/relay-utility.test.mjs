import test from 'node:test';
import assert from 'node:assert/strict';

const protocol = await import('../dist/index.js');

test('RELAY_TIERS exposes Bronze/Silver/Gold/Diamond in order', () => {
  assert.equal(protocol.RELAY_TIERS.length, 4);
  assert.equal(protocol.RELAY_TIERS[0].name, 'Bronze');
  assert.equal(protocol.RELAY_TIERS[1].name, 'Silver');
  assert.equal(protocol.RELAY_TIERS[2].name, 'Gold');
  assert.equal(protocol.RELAY_TIERS[3].name, 'Diamond');
});

test('Tier thresholds match Solidity OrderBook constants', () => {
  const wei = (n) => BigInt(n) * 10n ** 18n;
  assert.equal(protocol.RELAY_TIERS[0].minRelayWei, 0n);
  assert.equal(protocol.RELAY_TIERS[1].minRelayWei, wei(1_000));
  assert.equal(protocol.RELAY_TIERS[2].minRelayWei, wei(10_000));
  assert.equal(protocol.RELAY_TIERS[3].minRelayWei, wei(100_000));
});

test('Fee discount bps match Solidity OrderBook getDiscountBps', () => {
  assert.equal(protocol.RELAY_TIERS[0].feeDiscountBps, 0);
  assert.equal(protocol.RELAY_TIERS[1].feeDiscountBps, 2_500);
  assert.equal(protocol.RELAY_TIERS[2].feeDiscountBps, 5_000);
  assert.equal(protocol.RELAY_TIERS[3].feeDiscountBps, 7_500);
});

test('x402 bypass kicks in at tier 2+', () => {
  assert.equal(protocol.X402_BYPASS_TIER, 2);
  assert.equal(protocol.RELAY_TIERS[0].x402Bypass, false);
  assert.equal(protocol.RELAY_TIERS[1].x402Bypass, false);
  assert.equal(protocol.RELAY_TIERS[2].x402Bypass, true);
  assert.equal(protocol.RELAY_TIERS[3].x402Bypass, true);
});

test('relayTierFromStakedWei resolves correct tier for boundary values', () => {
  const wei = (n) => BigInt(n) * 10n ** 18n;
  assert.equal(protocol.relayTierFromStakedWei(0n).tier, 0);
  assert.equal(protocol.relayTierFromStakedWei(wei(999)).tier, 0);
  assert.equal(protocol.relayTierFromStakedWei(wei(1_000)).tier, 1);
  assert.equal(protocol.relayTierFromStakedWei(wei(9_999)).tier, 1);
  assert.equal(protocol.relayTierFromStakedWei(wei(10_000)).tier, 2);
  assert.equal(protocol.relayTierFromStakedWei(wei(99_999)).tier, 2);
  assert.equal(protocol.relayTierFromStakedWei(wei(100_000)).tier, 3);
  assert.equal(protocol.relayTierFromStakedWei(wei(1_000_000_000)).tier, 3);
});

test('relayTierById returns metadata for known tiers and throws otherwise', () => {
  for (let i = 0; i < 4; i++) {
    assert.equal(protocol.relayTierById(i).tier, i);
  }
  assert.throws(() => protocol.relayTierById(99), /Unknown RELAY tier/);
});

test('getRelayUtilityAddresses returns the production addresses from the manifest', () => {
  const addrs = protocol.getRelayUtilityAddresses('production');
  assert.equal(addrs.token.toLowerCase(), '0x580ff5ae64ec792a949c6123386a8a936c7ebb07');
  assert.equal(addrs.staking.toLowerCase(), '0x709d6006f026950b531d4883260c8416650c5ab7');
  assert.equal(
    addrs.rewardDistributor.toLowerCase(),
    '0x3c4c0a74f9d108f966908a835a9b4b8d946bbce3',
  );
});

test('getRelayChainId returns 8453 for production and 84532 for staging', () => {
  assert.equal(protocol.getRelayChainId('production'), 8453);
  assert.equal(protocol.getRelayChainId('staging'), 84532);
});

test('RELAY_DECIMALS is 18', () => {
  assert.equal(protocol.RELAY_DECIMALS, 18);
});

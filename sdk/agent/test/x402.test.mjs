import test from 'node:test';
import assert from 'node:assert/strict';

const sdk = await import('../dist/index.js');

test('qualifyX402ByTier resolves all four tiers', () => {
  const t0 = sdk.qualifyX402ByTier(0);
  assert.equal(t0.tier.tier, 0);
  assert.equal(t0.bypassesX402, false);
  assert.equal(t0.x402DiscountBps, 0);

  const t1 = sdk.qualifyX402ByTier(1);
  assert.equal(t1.tier.tier, 1);
  assert.equal(t1.bypassesX402, false);
  assert.equal(t1.x402DiscountBps, 2_500);

  const t2 = sdk.qualifyX402ByTier(2);
  assert.equal(t2.tier.tier, 2);
  assert.equal(t2.bypassesX402, true);
  assert.equal(t2.x402DiscountBps, 10_000);

  const t3 = sdk.qualifyX402ByTier(3);
  assert.equal(t3.tier.tier, 3);
  assert.equal(t3.bypassesX402, true);
  assert.equal(t3.x402DiscountBps, 10_000);
});

test('qualifyX402FromStaked snaps wallets at threshold boundaries', () => {
  const wei = (n) => BigInt(n) * 10n ** 18n;
  assert.equal(sdk.qualifyX402FromStaked(0n).tier.name, 'Bronze');
  assert.equal(sdk.qualifyX402FromStaked(wei(999)).tier.name, 'Bronze');
  assert.equal(sdk.qualifyX402FromStaked(wei(1_000)).tier.name, 'Silver');
  assert.equal(sdk.qualifyX402FromStaked(wei(10_000)).tier.name, 'Gold');
  assert.equal(sdk.qualifyX402FromStaked(wei(100_000)).tier.name, 'Diamond');
});

test('priceForX402Tier mirrors the Rust discounted_amount semantics', () => {
  const base = 10_000n; // micro-USDC
  const qBronze = sdk.qualifyX402ByTier(0);
  const qSilver = sdk.qualifyX402ByTier(1);
  const qGold = sdk.qualifyX402ByTier(2);
  const qDiamond = sdk.qualifyX402ByTier(3);

  assert.equal(sdk.priceForX402Tier(base, qBronze).effectiveMicroUsdc, 10_000n);
  assert.equal(sdk.priceForX402Tier(base, qSilver).effectiveMicroUsdc, 7_500n);
  assert.equal(sdk.priceForX402Tier(base, qGold).effectiveMicroUsdc, 0n);
  assert.equal(sdk.priceForX402Tier(base, qDiamond).effectiveMicroUsdc, 0n);
});

test('qualifyX402OnChain calls staking.getTier with the manifest address', async () => {
  let captured;
  const stub = {
    readContract: async (args) => {
      captured = args;
      return 2n;
    },
  };
  const result = await sdk.qualifyX402OnChain({
    client: stub,
    network: 'production',
    wallet: '0x1111111111111111111111111111111111111111',
  });
  assert.equal(
    captured.address.toLowerCase(),
    '0x709d6006f026950b531d4883260c8416650c5ab7',
  );
  assert.equal(captured.functionName, 'getTier');
  assert.equal(captured.args[0], '0x1111111111111111111111111111111111111111');
  assert.equal(result.tier.tier, 2);
  assert.equal(result.bypassesX402, true);
});

test('qualifyX402OnChain accepts both bigint and number tier responses', async () => {
  const numStub = {
    readContract: async () => 1,
  };
  const numQual = await sdk.qualifyX402OnChain({
    client: numStub,
    network: 'production',
    wallet: '0x1111111111111111111111111111111111111111',
  });
  assert.equal(numQual.tier.tier, 1);
});

test('Bronze tier (0) charges full price for x402', () => {
  const q = sdk.qualifyX402FromStaked(0n);
  const breakdown = sdk.priceForX402Tier(2_500n, q);
  assert.equal(breakdown.baseMicroUsdc, 2_500n);
  assert.equal(breakdown.effectiveMicroUsdc, 2_500n);
  assert.equal(breakdown.qualification.bypassesX402, false);
});

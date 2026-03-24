import { expect, test } from '@playwright/test';

const apiUrl = process.env.E2E_API_URL?.replace(//+$/, '');

test.describe('Base Sepolia smoke @base-sepolia', () => {
  test.skip(!apiUrl, 'E2E_API_URL must be set for Base Sepolia smoke tests');

  test('web pages render in Base mode', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/relay44/i);
    await expect(page.getByRole('link', { name: /relay44/i })).toBeVisible();

    await page.goto('/settings');
    await expect(page.getByRole('heading', { name: 'Network' })).toBeVisible();
    await expect(page.getByText('Base', { exact: true })).toBeVisible();
    await expect(page.getByText(/RPC:/i)).toBeVisible();

    await page.goto('/markets');
    await expect(page.getByRole('heading', { name: /all markets/i })).toBeVisible();
  });

  test('api health and base components are healthy', async ({ request }) => {
    const healthRes = await request.get(`${apiUrl}/health`);
    expect(healthRes.status()).toBe(200);
    const healthJson = await healthRes.json();
    expect(healthJson.status).toBe('healthy');

    const detailedRes = await request.get(`${apiUrl}/health/detailed`);
    expect(detailedRes.status()).toBe(200);
    const detailedJson = await detailedRes.json();
    const checks = detailedJson.checks || detailedJson.components || {};
    expect(checks.base?.status).toBe('healthy');
  });

  test('siwe nonce and evm market endpoints respond', async ({ request }) => {
    const nonceRes = await request.get(`${apiUrl}/v1/auth/siwe/nonce`);
    expect(nonceRes.status()).toBe(200);
    const nonceJson = await nonceRes.json();
    expect(typeof nonceJson.nonce).toBe('string');
    expect(nonceJson.nonce.length).toBeGreaterThanOrEqual(8);

    const marketsRes = await request.get(`${apiUrl}/v1/evm/markets?limit=1`);
    expect(marketsRes.status()).toBe(200);
    const marketsJson = await marketsRes.json();
    expect(Array.isArray(marketsJson.markets)).toBeTruthy();
    expect(marketsJson.markets.length).toBeGreaterThan(0);

    const marketId = String(marketsJson.markets[0].id);

    const orderbookRes = await request.get(
      `${apiUrl}/v1/evm/markets/${marketId}/orderbook?outcome=yes&depth=5`
    );
    expect(orderbookRes.status()).toBe(200);
    const orderbookJson = await orderbookRes.json();
    expect(Array.isArray(orderbookJson.bids)).toBeTruthy();
    expect(Array.isArray(orderbookJson.asks)).toBeTruthy();

    const tradesRes = await request.get(
      `${apiUrl}/v1/evm/markets/${marketId}/trades?limit=5`
    );
    expect(tradesRes.status()).toBe(200);
    const tradesJson = await tradesRes.json();
    expect(Array.isArray(tradesJson.trades)).toBeTruthy();
  });
});

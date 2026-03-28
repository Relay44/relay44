import { test, expect } from '@playwright/test';

test.describe('Wallet Connection UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('connect wallet button is visible', async ({ page }) => {
    const connectBtn = page.getByRole('button', { name: /connect wallet/i });
    await expect(connectBtn).toBeVisible();
  });

  test('connect wallet button uses the shared header action style', async ({ page }) => {
    const connectBtn = page.getByRole('button', { name: /connect wallet/i });
    await expect(connectBtn).toHaveClass(/border-border/);
  });

  test('connect wallet button is clickable', async ({ page }) => {
    const connectBtn = page.getByRole('button', { name: /connect wallet/i });
    await expect(connectBtn).toBeEnabled();
  });
});

test.describe('Wallet Required Pages', () => {
  test('portfolio page accessible without wallet', async ({ page }) => {
    await page.goto('/portfolio');
    await expect(page).toHaveURL('/portfolio');
  });

  test('market detail shows connect prompt when not connected', async ({ page }) => {
    await page.goto('/markets/polymarket%3A540816');
    await expect(
      page.getByText(/trading is currently unavailable|connect wallet to trade/i),
    ).toBeVisible();
  });
});

import { test, expect } from '@playwright/test';

test.describe('Settings Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/settings');
  });

  test('displays settings heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  });

  test('displays preferences section', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Preferences' })).toBeVisible();
    await expect(page.getByText('Dark Mode')).toBeVisible();
    await expect(page.getByText('Push Notifications')).toBeVisible();
  });

  test('displays network section', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Network' })).toBeVisible();
    await expect(page.getByText('Base', { exact: true })).toBeVisible();
    await expect(page.getByText('RPC: https://mainnet.base.org')).toBeVisible();
  });

  test('displays about section', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'About' })).toBeVisible();
    await expect(page.getByText('Version')).toBeVisible();
    await expect(page.getByText('Build')).toBeVisible();
  });

  test('does not display wallet section when not connected', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Wallet' })).not.toBeVisible();
  });
});

test.describe('Credential Settings', () => {
  test('credentials route shows live credential setup guidance and form controls', async ({
    page,
  }) => {
    await page.goto('/settings/credentials', { waitUntil: 'domcontentloaded' });

    await expect(page.getByRole('heading', { name: 'External Credentials' })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: /connect and authenticate/i }),
    ).toBeVisible();
    await expect(
      page.getByText(/connect your wallet, then authenticate with siwe before saving keys/i),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Save credential' })).toBeVisible();
    expect(
      await page
        .getByRole('heading', { name: /credential management is currently unavailable/i })
        .count(),
    ).toBe(0);
  });
});

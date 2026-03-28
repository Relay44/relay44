import { test, expect } from '@playwright/test';

const TEST_WALLET = '0x71C7656EC7ab88b098defB751B7401B5f6d8976F';

test.describe('Profile Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`/profile/${TEST_WALLET}`);
  });

  test('page loads successfully', async ({ page }) => {
    await expect(page).toHaveURL(`/profile/${TEST_WALLET}`);
  });

  test('has correct page title with truncated address', async ({ page }) => {
    await expect(page).toHaveTitle(/0x71C7.*976F.*relay44/i);
  });

  test('displays container with proper layout', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /public profiles are not live yet/i })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Browse markets' })).toBeVisible();
  });
});

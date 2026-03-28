import { test, expect } from '@playwright/test';

test.describe('Leaderboard Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/leaderboard');
  });

  test('page loads successfully', async ({ page }) => {
    await expect(page).toHaveURL('/leaderboard');
  });

  test('has correct page title in metadata', async ({ page }) => {
    await expect(page).toHaveTitle(/leaderboard.*relay44/i);
  });

  test('displays container with proper layout', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Leaderboard' })).toBeVisible();
    await expect(
      page
        .getByText(/loading leaderboard|leaderboard is not live yet|no data available for this period/i)
        .first()
    ).toBeVisible();
  });
});

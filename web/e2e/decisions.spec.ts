import { expect, test } from '@playwright/test';

test.describe('Decisions Page', () => {
  test('shows the disconnected access gate', async ({ page }) => {
    await page.goto('/decisions', { waitUntil: 'domcontentloaded' });

    const main = page.getByRole('main');
    await expect(page.getByRole('heading', { name: /connect your wallet/i })).toBeVisible();
    await expect(
      page.getByText(/authenticate before creating, editing, or automating a decision cell/i)
    ).toBeVisible();
    await expect(main.getByRole('link', { name: 'Browse markets' })).toBeVisible();
    await expect(main.getByRole('link', { name: 'Portfolio' })).toBeVisible();
  });

  test('create route uses the same access gate', async ({ page }) => {
    await page.goto('/decisions/create', { waitUntil: 'domcontentloaded' });

    await expect(page.getByRole('heading', { name: /connect your wallet/i })).toBeVisible();
    await expect(
      page.getByText(/authenticate before creating, editing, or automating a decision cell/i)
    ).toBeVisible();
  });
});

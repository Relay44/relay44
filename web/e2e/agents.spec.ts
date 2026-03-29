import { expect, test } from '@playwright/test';

test.describe('Agents Page', () => {
  test('loads without console errors and shows directory state', async ({ page }) => {
    const consoleErrors: string[] = [];

    page.on('console', (message) => {
      if (message.type() === 'error') {
        consoleErrors.push(message.text());
      }
    });
    page.on('pageerror', (error) => {
      consoleErrors.push(error.message);
    });

    await page.goto('/agents', { waitUntil: 'domcontentloaded' });

    await expect(page.getByRole('heading', { name: 'Agents' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Launch Agent' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Agent Directory' })).toBeVisible();
    await expect(page.getByText(/No agents found for current filter\./i)).toBeVisible();

    expect(consoleErrors).toEqual([]);
  });

  test('links to credentials management', async ({ page }) => {
    await page.goto('/agents', { waitUntil: 'domcontentloaded' });

    await expect(
      page.getByRole('link', { name: /manage venue credentials/i })
    ).toBeVisible();
  });

  test('exposes launch controls when runtime is live', async ({ page }) => {
    await page.goto('/agents', { waitUntil: 'domcontentloaded' });

    await expect(page.getByRole('heading', { name: 'Launch Agent' })).toBeVisible();
    await expect(page.getByRole('button', { name: /launch onchain agent/i })).toBeVisible();
    expect(
      await page.getByRole('heading', { name: /agent control is currently unavailable/i }).count(),
    ).toBe(0);
  });
});

import { test, expect } from "@playwright/test";

test.describe("Leaderboard Page", () => {
  test.beforeEach(async ({ page }) => {
    const response = await page.goto("/leaderboard");
    expect(response?.status()).toBe(404);
  });

  test("returns 404", async ({ page }) => {
    await expect(page).toHaveURL("/leaderboard");
  });

  test("shows the not found page", async ({ page }) => {
    await expect(page.getByText(/this page could not be found/i)).toBeVisible();
  });
});

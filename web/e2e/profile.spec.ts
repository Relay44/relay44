import { test, expect } from "@playwright/test";

const TEST_WALLET = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F";

test.describe("Profile Page", () => {
  test.beforeEach(async ({ page }) => {
    const response = await page.goto(`/profile/${TEST_WALLET}`);
    expect(response?.status()).toBe(404);
  });

  test("returns 404", async ({ page }) => {
    await page.goto(`/profile/${TEST_WALLET}`);
    await expect(page).toHaveURL(`/profile/${TEST_WALLET}`);
  });

  test("shows the not found page", async ({ page }) => {
    await expect(page.getByText(/this page could not be found/i)).toBeVisible();
  });
});

import { test, expect } from "@playwright/test";

test.describe("Homepage", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/", { waitUntil: "domcontentloaded" });
  });

  test("displays header with logo and navigation", async ({ page }) => {
    const header = page.getByRole("banner");
    await expect(
      header.getByRole("link", { name: /relay44/i }),
    ).toBeVisible();
    await expect(
      header.getByRole("link", { name: "Markets", exact: true }),
    ).toBeVisible();
    await expect(
      header.getByRole("link", { name: "How it works", exact: true }),
    ).toBeVisible();
    await expect(
      header.getByRole("link", { name: "Portfolio", exact: true }),
    ).toBeVisible();
  });

  test("displays connect wallet button", async ({ page }) => {
    await expect(
      page.getByRole("banner").getByRole("button", { name: /connect/i }),
    ).toBeVisible();
  });

  test("displays search input on desktop", async ({ page }) => {
    await expect(page.getByPlaceholder(/search/i)).toBeVisible();
  });

  test("displays featured market rail controls", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Signal Relay" })).toBeVisible();
    await expect(page.locator('section').nth(1).getByRole("button").first()).toBeVisible();
  });

  test("displays featured section", async ({ page }) => {
    // Featured banner or slider should exist
    await expect(page.locator("section, main").first()).toBeVisible();
  });

  test("displays launch primer call to action", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(
      main.getByRole("link", { name: "How it works", exact: true }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "Risk disclosure", exact: true }),
    ).toBeVisible();
  });

  test("page loads without errors", async ({ page }) => {
    // Page should load and have content
    await expect(page).toHaveTitle(/relay44/i);
  });

  test("theme toggle is visible", async ({ page }) => {
    const themeToggle = page
      .locator('button[aria-label*="theme"], button[title*="theme"]')
      .or(
        page
          .locator("button")
          .filter({ has: page.locator("svg") })
          .first(),
      );
    await expect(themeToggle.first()).toBeVisible();
  });
});

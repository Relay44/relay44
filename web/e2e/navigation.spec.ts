import { test, expect } from "@playwright/test";

test.describe("Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
  });

  test("navigates to markets page via header link", async ({ page }) => {
    await page.goto("/");
    await page
      .getByRole("banner")
      .getByRole("link", { name: "Markets", exact: true })
      .click();
    await expect(page).toHaveURL("/markets");
    await expect(
      page.getByRole("heading", { name: /all markets/i }),
    ).toBeVisible();
  });

  test("navigates to portfolio page via header link", async ({ page }) => {
    await page.goto("/");
    await page
      .getByRole("banner")
      .getByRole("link", { name: "Portfolio", exact: true })
      .click();
    await expect(page).toHaveURL("/portfolio");
  });

  test("navigates to how it works page via header link", async ({ page }) => {
    await page.goto("/");
    await page
      .getByRole("banner")
      .getByRole("link", { name: "How it works", exact: true })
      .click();
    await expect(page).toHaveURL("/how-it-works");
    await expect(
      page.getByRole("heading", { name: /how relay44 works/i }),
    ).toBeVisible();
  });

  test("navigates home via logo click", async ({ page }) => {
    await page.goto("/markets");
    await page
      .getByRole("banner")
      .getByRole("link", { name: /relay44/i })
      .click();
    await expect(page).toHaveURL("/");
  });

  test("navigates to leaderboard page", async ({ page }) => {
    await page.goto("/leaderboard");
    await expect(page).toHaveURL("/leaderboard");
  });

  test("navigates to settings page", async ({ page }) => {
    await page.goto("/settings");
    await expect(page).toHaveURL("/settings");
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  test("navigates to profile page with wallet address", async ({ page }) => {
    const testWallet = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F";
    await page.goto(`/profile/${testWallet}`);
    await expect(page).toHaveURL(`/profile/${testWallet}`);
  });
});

test.describe("Header Active States", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
  });

  test("markets link shows active state on markets page", async ({ page }) => {
    await page.goto("/markets");
    const marketsLink = page
      .getByRole("banner")
      .getByRole("link", { name: "Markets", exact: true });
    await expect(marketsLink).toHaveClass(/bg-bg-secondary/);
  });

  test("portfolio link shows active state on portfolio page", async ({
    page,
  }) => {
    await page.goto("/portfolio");
    const portfolioLink = page
      .getByRole("banner")
      .getByRole("link", { name: "Portfolio", exact: true });
    await expect(portfolioLink).toHaveClass(/bg-bg-secondary/);
  });

  test("how it works link shows active state on how it works page", async ({
    page,
  }) => {
    await page.goto("/how-it-works");
    const howItWorksLink = page
      .getByRole("banner")
      .getByRole("link", { name: "How it works", exact: true });
    await expect(howItWorksLink).toHaveClass(/bg-bg-secondary/);
  });
});

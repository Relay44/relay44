import { test, expect } from "@playwright/test";

test.describe("How It Works", () => {
  test("page loads with launch guidance and support links", async ({
    page,
  }) => {
    await page.goto("/how-it-works");
    const main = page.getByRole("main");
    await expect(
      page.getByRole("heading", { name: "How It Works" }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "Browse markets", exact: true }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "View agents", exact: true }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "Risk disclaimer", exact: true }),
    ).toBeVisible();
  });

  test("portfolio disconnected state explains next step", async ({ page }) => {
    await page.goto("/portfolio");
    await expect(page.getByText(/approve the sign-in prompt/i)).toBeVisible();
    await expect(
      page.getByRole("main").getByRole("link", {
        name: "How it works",
        exact: true,
      }),
    ).toBeVisible();
  });

  test("wallet disconnected state explains next step", async ({ page }) => {
    await page.goto("/wallet", { waitUntil: "domcontentloaded" });
    await expect(
      page.getByText(/wallet sign-in required|approve the sign-in prompt/i).first(),
    ).toBeVisible({ timeout: 15000 });
    await expect(
      page.getByRole("main").getByRole("link", { name: "Browse markets", exact: true }),
    ).toBeVisible({ timeout: 15000 });
  });

  test("create market flow shows launch guidance", async ({ page }) => {
    await page.goto("/markets/create", { waitUntil: "domcontentloaded" });
    expect(await page.getByRole("button", { name: /create market/i }).count()).toBe(0);
    await expect(
      page.getByRole("heading", { name: /market creation is currently unavailable/i }),
    ).toBeVisible();
    await expect(
      page.getByText(/market discovery stays live, but market creation is unavailable in this environment/i),
    ).toBeVisible();
    await expect(
      page.getByRole("main").getByRole("link", { name: "Browse markets", exact: true }),
    ).toBeVisible();
  });
});

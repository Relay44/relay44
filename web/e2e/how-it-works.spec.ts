import { test, expect } from "@playwright/test";

test.describe("How It Works", () => {
  test("page loads with launch guidance and support links", async ({
    page,
  }) => {
    await page.goto("/how-it-works");
    const main = page.getByRole("main");
    await expect(
      page.getByRole("heading", { name: /how relay44 works/i }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "Browse markets", exact: true }),
    ).toBeVisible();
    await expect(
      main.getByRole("link", { name: "Create a market", exact: true }),
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
    await page.goto("/wallet");
    await expect(page.getByText(/approve the sign-in prompt/i)).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Browse markets" }),
    ).toBeVisible();
  });

  test("create market flow shows launch guidance", async ({ page }) => {
    await page.goto("/markets/create");
    await expect(page.getByText(/question checklist/i)).toBeVisible();
    await expect(page.getByText(/objective yes or no outcome/i)).toBeVisible();
  });
});

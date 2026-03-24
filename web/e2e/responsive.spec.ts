import { expect, test } from "@playwright/test";

test.describe("Responsive launch surfaces", () => {
  test("homepage keeps launch guidance reachable on mobile", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await expect(
      page.getByRole("button", { name: "Open navigation menu" }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "How it works" }).first(),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Risk disclosure" }),
    ).toBeVisible();
  });
});

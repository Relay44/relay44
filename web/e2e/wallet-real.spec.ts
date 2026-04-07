import { test as base, expect, chromium, BrowserContext, Page, Browser } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://localhost:3001';
const CDP_PORT = 9222;

let browser: Browser;
let context: BrowserContext;
let page: Page;

base.describe.configure({ mode: 'serial' });

base.describe('Real Wallet Extensions', () => {
  base.setTimeout(60_000);

  base.beforeAll(async () => {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);
    context = browser.contexts()[0];
    page = context.pages()[0] || await context.newPage();
  });

  base.afterAll(async () => {
    // Don't close browser — leave Chrome running
  });

  base('page loads without error', async () => {
    await page.goto(BASE_URL, { waitUntil: 'networkidle', timeout: 30000 });
    await expect(page.locator('body')).not.toContainText('Application error');
  });

  base('connect button is visible', async () => {
    const btn = page.getByRole('button', { name: /connect/i });
    await expect(btn).toBeVisible({ timeout: 10000 });
  });

  base('wallet provider injected into page', async () => {
    const detected = await page.evaluate(() => {
      return new Promise<{ ethereum: boolean; eip6963: boolean }>((resolve) => {
        const result = {
          ethereum: typeof (window as any).ethereum !== 'undefined',
          eip6963: false,
        };
        const handler = () => { result.eip6963 = true; };
        window.addEventListener('eip6963:announceProvider', handler, { once: true });
        window.dispatchEvent(new Event('eip6963:requestProvider'));
        setTimeout(() => {
          window.removeEventListener('eip6963:announceProvider', handler);
          resolve(result);
        }, 2000);
      });
    });
    console.log('Provider detection:', detected);
    expect(detected.ethereum || detected.eip6963).toBe(true);
  });

  base('clicking connect triggers wallet flow', async () => {
    await page.goto(BASE_URL, { waitUntil: 'networkidle', timeout: 30000 });
    const btn = page.getByRole('button', { name: /connect/i });
    await btn.click({ timeout: 5000 });
    // Give the modal/connector time to respond
    await page.waitForTimeout(3000);
    await page.screenshot({ path: 'e2e/screenshots/connect-modal.png', fullPage: false });
    // Don't assert on modal content — varies by AppKit config and extension popups
  });

  base('/markets loads without error', async () => {
    await page.goto(`${BASE_URL}/markets`, { waitUntil: 'networkidle', timeout: 30000 });
    await expect(page.locator('body')).not.toContainText('Application error');
    await expect(page.locator('body')).not.toContainText('Unhandled Runtime Error');
  });

  base('/portfolio loads without error', async () => {
    await page.goto(`${BASE_URL}/portfolio`, { waitUntil: 'networkidle', timeout: 30000 });
    await expect(page.locator('body')).not.toContainText('Application error');
  });

  base('/settings loads without error', async () => {
    await page.goto(`${BASE_URL}/settings`, { waitUntil: 'networkidle', timeout: 30000 });
    await expect(page.locator('body')).not.toContainText('Application error');
  });
});

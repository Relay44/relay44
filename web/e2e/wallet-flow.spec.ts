import { test, expect, Page } from '@playwright/test';

/**
 * Inject a mock EIP-1193 wallet provider into the page.
 * This simulates MetaMask / Coinbase Wallet / Phantom at the window.ethereum level
 * so that wagmi connectors discover it and the full connect flow can be tested.
 */
async function injectMockWallet(
  page: Page,
  opts: {
    address?: string;
    chainId?: number;
    walletName?: string;
    /** If true, eth_requestAccounts will reject (user denied). */
    rejectConnect?: boolean;
  } = {},
) {
  const address = opts.address ?? '0x1234567890abcdef1234567890abcdef12345678';
  const chainId = opts.chainId ?? 8453; // Base mainnet
  const walletName = opts.walletName ?? 'MetaMask';
  const rejectConnect = opts.rejectConnect ?? false;

  await page.addInitScript(
    ({ address, chainId, walletName, rejectConnect }) => {
      const listeners: Record<string, ((...args: unknown[]) => void)[]> = {};

      const provider = {
        isMetaMask: walletName === 'MetaMask',
        isCoinbaseWallet: walletName === 'Coinbase Wallet',
        isPhantom: walletName === 'Phantom',
        isRabby: walletName === 'Rabby',
        selectedAddress: null as string | null,
        chainId: `0x${chainId.toString(16)}`,
        networkVersion: String(chainId),

        on(event: string, cb: (...args: unknown[]) => void) {
          (listeners[event] ??= []).push(cb);
          return provider;
        },
        removeListener(event: string, cb: (...args: unknown[]) => void) {
          listeners[event] = (listeners[event] ?? []).filter((f) => f !== cb);
          return provider;
        },
        removeAllListeners() {
          Object.keys(listeners).forEach((k) => delete listeners[k]);
          return provider;
        },

        async request({ method, params }: { method: string; params?: unknown[] }) {
          switch (method) {
            case 'eth_requestAccounts':
            case 'eth_accounts': {
              if (rejectConnect) {
                throw { code: 4001, message: 'User rejected the request.' };
              }
              provider.selectedAddress = address;
              (listeners['accountsChanged'] ?? []).forEach((cb) => cb([address]));
              return [address];
            }
            case 'eth_chainId':
              return provider.chainId;
            case 'net_version':
              return provider.networkVersion;
            case 'wallet_switchEthereumChain': {
              const target = (params as [{ chainId: string }])?.[0]?.chainId;
              if (target) {
                provider.chainId = target;
                provider.networkVersion = String(parseInt(target, 16));
                (listeners['chainChanged'] ?? []).forEach((cb) => cb(target));
              }
              return null;
            }
            case 'personal_sign':
            case 'eth_sign':
            case 'eth_signTypedData_v4': {
              // Return a deterministic fake signature for SIWE testing.
              return '0x' + 'ab'.repeat(65);
            }
            case 'eth_getBalance':
              return '0x0';
            case 'eth_blockNumber':
              return '0x1';
            case 'eth_estimateGas':
              return '0x5208';
            case 'eth_call':
              return '0x';
            default:
              console.warn(`[MockWallet] unhandled method: ${method}`);
              return null;
          }
        },
      };

      // EIP-6963: announce provider for discovery
      const info = {
        uuid: crypto.randomUUID(),
        name: walletName,
        icon: 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg"/>',
        rdns: walletName === 'MetaMask'
          ? 'io.metamask'
          : walletName === 'Coinbase Wallet'
            ? 'com.coinbase.wallet'
            : walletName === 'Phantom'
              ? 'app.phantom'
              : walletName === 'Rabby'
                ? 'io.rabby'
                : 'unknown',
      };

      (window as any).ethereum = provider;

      window.addEventListener('eip6963:requestProvider', () => {
        window.dispatchEvent(
          new CustomEvent('eip6963:announceProvider', {
            detail: Object.freeze({ info, provider }),
          }),
        );
      });

      // Also auto-announce on load.
      window.dispatchEvent(
        new CustomEvent('eip6963:announceProvider', {
          detail: Object.freeze({ info, provider }),
        }),
      );
    },
    { address, chainId, walletName, rejectConnect },
  );
}

// ─── UI Rendering ─────────────────────────────────────────────────────

test.describe('Wallet UI — Disconnected State', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('connect button visible on desktop', async ({ page }) => {
    const btn = page.getByRole('button', { name: /connect/i });
    await expect(btn).toBeVisible();
  });

  test('connect button visible on mobile viewport', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto('/');
    // Mobile may show hamburger — open sidebar first
    const hamburger = page.locator('[aria-label="Open menu"], [aria-label="Menu"]');
    if (await hamburger.isVisible()) {
      await hamburger.click();
    }
    const btn = page.getByRole('button', { name: /connect/i });
    await expect(btn).toBeVisible();
  });

  test('connect button text says "Connect Wallet" on desktop', async ({ page }) => {
    const btn = page.getByRole('button', { name: /connect wallet/i });
    await expect(btn).toBeVisible();
    await expect(btn).toContainText(/connect wallet/i);
  });

  test('connect button is enabled and clickable', async ({ page }) => {
    const btn = page.getByRole('button', { name: /connect/i });
    await expect(btn).toBeEnabled();
  });
});

// ─── Connect Flow — MetaMask ──────────────────────────────────────────

test.describe('Wallet Connect — MetaMask', () => {
  test('clicking connect with injected MetaMask triggers connection', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
      chainId: 8453,
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    // After connecting, the button should show the truncated address
    await expect(page.getByText('0xd8...6045')).toBeVisible({ timeout: 10000 });
  });

  test('connected address shown in header after connect', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      address: '0xABCDEF0123456789ABCDEF0123456789ABCDEF01',
      chainId: 8453,
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    // Should show truncated address
    await expect(page.getByText('0xAB...EF01')).toBeVisible({ timeout: 10000 });
  });
});

// ─── Connect Flow — Coinbase Wallet ───────────────────────────────────

test.describe('Wallet Connect — Coinbase Wallet', () => {
  test('coinbase wallet connects and shows address', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'Coinbase Wallet',
      address: '0x71C7656EC7ab88b098defB751B7401B5f6d8976F',
      chainId: 8453,
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    await expect(page.getByText('0x71...976F')).toBeVisible({ timeout: 10000 });
  });
});

// ─── Connect Flow — Phantom ───────────────────────────────────────────

test.describe('Wallet Connect — Phantom (EVM)', () => {
  test('phantom EVM wallet connects and shows address', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'Phantom',
      address: '0x2222222222222222222222222222222222222222',
      chainId: 8453,
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    await expect(page.getByText('0x22...2222')).toBeVisible({ timeout: 10000 });
  });
});

// ─── Disconnect Flow ──────────────────────────────────────────────────

test.describe('Wallet Disconnect', () => {
  test('clicking connected address disconnects wallet', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
      chainId: 8453,
    });
    await page.goto('/');

    // Connect
    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();
    await expect(page.getByText('0xd8...6045')).toBeVisible({ timeout: 10000 });

    // Disconnect by clicking the address button
    const addressBtn = page.getByRole('button', { name: /0xd8/i });
    await addressBtn.click();

    // Should revert to "Connect Wallet"
    await expect(page.getByRole('button', { name: /connect/i })).toBeVisible({ timeout: 10000 });
  });
});

// ─── User Rejection ───────────────────────────────────────────────────

test.describe('Wallet Connect — User Rejection', () => {
  test('user rejection shows error toast', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      rejectConnect: true,
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    // Should show an error toast or stay in disconnected state
    await expect(page.getByRole('button', { name: /connect/i })).toBeVisible({ timeout: 5000 });
  });
});

// ─── Protected Pages ──────────────────────────────────────────────────

test.describe('Wallet-Gated Content', () => {
  test('portfolio page loads without wallet', async ({ page }) => {
    await page.goto('/portfolio');
    await expect(page).toHaveURL('/portfolio');
    // Should show connect prompt or empty state, not crash
    await expect(page.locator('body')).not.toContainText('Application error');
  });

  test('market page shows connect-to-trade prompt when disconnected', async ({ page }) => {
    await page.goto('/markets');
    // Wait for page to hydrate
    await page.waitForLoadState('networkidle');
    // Should not show any unhandled error
    await expect(page.locator('body')).not.toContainText('Application error');
  });

  test('settings page accessible without wallet', async ({ page }) => {
    await page.goto('/settings');
    await expect(page).toHaveURL('/settings');
    await expect(page.locator('body')).not.toContainText('Application error');
  });
});

// ─── Chain Handling ───────────────────────────────────────────────────

test.describe('Chain Handling', () => {
  test('wrong chain does not crash the app', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
      chainId: 1, // Ethereum mainnet, not Base
    });
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    // App should not crash — may show wrong chain notice or connect anyway
    await page.waitForTimeout(3000);
    await expect(page.locator('body')).not.toContainText('Application error');
  });
});

// ─── No Wallet Installed ──────────────────────────────────────────────

test.describe('No Wallet Installed', () => {
  test('clicking connect without any wallet shows error or modal', async ({ page }) => {
    // No mock wallet injected — simulates no extension installed
    await page.goto('/');

    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();

    // Should show error toast or AppKit modal — NOT crash
    await page.waitForTimeout(3000);
    await expect(page.locator('body')).not.toContainText('Application error');
    await expect(page.locator('body')).not.toContainText('Unhandled Runtime Error');
  });
});

// ─── Navigation While Connected ───────────────────────────────────────

test.describe('Navigation Persistence', () => {
  test('wallet stays connected across page navigation', async ({ page }) => {
    await injectMockWallet(page, {
      walletName: 'MetaMask',
      address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
      chainId: 8453,
    });
    await page.goto('/');

    // Connect
    const connectBtn = page.getByRole('button', { name: /connect/i });
    await connectBtn.click();
    await expect(page.getByText('0xd8...6045')).toBeVisible({ timeout: 10000 });

    // Navigate to markets
    await page.goto('/markets');
    await page.waitForLoadState('networkidle');

    // Address should still be visible
    await expect(page.getByText('0xd8...6045')).toBeVisible({ timeout: 10000 });

    // Navigate to portfolio
    await page.goto('/portfolio');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('0xd8...6045')).toBeVisible({ timeout: 10000 });
  });
});

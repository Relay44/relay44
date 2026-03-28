import { expect, test } from '@playwright/test';

test.describe('API proxy fallback', () => {
  test('internal market reads degrade to an empty list when Base is unavailable', async ({
    request,
  }) => {
    const response = await request.get('/v1/evm/markets?limit=1&source=internal&tradable=all');

    expect(response.status()).toBe(200);

    const payload = await response.json();
    expect(Array.isArray(payload.markets)).toBeTruthy();
  });

  test('capabilities endpoint stays available through the proxy', async ({ request }) => {
    const response = await request.get('/api/proxy/web4/capabilities');

    expect(response.status()).toBe(200);

    const payload = await response.json();
    expect(payload.launch).toBeTruthy();
    expect(typeof payload.launch.beta).toBe('boolean');
  });

  test('market reads stay available through the proxy', async ({ request }) => {
    const response = await request.get('/api/proxy/evm/markets?limit=1');

    expect(response.status()).toBe(200);

    const payload = await response.json();
    expect(Array.isArray(payload.markets)).toBeTruthy();
    expect(payload.limit).toBe(1);
  });

  test('agent reads stay available through the proxy', async ({ request }) => {
    const response = await request.get('/api/proxy/evm/agents?limit=1&active=true');

    expect(response.status()).toBe(200);

    const payload = await response.json();
    expect(Array.isArray(payload.agents)).toBeTruthy();
    expect(payload.limit).toBe(1);
  });
});

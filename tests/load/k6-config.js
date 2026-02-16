/**
 * Relay44 Load Testing Configuration
 *
 * Uses k6 for load testing. Install: https://k6.io/docs/getting-started/installation/
 *
 * Run:
 *   k6 run tests/load/k6-config.js
 *
 * With options:
 *   k6 run --vus 50 --duration 5m tests/load/k6-config.js
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Configuration
const BASE_URL = __ENV.API_URL || 'http://localhost:8080';

// Custom metrics
const errorRate = new Rate('errors');
const orderLatency = new Trend('order_latency');
const authLatency = new Trend('auth_latency');
const marketLatency = new Trend('market_latency');
const ordersPlaced = new Counter('orders_placed');
const ordersCancelled = new Counter('orders_cancelled');

// Test configuration
export const options = {
  scenarios: {
    // Smoke test: minimal load
    smoke: {
      executor: 'constant-vus',
      vus: 1,
      duration: '30s',
      tags: { scenario: 'smoke' },
      env: { SCENARIO: 'smoke' },
    },

    // Load test: typical production load
    load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 20 },  // Ramp up
        { duration: '5m', target: 20 },  // Sustained load
        { duration: '2m', target: 50 },  // Peak load
        { duration: '5m', target: 50 },  // Sustained peak
        { duration: '2m', target: 0 },   // Ramp down
      ],
      tags: { scenario: 'load' },
      env: { SCENARIO: 'load' },
      startTime: '35s', // Start after smoke
    },

    // Stress test: beyond normal capacity
    stress: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 50 },
        { duration: '5m', target: 100 },
        { duration: '2m', target: 150 },
        { duration: '5m', target: 150 },
        { duration: '2m', target: 0 },
      ],
      tags: { scenario: 'stress' },
      env: { SCENARIO: 'stress' },
      startTime: '17m', // Start after load
    },

    // Spike test: sudden traffic spike
    spike: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 10 },   // Normal load
        { duration: '30s', target: 200 },  // Spike!
        { duration: '1m', target: 200 },   // Sustained spike
        { duration: '30s', target: 10 },   // Recovery
        { duration: '1m', target: 10 },    // Verify recovery
      ],
      tags: { scenario: 'spike' },
      env: { SCENARIO: 'spike' },
      startTime: '35m', // Start after stress
    },
  },

  thresholds: {
    // HTTP errors should be less than 1%
    http_req_failed: ['rate<0.01'],

    // 95th percentile response time < 500ms
    http_req_duration: ['p(95)<500'],

    // Custom thresholds
    errors: ['rate<0.05'],
    order_latency: ['p(95)<1000', 'p(99)<2000'],
    auth_latency: ['p(95)<500'],
    market_latency: ['p(95)<200'],
  },
};

// Simulated wallet for testing
function generateWallet() {
  const id = `test-wallet-${__VU}-${Date.now()}`;
  return {
    address: id,
    // In real tests, would use proper Ed25519 keys
    sign: (message) => 'mock-signature',
  };
}

// Get authentication token
function authenticate(wallet) {
  const start = Date.now();

  // Get nonce
  const nonceResp = http.get(`${BASE_URL}/v1/auth/nonce?wallet=${wallet.address}`);
  if (nonceResp.status !== 200) {
    errorRate.add(1);
    return null;
  }

  const nonce = nonceResp.json('nonce');
  const message = `Sign this message to authenticate with Relay44.\n\nWallet: ${wallet.address}\nNonce: ${nonce}`;

  // Verify (mock signature in load test)
  const verifyResp = http.post(
    `${BASE_URL}/v1/auth/verify`,
    JSON.stringify({
      wallet: wallet.address,
      signature: wallet.sign(message),
      message: message,
    }),
    { headers: { 'Content-Type': 'application/json' } }
  );

  authLatency.add(Date.now() - start);

  if (verifyResp.status !== 200) {
    errorRate.add(1);
    return null;
  }

  return verifyResp.json('access_token');
}

// Main test function
export default function () {
  const wallet = generateWallet();

  group('Health Check', () => {
    const resp = http.get(`${BASE_URL}/health`);
    check(resp, {
      'health check status is 200': (r) => r.status === 200,
      'health status is healthy': (r) => r.json('status') === 'healthy',
    });
  });


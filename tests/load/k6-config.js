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

  group('Markets', () => {
    const start = Date.now();
    const resp = http.get(`${BASE_URL}/v1/markets`);
    marketLatency.add(Date.now() - start);

    const success = check(resp, {
      'markets status is 200': (r) => r.status === 200,
      'markets has data': (r) => r.json('markets') !== undefined,
    });

    errorRate.add(!success);
  });

  group('Authentication', () => {
    // Note: In real load tests, use pre-generated valid signatures
    const token = authenticate(wallet);

    if (token) {
      group('Authenticated Operations', () => {
        const headers = {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        };

        // List orders
        const ordersResp = http.get(`${BASE_URL}/v1/orders`, { headers });
        check(ordersResp, {
          'orders status is 200': (r) => r.status === 200,
        });

        // List positions
        const positionsResp = http.get(`${BASE_URL}/v1/positions`, { headers });
        check(positionsResp, {
          'positions status is 200': (r) => r.status === 200,
        });

        // Place order (if markets available)
        const marketsResp = http.get(`${BASE_URL}/v1/markets?status=active&limit=1`);
        const markets = marketsResp.json('markets');

        if (markets && markets.length > 0) {
          const start = Date.now();
          const orderResp = http.post(
            `${BASE_URL}/v1/orders`,
            JSON.stringify({
              market_id: markets[0].id,
              side: Math.random() > 0.5 ? 'buy' : 'sell',
              outcome: Math.random() > 0.5 ? 'yes' : 'no',
              price: randomIntBetween(10, 90) / 100,
              quantity: randomIntBetween(10, 1000),
              order_type: 'limit',
            }),
            {
              headers: {
                ...headers,
                'Idempotency-Key': `load-test-${__VU}-${Date.now()}`,
              },
            }
          );

          orderLatency.add(Date.now() - start);

          const orderSuccess = check(orderResp, {
            'order placed': (r) => r.status === 201 || r.status === 400, // 400 = validation error, acceptable
          });

          if (orderResp.status === 201) {
            ordersPlaced.add(1);

            // Cancel order
            const orderId = orderResp.json('order_id');
            if (orderId) {
              const cancelResp = http.del(`${BASE_URL}/v1/orders/${orderId}`, null, { headers });
              if (cancelResp.status === 200) {
                ordersCancelled.add(1);
              }
            }
          }

          errorRate.add(!orderSuccess);
        }
      });
    }
  });

  // Think time between iterations
  sleep(randomIntBetween(1, 3));
}

// Setup function - runs once before all VUs
export function setup() {
  console.log(`Starting load test against ${BASE_URL}`);

  // Verify API is accessible
  const resp = http.get(`${BASE_URL}/health`);
  if (resp.status !== 200) {
    throw new Error(`API not accessible: ${resp.status}`);
  }

  return {
    startTime: Date.now(),
  };
}

// Teardown function - runs once after all VUs
export function teardown(data) {
  const duration = (Date.now() - data.startTime) / 1000;
  console.log(`Load test completed in ${duration}s`);
}

// Handle summary
export function handleSummary(data) {
  return {
    'tests/load/summary.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data, options) {
  const lines = [];
  lines.push('\n=== Load Test Summary ===\n');

  // Scenarios
  if (data.metrics) {
    const httpReqs = data.metrics.http_reqs;
    const httpDuration = data.metrics.http_req_duration;
    const httpFailed = data.metrics.http_req_failed;

    if (httpReqs) {
      lines.push(`Total Requests: ${httpReqs.values.count}`);
      lines.push(`RPS: ${(httpReqs.values.rate).toFixed(2)}`);
    }

    if (httpDuration) {
      lines.push(`\nLatency:`);
      lines.push(`  avg: ${httpDuration.values.avg.toFixed(2)}ms`);
      lines.push(`  p50: ${httpDuration.values['p(50)'].toFixed(2)}ms`);
      lines.push(`  p95: ${httpDuration.values['p(95)'].toFixed(2)}ms`);
      lines.push(`  p99: ${httpDuration.values['p(99)'].toFixed(2)}ms`);
    }

    if (httpFailed) {
      lines.push(`\nError Rate: ${(httpFailed.values.rate * 100).toFixed(2)}%`);
    }
  }

  // Thresholds
  if (data.thresholds) {
    lines.push('\nThresholds:');
    for (const [name, threshold] of Object.entries(data.thresholds)) {
      const status = threshold.ok ? 'PASS' : 'FAIL';
      lines.push(`  ${name}: ${status}`);
    }
  }

  return lines.join('\n');
}


import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

const BASE_URL = __ENV.API_URL || 'http://localhost:8080';
const TARGET_QPS = Number(__ENV.TARGET_QPS || 80);
const DURATION = __ENV.DURATION || '10m';
const PRE_ALLOCATED_VUS = Number(__ENV.PRE_ALLOCATED_VUS || Math.max(20, Math.ceil(TARGET_QPS * 0.6)));
const MAX_VUS = Number(__ENV.MAX_VUS || Math.max(200, TARGET_QPS * 4));

const failedChecks = new Rate('failed_checks');
const healthLatency = new Trend('health_latency');
const marketsLatency = new Trend('markets_latency');

export const options = {
  scenarios: {
    public_baseline: {
      executor: 'constant-arrival-rate',
      rate: TARGET_QPS,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: PRE_ALLOCATED_VUS,
      maxVUs: MAX_VUS,
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.01'],
    http_req_duration: ['p(95)<500', 'p(99)<900'],
    health_latency: ['p(95)<250', 'p(99)<500'],
    markets_latency: ['p(95)<400', 'p(99)<800'],
    failed_checks: ['rate<0.01'],
  },
};

export function setup() {
  const response = http.get(`${BASE_URL}/health`);
  if (response.status !== 200) {
    throw new Error(`health endpoint unavailable at ${BASE_URL}/health (status=${response.status})`);
  }

  return { startedAt: Date.now() };
}

export default function () {
  const healthResponse = http.get(`${BASE_URL}/health`);
  healthLatency.add(healthResponse.timings.duration);
  const healthOk = check(healthResponse, {
    'health status is 200': (r) => r.status === 200,
    'health response healthy': (r) => r.json('status') === 'healthy',
  });
  failedChecks.add(!healthOk);

  const marketsResponse = http.get(`${BASE_URL}/v1/markets?limit=20&status=active`);
  marketsLatency.add(marketsResponse.timings.duration);
  const marketsOk = check(marketsResponse, {
    'markets status is 200': (r) => r.status === 200,
    'markets payload has list': (r) => Array.isArray(r.json('markets')),
  });
  failedChecks.add(!marketsOk);

  sleep(0.05);
}

export function teardown(data) {
  const durationSec = (Date.now() - data.startedAt) / 1000;
  console.log(`baseline test duration: ${durationSec.toFixed(1)}s`);
}

export function handleSummary(data) {
  return {
    'tests/load/public-baseline-summary.json': JSON.stringify(data, null, 2),
    stdout: summaryText(data),
  };
}

function summaryText(data) {
  const lines = ['\n=== Public Baseline Summary ==='];
  const metrics = data.metrics || {};
  const req = metrics.http_reqs?.values;
  const duration = metrics.http_req_duration?.values;
  const failed = metrics.http_req_failed?.values;

  if (req) {
    lines.push(`requests: ${req.count}`);
    lines.push(`rps: ${req.rate.toFixed(2)}`);
  }

  if (duration) {
    lines.push(`latency avg: ${duration.avg.toFixed(2)}ms`);
    lines.push(`latency p95: ${duration['p(95)'].toFixed(2)}ms`);
    lines.push(`latency p99: ${duration['p(99)'].toFixed(2)}ms`);
  }

  if (failed) {
    lines.push(`http error rate: ${(failed.rate * 100).toFixed(2)}%`);
  }

  if (data.thresholds) {
    lines.push('thresholds:');
    for (const [name, threshold] of Object.entries(data.thresholds)) {
      lines.push(`- ${name}: ${threshold.ok ? 'PASS' : 'FAIL'}`);
    }
  }

  return `${lines.join('\n')}\n`;
}

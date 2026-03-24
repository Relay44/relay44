# Relay44 Load Testing

Load testing configuration using k6.

## Prerequisites

Install k6:
```bash
# macOS
brew install k6

# Linux
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Docker
docker run --rm -i grafana/k6 run - <script.js
```

## Quick Start

```bash
# Smoke test (minimal load)
k6 run --env SCENARIO=smoke tests/load/k6-config.js

# Load test (20-50 VUs)
k6 run tests/load/k6-config.js

# Custom load
k6 run --vus 100 --duration 10m tests/load/k6-config.js

# Against specific environment
API_URL=https://api.staging.relay44.com k6 run tests/load/k6-config.js

# Public baseline test (target QPS + p95/p99)
API_URL=https://relay44.com/v1 TARGET_QPS=80 DURATION=10m \
  k6 run tests/load/public-baseline.js
```

## Test Scenarios

### Smoke Test
Minimal load to verify basic functionality.
- 1 VU
- 30 seconds
- Validates endpoints respond correctly

### Load Test
Simulates typical production traffic patterns.
- Ramps from 0 to 20 VUs (2 min)
- Sustained 20 VUs (5 min)
- Peak 50 VUs (5 min)
- Ramp down (2 min)

### Stress Test
Pushes beyond normal capacity to find breaking points.
- Ramps up to 150 VUs
- Identifies performance degradation thresholds
- Tests recovery behavior

### Spike Test
Simulates sudden traffic spikes (e.g., market event).
- Quick spike to 200 VUs
- Tests auto-scaling and recovery

## Thresholds

| Metric | Threshold | Description |
|--------|-----------|-------------|
| `http_req_failed` | < 1% | HTTP error rate |
| `http_req_duration` | p95 < 500ms | Response time |
| `order_latency` | p95 < 1000ms | Order placement time |
| `auth_latency` | p95 < 500ms | Authentication time |
| `market_latency` | p95 < 200ms | Market list time |

## Output

Results are written to:
- Console: Real-time metrics
- `tests/load/summary.json`: Detailed results
- `tests/load/public-baseline-summary.json`: Public baseline scenario results

## CI Integration

```yaml
load-test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: grafana/k6-action@v0.3.0
      with:
        filename: tests/load/k6-config.js
        flags: --env SCENARIO=smoke
```

## Grafana Dashboard

For real-time visualization, export to InfluxDB:

```bash
k6 run --out influxdb=http://localhost:8086/k6 tests/load/k6-config.js
```

Import the Grafana dashboard from `infra/grafana/dashboards/k6-load-test.json`.

# Testing

Relay44 uses three test layers aligned to the monorepo structure. All gates must pass before merging to `main`.

## Rust API — Integration Tests

Located in `app/tests/`. Each file covers one API domain against a real database and HTTP stack.

```bash
cargo test --manifest-path app/Cargo.toml --release
```

| File | Scope |
| --- | --- |
| `api_auth.rs` | SIWE login, JWT lifecycle, session validation |
| `api_markets.rs` | Market CRUD, filtering, pagination |
| `api_orders.rs` | Order placement, cancellation, matching |
| `api_positions.rs` | Position open/close, PnL calculation |
| `api_distribution.rs` | Distribution market settlement |
| `api_copy_trading.rs` | Copy trading subscriptions and execution |
| `api_referrals.rs` | Referral tracking and attribution |

Shared fixtures and helpers live in `app/tests/common/`.

### Writing a new test

1. Add a new file in `app/tests/` following the `api_*.rs` naming convention.
2. Use the shared `TestApp` helper from `common/` to bootstrap state.
3. Assert against HTTP responses — do not test internal service methods directly.

## Solidity — Foundry Tests

Located in `evm/test/`. Each contract has a corresponding `.t.sol` file.

```bash
forge test --root evm
```

Mock contracts live in `evm/test/mocks/`. Tests use Foundry's `Test` base with `setUp()` for deployment and state initialization.

## Web — End-to-End Tests

Located in `web/e2e/`. Uses Playwright against a running dev server.

```bash
npm --prefix web run test:e2e
```

Coverage includes navigation, wallet connection, market interactions, responsive layout, and legal page rendering.

### Running specific tests

```bash
# Single Rust test file
cargo test --manifest-path app/Cargo.toml --release --test api_orders

# Single Foundry test contract
forge test --root evm --match-contract MarketCoreTest

# Single Playwright spec
npx --prefix web playwright test e2e/wallet.spec.ts
```

## CI

The full test suite runs in `.github/workflows/ci.yml`. Release builds in `release.yml` gate on the same suite plus `cargo audit` and repository standards checks.

## What to test

- API behavior changes: add or update an integration test in `app/tests/`.
- Contract logic changes: add or update a Foundry test in `evm/test/`.
- UI flow changes: add or update a Playwright spec in `web/e2e/`.
- Configuration or migration changes: verify with `npm run launch:config:prod-strict`.

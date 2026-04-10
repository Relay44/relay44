# Relay44

[![CI](https://github.com/Relay44/relay44/actions/workflows/ci.yml/badge.svg)](https://github.com/Relay44/relay44/actions/workflows/ci.yml)
[![Workflow Lint](https://github.com/Relay44/relay44/actions/workflows/workflow-lint.yml/badge.svg)](https://github.com/Relay44/relay44/actions/workflows/workflow-lint.yml)
[![CodeQL](https://github.com/Relay44/relay44/actions/workflows/codeql.yml/badge.svg)](https://github.com/Relay44/relay44/actions/workflows/codeql.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

![1500x500](https://github.com/user-attachments/assets/6475e5b0-20a4-416b-90a2-92184ca3150b)

Open infrastructure for prediction markets on Base.

Relay44 is a prediction market platform built on Base. It ships a Rust API, Base smart contracts, a Next.js web client, and PostgreSQL migrations in a single monorepo.

**Links**: [relay44.com](https://relay44.com) · [CONTRIBUTING](CONTRIBUTING.md) · [SECURITY](SECURITY.md) · [SUPPORT](SUPPORT.md) · [CHANGELOG](CHANGELOG.md) · [ARCHITECTURE](ARCHITECTURE.md) · [TESTING](TESTING.md)

## What This Is

A vertically integrated prediction market stack:

- **Contracts** — Base-native markets, order books, vaults, and agent execution (`evm/`)
- **API** — Rust backend for market data, compliance, order routing, and write preparation (`app/`)
- **Web** — Next.js frontend for market discovery, trading, and wallet flows (`web/`)
- **Data** — PostgreSQL schema and migrations (`migrations/`)

## Architecture

![Relay44 architecture](assets/architecture/diagram-white.svg)

```
app/            Rust API (Actix-web, PostgreSQL, Redis)
web/            Next.js frontend (App Router, wagmi, TradingView)
evm/            Foundry workspace — Base contracts + tests
programs/       Anchor programs (Solana, experimental)
migrations/     PostgreSQL schema
scripts/        Operator and release tooling
sdk/            TypeScript client SDK
.github/        CI, issue forms, policy
```

## Getting Started

### Prerequisites

- Node.js 22+
- Rust stable toolchain with `rustfmt`
- Docker (PostgreSQL and Redis via Docker Compose)
- Foundry (optional, for Base contract builds and tests)

### Local Setup

```bash
cp .env.example .env
docker compose up -d postgres redis
npm ci
npm ci --prefix web
cargo run --manifest-path app/Cargo.toml
```

Start the web application in a separate terminal:

```bash
npm --prefix web run dev
```

Open `http://localhost:3000`.

The default environment is designed for local development. It does not require production secrets, deployed contract addresses, or funded wallets.

### Enabling Write Flows

Write-enabled Base features require production-grade configuration:

- Deployed contract addresses and Base RPC access
- Wallet and SIWE configuration
- `BOOTSTRAP_OPERATOR_ADDRESS` and an operator wallet in `ADMIN_WALLETS` for bootstrap liquidity automation
- External venue credentials for live external trading
- x402 keys for paid resource flows

Without these, the stack runs normally but the corresponding features remain unavailable.

## Usage

```bash
# Health check
curl https://relay44-api.onrender.com/health

# Local web app
npm --prefix web run dev

# Verify on-chain deployment
npm run launch:onchain:verify
```

Operator agents, indexers, and smoke tests are configured via environment variables. See `.env.example` and the scripts in `scripts/` for details.

## Development

Install hooks before starting work:

```bash
npm run ops:hooks:install
```

Run the validation suite before opening a pull request:

```bash
npm run ops:repo-standards
npm run ops:silo-check:strict
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

Production-oriented checks:

```bash
npm run launch:onchain:verify
npm run launch:config:prod-strict
npm run production:gates:strict
```

## Support

- **Bugs**: Open a GitHub issue with a minimal reproduction
- **Feature proposals**: Open a GitHub issue with problem statement, motivation, and alternatives
- **Usage questions**: See [SUPPORT.md](SUPPORT.md)
- **Security**: See [SECURITY.md](SECURITY.md) — do not use public issues

## Governance

Relay44 uses a maintainer-led governance model. Changes are reviewed through code ownership, repository policy gates, and CI. Release, security, and protocol decisions are maintained by the core team.

- [GOVERNANCE.md](GOVERNANCE.md)
- [MAINTAINERS.md](MAINTAINERS.md)
- [.github/CODEOWNERS](.github/CODEOWNERS)

## Releases

See [RELEASING.md](RELEASING.md) for the tagging and publication process. Notable changes are tracked in [CHANGELOG.md](CHANGELOG.md).

## Security

Do not report vulnerabilities through public issues. Use GitHub Security Advisories or the private contact path in [SECURITY.md](SECURITY.md).

## License

Licensed under [Apache-2.0](LICENSE).

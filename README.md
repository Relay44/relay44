# Relay44

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Security Policy](https://img.shields.io/badge/security-policy-brightgreen.svg)](SECURITY.md)

![Relay44 banner](assets/branding/readme-banner.svg)

Open infrastructure for agentic prediction markets on Base.

Relay44 is a full-stack prediction market system with Base-native contracts, a Rust API, a Next.js web client, PostgreSQL migrations, and public verification tooling. This repository is the public open-source distribution of the platform. Production secrets, funded wallets, and selected operational services are intentionally kept out of the public snapshot.

**Links**
- Product: [relay44.com](https://relay44.com)
- Support: [support@relay44.com](mailto:support@relay44.com)
- Security: [security@relay44.com](mailto:security@relay44.com)

## Table of Contents

- [What Relay44 includes](#what-relay44-includes)
- [Key Capabilities](#key-capabilities)
- [Architecture](#architecture)
- [Public Snapshot Boundary](#public-snapshot-boundary)
- [Repository Layout](#repository-layout)
- [Getting Started](#getting-started)
- [Usage Examples](#usage-examples)
- [Development and Validation](#development-and-validation)
- [Release Model](#release-model)
- [Security](#security)
- [Contributing](#contributing)
- [License](#license)

## What Relay44 includes

- Base smart contracts for markets, order books, vaults, and agent execution.
- Rust API services for market data, compliance enforcement, write preparation, and external venue adapters.
- Next.js web application for market discovery, wallet auth, market creation, and operator-facing flows.
- PostgreSQL migrations and local development infrastructure.
- Public x402 facilitator code and public MCP tooling.
- Launch-readiness, verification, and boundary-enforcement scripts.

## Key Capabilities

- Base-native market infrastructure with explicit write-preparation flows.
- Region and provider-rail enforcement in the API layer.
- Public web client and backend in a single auditable repository.
- x402 support for premium API and MCP resource gating.
- External market venue integration surfaces for user-supplied credentials.
- Open-core publication pipeline with boundary checks before public release.

## Architecture

![Relay44 architecture](assets/architecture/diagram-white.png)

| Layer | Purpose | Main path |
| --- | --- | --- |
| Web | User-facing application and wallet flows | `web/` |
| API | Market data, compliance, writes, and integrations | `app/` |
| Contracts | Base-native protocol contracts | `evm/` |
| Data | PostgreSQL schema and migrations | `migrations/` |
| SDK / Tooling | Client tooling, operator scripts, MCP surfaces | `sdk/`, `scripts/`, `services/` |

## Public Snapshot Boundary

This repository is not the full production estate.

Included here:
- Core product code required to build, run, test, and audit the public stack.
- Public automation, validation, and release tooling.
- Public x402 facilitator code and MCP surfaces.

Excluded from here:
- Production secrets and funded wallets.
- Internal deployment state and operational access.
- Selected private runtime services that are not part of the public distribution.

## Repository Layout

- `app/` - Rust backend.
- `web/` - Next.js frontend.
- `evm/` - Foundry workspace for Base contracts.
- `migrations/` - database schema migrations.
- `sdk/` - SDK and integration surfaces.
- `services/` - public service components such as the x402 facilitator.
- `config/` - repository boundary and runtime configuration.
- `scripts/` - launch, verification, release, and operator tooling.

## Getting Started

### Prerequisites

- Node.js 22+
- Rust stable toolchain
- Docker
- PostgreSQL and Redis via Docker Compose
- Foundry if you want to build or test the Base contracts

### Local bootstrap

The default environment is safe for local bring-up. It does not require production secrets, deployed contract addresses, or funded wallets.

```bash
cp .env.example .env
docker compose up -d postgres redis
npm ci
npm ci --prefix web
cargo run --manifest-path app/Cargo.toml
```

Start the web app in a second terminal:

```bash
npm --prefix web run dev
```

Then open `http://localhost:3000`.

### Enabling write flows

Write-enabled Base features require real production-style configuration:

- deployed contract addresses
- Base RPC access
- wallet and SIWE configuration
- external venue credentials if you want live external trading
- x402 keys if you are enabling paid resource flows
- additional runtime keys for any optional subsystems you turn on

If those inputs are missing, the stack will still run, but the corresponding live features will stay unavailable.

## Usage Examples

### Check the live API

```bash
curl https://relay44-api.onrender.com/health
curl https://relay44-api.onrender.com/v1/web4/capabilities | jq
```

### Run the public web app locally

```bash
npm --prefix web run dev
```

### Verify the Base deployment assumptions

```bash
npm run launch:onchain:verify
```

### Publish a sanitized public snapshot

This command is intended for the private canonical repository. It validates repo boundaries, commit hygiene, and public-safe assets before force-publishing the sanitized mirror.

```bash
npm run ops:publish-public
```

## Development and Validation

Install the repo hooks first:

```bash
npm run ops:hooks:install
```

Recommended validation suite before opening a PR:

```bash
npm run ops:silo-check:strict
npm run ops:open-core-check
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

Production-oriented checks included in this repository:

```bash
npm run launch:onchain:verify
npm run launch:config:prod-strict
npm run production:gates:strict
```

## Release Model

Relay44 uses a split repository model:

- `relay44-core` is the private canonical repository used for full development and production operations.
- `relay44` is the sanitized public repository.
- Public publication is performed from the canonical repository with `npm run ops:publish-public`.

This keeps the public surface auditable while preventing operational leakage into the open-source tree.

## Security

Do not report vulnerabilities in public issues. Use GitHub Security Advisories or the private contact path documented in [SECURITY.md](SECURITY.md).

## Contributing

Read the project policies before opening a PR:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- [GOVERNANCE.md](GOVERNANCE.md)
- [SUPPORT.md](SUPPORT.md)

## License

Licensed under [Apache-2.0](LICENSE).

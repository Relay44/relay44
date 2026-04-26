# Changelog

All notable changes to this project are documented here.

Format: keep unreleased changes under `Unreleased`. Move entries to a dated section when a tag is cut.

## Unreleased

### Added

- Public Protocol Reference at `/docs/contracts` with live mainnet + sepolia addresses, Basescan links, package install path, and a copy-paste viem integration snippet
- Public Tokenomics page at `/tokenomics` covering fee flow, staking tiers, reward allocation, and roadmap
- `GET /api/contracts/[name]/abi` endpoint serving MarketCore, OrderBook, RelayStaking, and ERC20 ABIs as JSON for external integrators
- `GET /v1/protocol/metrics` endpoint and `/protocol` dashboard for public protocol-level markets, agents, settlement volume, and collateral metrics
- `GET /v1/protocol/relay-utility` endpoint exposing chain id, RELAY token state, staking total + tier table with fee-discount bps and x402 bypass flags, reward distributor address, and live utility flags
- `@relay44/protocol` workspace package with generated ABIs, deployment manifest, typed addresses, and helper functions
- `examples/protocol-read-market` TypeScript example that reads `MarketCore.marketCount` on Base mainnet
- npm publish workflow for `@relay44/protocol` and `@relay44/agent-sdk`
- `web/src/lib/protocol.ts` as single source of truth for docs-facing contract metadata
- Footer resources, `/docs` hero, and `/staking` cross-links to the new protocol pages
- Repository standards enforcement with `MAINTAINERS.md`, `RELEASING.md`, `.github/CODEOWNERS`, and automated verification
- Issue forms, PR template, and contributor policy documentation
- Workflow linting for GitHub Actions definitions

### Changed

- Expanded `README.md`, `CONTRIBUTING.md`, `SUPPORT.md`, `SECURITY.md`, and `GOVERNANCE.md`
- `@relay44/agent-sdk` now re-exports protocol artifacts from `@relay44/protocol` instead of maintaining a separate ABI surface

## 0.1.0 - 2026-03-30

### Added

- Initial open-source release covering web, API, contracts, SDK, migrations, and tooling
- Repository validation gates for commit hygiene, asset detection, and standards enforcement

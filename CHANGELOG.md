# Changelog

All notable changes to this project are documented here.

Format: keep unreleased changes under `Unreleased`. Move entries to a dated section when a tag is cut.

## Unreleased

### Added

- Public Protocol Reference at `/docs/contracts` with live mainnet + sepolia addresses, Basescan links, per-contract source links, and a copy-paste viem integration snippet
- Public Tokenomics page at `/tokenomics` covering fee flow, staking tiers, reward allocation, and roadmap
- `GET /api/contracts/[name]/abi` endpoint serving MarketCore, OrderBook, RelayStaking, and ERC20 ABIs as JSON for external integrators
- `web/src/lib/protocol.ts` as single source of truth for docs-facing contract metadata
- Footer resources, `/docs` hero, and `/staking` cross-links to the new protocol pages
- Repository standards enforcement with `MAINTAINERS.md`, `RELEASING.md`, `.github/CODEOWNERS`, and automated verification
- Issue forms, PR template, and contributor policy documentation
- Workflow linting for GitHub Actions definitions

### Changed

- Expanded `README.md`, `CONTRIBUTING.md`, `SUPPORT.md`, `SECURITY.md`, and `GOVERNANCE.md`

## 0.1.0 - 2026-03-30

### Added

- Initial open-source release covering web, API, contracts, SDK, migrations, and tooling
- Repository validation gates for commit hygiene, asset detection, and standards enforcement

# Contributing to Relay44

Read this document before opening an issue or pull request.

Relay44 is an open-core project. Contributions are accepted for the public code in this repository. Private runtime services, funded operational paths, and closed-edge execution logic are out of scope here.

## Start Here

- read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- read [SUPPORT.md](SUPPORT.md) so you choose the right channel
- read [SECURITY.md](SECURITY.md) before reporting anything security-sensitive
- check open issues and pull requests for duplicates before opening a new one

## Choose the Right Channel

### Usage, integration, and operator questions

Start with [SUPPORT.md](SUPPORT.md). If the answer is not already covered there, use the support channel documented in that file instead of opening a bug without evidence.

### Bug reports

Open an issue when you can provide a minimal, reproducible defect report.

Before opening the issue:

- reproduce the problem on the latest `main` branch or latest release tag you can test
- gather exact steps, config assumptions, logs, and observed behavior
- confirm the problem is in open-core code, not a private deployment-only path

If you already have a fix ready, open the pull request directly and include the reproduction and validation evidence there instead of opening both an issue and a PR for the same change.

### Feature proposals

Prefer pull requests for small, concrete changes. For larger work, open an issue with:

- the problem you are solving
- who is affected
- the expected behavior
- alternatives considered
- why the work belongs in the open-source mirror

Maintainers may ask for a pull request instead of keeping a broad feature request open indefinitely.

### Security reports

Do not use public issues. Follow [SECURITY.md](SECURITY.md).

## Local Setup

### Prerequisites

- Node.js 22+
- Rust stable toolchain
- Docker
- Foundry (`forge`, `cast`) if you need EVM contract builds or tests

### Bootstrap

```bash
cp .env.example .env
docker compose up -d postgres redis
npm ci
npm ci --prefix web
```

Optional dev servers:

```bash
cargo run --manifest-path app/Cargo.toml
npm --prefix web run dev
```

## Branching and Scope

- use `main` as the integration base
- use short, descriptive branch names with `relay44/` prefix
- keep pull requests focused and reviewable
- do not mix boundary, release, and product changes unless they are part of the same fix

## Required Checks Before PR

```bash
npm run ops:repo-standards
npm run ops:silo-check:strict
npm run ops:open-core-check
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

If your change only touches documentation or repository metadata, explain which checks were intentionally skipped and why.

## Coding Standards

- match existing style and project conventions
- prefer small, explicit changes over broad refactors
- add tests for behavior changes
- keep public docs honest about what is and is not live
- do not introduce credentials, internal runbooks, private runtime logic, or deployment-only state

## Commit Policy

- commit subjects must be short, lowercase, and descriptive
- AI attribution lines and co-author trailers are not allowed
- install repo hooks with `npm run ops:hooks:install` before starting work
- if a feature branch fails the hygiene gate, rewrite its local commits before asking for review

## Pull Request Expectations

Every PR should make review easy:

- explain what changed and why
- describe user-facing or operator-facing impact
- include validation evidence
- update docs and changelog entries when the public contract changes
- call out breaking changes, migrations, or rollout risk explicitly

Maintainers may reject or defer changes that increase public maintenance burden without a clear operational payoff.

## Review and Merge

- CODEOWNERS define the default review path
- maintainers decide merge order, release timing, and whether a change is backportable
- high-impact changes to contracts, protocol behavior, auth, or security-sensitive paths require stronger review and explicit release notes

## LLM-Assisted Contributions

LLM-assisted contributions are acceptable only if you can fully own the change.

- manually review every generated diff before submitting it
- verify all commands, code, and prose yourself
- be able to answer review comments without deferring to the tool
- do not add AI attribution text to commits or release metadata

## First-Time Contributors

First-time contributors are welcome. Small fixes to docs, CI, validation tooling, and isolated product bugs are the easiest place to start. If you are unsure whether a change belongs here, ask through the support path before investing a large amount of work.

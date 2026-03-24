# Contributing

## Scope

Contributions are accepted for open-core code in this repository.
Private edge runtime code is out of scope for this repo.

## Prerequisites

- Node.js 20+
- Rust stable toolchain
- Foundry (`forge`, `cast`)
- Docker (for local Postgres/Redis)

## Local Setup

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

## Branching

- Use `main` as the integration base.
- Use short, descriptive branch names with `relay44/` prefix.
- Keep PRs focused and reviewable.

## Required Checks Before PR

```bash
npm run ops:silo-check:strict
npm run ops:open-core-check
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

## Commit Metadata Policy

- Commit subjects must be short, lowercase, and descriptive.
- AI attributions and co-author trailers are not allowed.
- Commits with AI-style author names or attribution lines are blocked by local hooks and CI.
- Install repo hooks with `npm run ops:hooks:install` before you start work.
- If a feature branch fails the gate, rewrite its local commits with `npm run ops:rewrite-branch-commits`, then force-push the branch with lease protection.

## Coding Standards

- Match existing style and project conventions.
- Prefer small, explicit changes over broad refactors.
- Add tests for behavior changes.
- Do not introduce credentials, internal runbooks, or private edge logic.

## Pull Requests

1. Open a PR against `main`.
2. Fill out the PR template completely.
3. Link relevant issues and include verification evidence.
4. Wait for required checks and maintainer approval.

## Security Contributions

Do not open public issues for vulnerabilities.
Use the private reporting flow in `SECURITY.md`.

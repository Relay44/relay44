# Contributing to Relay44

Read this document before opening an issue or pull request.

## Before You Start

- Read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- Read [SUPPORT.md](SUPPORT.md) to choose the right channel.
- Read [SECURITY.md](SECURITY.md) before reporting anything security-related.
- Check open issues and pull requests for duplicates.

## Choosing the Right Channel

### Usage and Integration Questions

Start with [SUPPORT.md](SUPPORT.md). If the answer is not covered there, use the support channel listed in that file rather than opening an issue.

### Bug Reports

Open an issue when you can provide a minimal, reproducible defect report.

Before opening:

- Reproduce the problem on the latest `main` branch or latest release tag.
- Gather exact steps, configuration, logs, and observed behavior.
- Confirm the problem is reproducible from the code in this repository.

If you already have a fix, open the pull request directly and include the reproduction and validation evidence there.

### Feature Proposals

For small, concrete changes, open a pull request directly. For larger work, open an issue with:

- The problem being solved
- Who is affected
- Expected behavior
- Alternatives considered

Maintainers may close broad feature requests and ask for a focused pull request instead.

### Security Reports

Do not use public issues. Follow [SECURITY.md](SECURITY.md).

## Local Setup

### Prerequisites

- Node.js 22+
- Rust stable toolchain
- Docker
- Foundry (`forge`, `cast`) for EVM contract builds and tests

### Bootstrap

```bash
cp .env.example .env
docker compose up -d postgres redis
npm ci
npm ci --prefix web
```

Optional development servers:

```bash
cargo run --manifest-path app/Cargo.toml
npm --prefix web run dev
```

## Branching and Scope

- Branch from `main`.
- Use short, descriptive branch names with a `relay44/` prefix.
- Keep pull requests focused and reviewable.
- Do not mix unrelated changes in a single pull request.

## Required Checks

Run the following before opening a pull request:

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

If your change only touches documentation or repository metadata, note which checks were skipped and why.

## Coding Standards

- Match existing style and conventions.
- Prefer small, targeted changes over broad refactors.
- Add tests when behavior changes.
- Keep documentation accurate.
- Do not introduce credentials, private runtime state, or deployment-specific configuration.

## Commit Policy

- Commit subjects: short, lowercase, descriptive.
- No AI attribution lines or co-author trailers.
- Install hooks with `npm run ops:hooks:install` before starting work.
- Fix hygiene gate failures before requesting review.

## Pull Request Expectations

Every pull request should include:

- What changed and why.
- User-facing or operator-facing impact.
- Validation evidence (tests, logs, screenshots).
- Updated documentation and changelog entries where applicable.
- Explicit callout of breaking changes, migrations, or rollout risk.

Maintainers may reject or defer changes that increase maintenance burden without clear operational value.

## Review and Merge

- CODEOWNERS defines the default review path.
- Maintainers control merge order, release timing, and backport decisions.
- Contract changes, protocol behavior, authentication, and security-sensitive paths require thorough review and release notes.

## LLM-Assisted Contributions

LLM-assisted contributions are permitted if you take full ownership of the result.

- Review every generated diff before submitting.
- Verify all commands, code, and prose.
- Be prepared to answer review comments without deferring to the tool.
- Do not add AI attribution to commits or release metadata.

## First-Time Contributors

Small fixes to documentation, CI, validation tooling, and isolated product bugs are a good place to start. If you are unsure whether a change belongs in this repository, ask through the support channel before investing significant effort.

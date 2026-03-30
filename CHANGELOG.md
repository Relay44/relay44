# Changelog

All notable public-facing changes to the Relay44 open-source mirror are documented here.

The repository follows a simple changelog discipline:

- keep the current release candidate under `Unreleased`
- summarize changes that matter to public users, contributors, or operators of the open-source stack
- move those entries into a dated release section when a public tag is cut

## Unreleased

### Added

- maintainer-facing repository standards with `MAINTAINERS.md`, `RELEASING.md`, `.github/CODEOWNERS`, and a public repo standards verifier
- a stronger contribution and support surface with improved issue forms, PR template, and maintainer policy docs
- workflow linting for GitHub Actions definitions

### Changed

- raised the open-source mirror contract so publication now verifies repository standards in addition to boundary and hygiene checks
- expanded `README.md`, `CONTRIBUTING.md`, `SUPPORT.md`, `SECURITY.md`, and `GOVERNANCE.md` to reflect a maintainer-run enterprise open-source workflow

## 0.1.0 - 2026-03-30

### Added

- public open-source distribution of the Relay44 platform covering web, API, contracts, SDK, migrations, and release tooling
- repository policy gates for open-core boundary enforcement, internal asset detection, and commit hygiene

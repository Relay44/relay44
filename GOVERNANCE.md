# Governance

## Maintainer Model

relay44 uses a maintainer-led model.

- Maintainers review and merge pull requests.
- Maintainers own release and incident decisions.
- Maintainers enforce repository policy and security requirements.

## Decision Process

- Small changes: maintainer review and merge.
- High-impact changes (security, protocol behavior, API contracts): maintainer consensus and explicit release notes.
- Emergency fixes: fast-track merge allowed, followed by post-merge review.

## Release Responsibility

- Tagging and release notes are maintainer-owned.
- Production deployment and rollback actions are executed through workflow gates.

## Repository Boundaries

This repository is open core only.
Private edge runtime remains in a separate private repository/workspace.

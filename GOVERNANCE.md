# Governance

Relay44 uses a maintainer-led governance model.

## Roles

### Maintainers

Maintainers review and merge pull requests, manage issue triage, enforce repository policy, and decide when work is ready for release.

### Release Stewards

Release stewards control version tags, release notes, and publication. They may defer otherwise correct changes if release timing or operational risk requires it.

### Security Responders

Security responders handle vulnerability intake, coordinated disclosure, and emergency decisions when user funds, authentication, or protocol integrity are at risk.

## Decision Model

- Low-risk changes: maintainer review and merge.
- High-impact changes: explicit maintainer consensus.
- Security or incident response: responders may act immediately, with follow-up review after the system is stable.

The following always require maintainer consensus:

- Contract semantics or settlement behavior
- Authentication, wallet, or session model changes
- Public API changes
- Security controls and disclosure policy
- Release workflow changes

## Merge Policy

- Maintainers decide review depth and required approvals.
- CODEOWNERS define the default review path, not an entitlement to merge.
- Maintainers may request narrower scope, additional tests, or release notes before merge.
- Stale or low-signal changes may be closed rather than carried indefinitely.

## Release Policy

- Tags, release notes, and publication are maintainer-owned.
- Security fixes may be disclosed on a delayed schedule when coordinated disclosure is required.

See [RELEASING.md](RELEASING.md) for the full checklist.

## Repository Standards

Automated validation scripts and CI enforce code quality and repository hygiene. Maintainers may reject otherwise useful changes if they fail these gates.

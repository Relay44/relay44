# Governance

Relay44 uses a maintainer-led governance model.

## Roles

### Maintainers

Maintainers review and merge pull requests, manage issue triage, enforce repository policy, and decide when work is ready for release.

### Release Stewards

Release stewards control version tags, release notes, and public mirror publication. They may defer otherwise correct changes if release timing or operational risk is wrong.

### Security Responders

Security responders own private vulnerability intake, coordinated disclosure timing, and emergency policy decisions when user funds, auth, or protocol integrity are at risk.

## Decision Model

- low-risk changes: maintainer review and merge
- high-impact changes: explicit maintainer consensus
- security or incident response: responders may act immediately, with follow-up review after the system is stable

The following always count as high-impact:

- contract semantics or settlement behavior
- auth, wallet, or session model changes
- public API contract changes
- security controls and disclosure policy
- release and publication workflow changes

## Merge Policy

- maintainers decide review depth and required approvals
- CODEOWNERS define the default ownership path, not an entitlement to merge
- maintainers may request narrower scope, additional tests, or release notes before merge
- stale or low-signal changes may be closed rather than carried indefinitely

## Release Policy

- tags, release notes, and mirror publication are maintainer-owned
- public releases should reflect the state of the open-source mirror, not hidden local work
- security fixes may be disclosed on a delayed timetable when coordinated disclosure is required

See [RELEASING.md](RELEASING.md) for the operational checklist.

## Repository Boundary

This repository is open core only.

- public code belongs here
- private runtime services do not
- internal deployment state does not
- open-core code must never depend on closed-edge runtime paths

Boundary enforcement is automated through repo scripts and CI. Maintainers may reject otherwise useful changes if they weaken that separation.

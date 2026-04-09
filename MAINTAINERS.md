# Maintainers

## Active Maintainers

| Name | GitHub | Area |
| --- | --- | --- |
| 0p3r4t0r44 | [@0p3r4t0r44](https://github.com/0p3r4t0r44) | Architecture, backend, contracts, operations |

## Contact

| Queue | Address | Use |
| --- | --- | --- |
| `support@relay44.com` | Maintainer queue | Issue triage, contributor questions, review routing |
| `security@relay44.com` | Security response | Vulnerability intake, coordinated disclosure |
| `hello@relay44.com` | General | Release notes, governance, administrative escalation |

## Response Targets

| Queue | Target |
| --- | --- |
| Issue acknowledgement | 5 business days |
| Pull request first response | 5 business days |
| Security acknowledgement | 72 hours |
| Release blocker escalation | Same day during release windows |

These are operating targets, not guarantees.

## Ownership Map

| Path | Owner |
| --- | --- |
| `app/`, `migrations/`, `services/`, `scripts/` | @0p3r4t0r44 |
| `web/`, `sdk/` | @0p3r4t0r44 |
| `evm/`, `programs/` | @0p3r4t0r44 |
| `.github/`, policy docs, release metadata | @0p3r4t0r44 |

The authoritative ownership map for GitHub review routing is in [.github/CODEOWNERS](.github/CODEOWNERS).

## Expectations

Maintainers are expected to:

- Keep documentation accurate and up to date.
- Triage issues and pull requests with enough context for contributors to act.
- Enforce repository standards and reject changes that introduce secrets or private deployment state.
- Require tests, documentation, or release notes when behavior changes.
- Act quickly when security or user-funds risk is involved.

## Escalation

- `support@relay44.com` — contributor and triage escalation
- `security@relay44.com` — vulnerability or abuse escalation
- `hello@relay44.com` — release-blocking or governance issues

# Maintainers

Relay44 uses role aliases rather than publishing individual maintainer identities in the repository metadata.

## Contact Roles

| Alias | Responsibility | Typical use |
| --- | --- | --- |
| `support@relay44.com` | maintainer queue | issue triage, contributor questions, general review routing |
| `security@relay44.com` | security response | vulnerability intake, security-sensitive review, coordinated disclosure |
| `hello@relay44.com` | release and governance | release notes, publication, policy, and administrative escalation |

## Response Targets

| Queue | Target |
| --- | --- |
| issue acknowledgement | within 5 business days |
| pull request first maintainer response | within 5 business days |
| security acknowledgement | within 72 hours |
| release blocker escalation | same day when actively in a release window |

These are operating targets, not guarantees.

## Ownership Map

| Path | Primary owner | Secondary owner |
| --- | --- | --- |
| `.github/`, release metadata, policy docs | `hello@relay44.com` | `security@relay44.com` |
| `app/`, `sdk/`, `services/`, `scripts/`, `config/`, `migrations/` | `support@relay44.com` | `hello@relay44.com` |
| `web/` | `support@relay44.com` | `hello@relay44.com` |
| `evm/`, `programs/` | `support@relay44.com` | `security@relay44.com` |
| `SECURITY.md`, security workflows, auth and trust boundaries | `security@relay44.com` | `hello@relay44.com` |

The authoritative path map used by GitHub review routing lives in [.github/CODEOWNERS](.github/CODEOWNERS).

## Maintainer Expectations

Maintainers are expected to:

- keep public docs honest about feature readiness and repository boundaries
- triage issues and pull requests with enough context that contributors can act
- protect the open-core boundary and reject leaks of private runtime code or internal deployment state
- require tests, docs, or release notes when public behavior changes
- act quickly when security or user-funds risk is involved

## Escalation

Use:

- `support@relay44.com` for normal contributor and triage escalation
- `security@relay44.com` for vulnerability or abuse-path escalation
- `hello@relay44.com` for release-blocking governance or publication issues

# Security Policy

## Supported Versions

Security fixes are applied to the latest `main` branch and the latest tagged release line.

| Version | Supported |
| --- | --- |
| `main` | yes |
| latest release tag | yes |
| older tags | no |

## Reporting a Vulnerability

Do not open public issues for vulnerabilities.

Use one of these private channels:

1. GitHub Security Advisory (preferred)
2. Email: `hello@relay44.com` with subject `Security Report`

Include:

- affected component/path
- clear reproduction steps
- impact assessment
- proposed fix (if available)

## Disclosure Process

- Initial acknowledgement target: within 72 hours
- Triage status update target: within 7 days
- Coordinated disclosure after fix validation and release

## Scope

In scope:

- contracts under `evm/`
- API services under `app/`
- web client under `web/`
- repository and CI/CD workflow security

Out of scope:

- vulnerabilities in private edge runtime (tracked privately)
- issues requiring physical access to maintainer systems

## Safe Harbor

Good-faith security research within this policy will not be pursued legally by maintainers.

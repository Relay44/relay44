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

Preferred channels:

1. GitHub Security Advisory for the repository
2. Email `security@relay44.com` with subject `Security Report`

Include:

- affected component or path
- reproduction steps or proof of impact
- severity or exploitability assessment
- proposed fix or mitigation, if you have one

## Response Targets

- acknowledgement target: within 72 hours
- initial triage update: within 7 days
- coordinated disclosure after fix validation and release preparation

These are targets, not guarantees, but maintainers treat them as the operating standard.

## Scope

In scope:

- contracts under `evm/`
- API services under `app/`
- web client under `web/`
- repository CI/CD and release automation
- credential handling, auth, and signing flows in this repository

Out of scope:

- issues that require physical access to maintainer systems
- unsupported historical tags with no active fix line

## Disclosure Policy

- keep reports private until maintainers confirm a fix or mitigation path
- maintainers may request extra time when user safety or coordinated rollout requires it
- public advisories should describe impact, affected versions, and upgrade guidance

## Safe Harbor

Good-faith security research that follows this policy will not be pursued legally by project maintainers. Do not exploit vulnerabilities for real-user impact, data access, service degradation, or financial gain.

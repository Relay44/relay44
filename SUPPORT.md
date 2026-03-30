# Support

Use this file to choose the right support path before opening an issue.

## Which Channel to Use

| Need | Best path |
| --- | --- |
| Reproducible bug in open-core code | GitHub issue |
| Feature proposal | GitHub issue |
| Usage, integration, or setup question | `support@relay44.com` |
| Security concern | [SECURITY.md](SECURITY.md) |
| Conduct concern | [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) |

## Before You Ask for Help

Gather enough information that someone else can act on it:

- exact command, route, or code path involved
- version or commit SHA
- environment assumptions
- logs, stack traces, or screenshots if relevant
- expected behavior and actual behavior

Questions without concrete context are slow to resolve and may be redirected until that information is added.

## Issue Triage Rules

Open an issue when all of the following are true:

- the problem is reproducible
- the problem belongs to open-core code in this repository
- you can describe impact and reproduction clearly

Do not open a public issue for:

- private deployment or credential problems you cannot reproduce in open-core code
- security vulnerabilities
- general product or business inquiries

## Response Expectations

- community issues are triaged on a best-effort basis
- maintainers prioritize security, correctness, release blockers, and reproducible regressions
- urgent production support for private runtime systems is not handled through the public issue tracker

## What Maintainers Need in a Good Report

- minimal reproduction steps
- version or commit
- affected area (`app/`, `web/`, `evm/`, `sdk/`, or repo tooling)
- whether the issue is a regression
- logs or traces that identify the failure mode

If you can also point to the failing file, route, or workflow, triage gets much faster.

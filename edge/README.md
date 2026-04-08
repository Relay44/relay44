# Edge

This directory contains public interface contracts and boundary documentation for components that are not included in this repository.

## Allowed Contents

- `README.md`
- `LICENSE`
- `interfaces/` (public interface contracts only)

## Not Included

- Private execution logic
- Production operator code
- Internal runbooks or incident data

## Dependency Direction

Code in this repository must not import from private edge paths. Edge components may depend on the public codebase, but not the reverse.

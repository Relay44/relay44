# Releasing Relay44

## Versioning

- Tags use `vMAJOR.MINOR.PATCH`.
- Prereleases may use `vMAJOR.MINOR.PATCH-suffix`.

## Pre-Release Checklist

Before tagging:

- Update `CHANGELOG.md` with all notable changes.
- Confirm CI is green on `main`.
- Confirm documentation matches the behavior being released.

Validation suite:

```bash
npm run ops:repo-standards
npm run ops:silo-check:strict
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

## Tagging

1. Merge release-ready changes to `main`.
2. Update `CHANGELOG.md`.
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin vX.Y.Z`
5. `.github/workflows/release.yml` creates the GitHub Release from the tag.

## Release Notes

The GitHub Release page may use auto-generated notes. `CHANGELOG.md` is the curated summary of notable changes. If a change matters to users, contributors, or operators, it should appear there.

## Security Releases

- Handle security-sensitive work privately.
- Coordinate disclosure through [SECURITY.md](SECURITY.md).
- Publish the fix and advisory only after the mitigation is ready.

## Rollback

If a tagged release contains a defect:

- Fix forward when possible.
- Publish a follow-up patch tag.

# Releasing Relay44

This file describes the public release process for the open-source mirror.

## Versioning

- tags use `vMAJOR.MINOR.PATCH`
- prereleases may use `vMAJOR.MINOR.PATCH-suffix`
- public tags should reflect the state of the open-source mirror, not unpublished local work

## Release Inputs

Before tagging a release:

- ensure `CHANGELOG.md` is updated
- ensure repo standards, boundary checks, and CI are green
- confirm public docs match the behavior being released
- confirm there is no internal deployment state or closed-edge code in the mirrored tree

Recommended validation suite:

```bash
npm run ops:repo-standards
npm run ops:silo-check:strict
npm run ops:open-core-check
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

## Tagging and GitHub Release

1. Merge the release-ready changes to `main`.
2. Update `CHANGELOG.md`.
3. Create a semantic tag: `git tag vX.Y.Z`.
4. Push the tag.
5. Let `.github/workflows/release.yml` create the GitHub release notes from that tag.

## Public Mirror Publication

Relay44 uses a split-repository model:

- `relay44-core` is the private canonical repository
- `relay44` is the sanitized open-source mirror

Publish the mirror from the canonical repository:

```bash
npm run ops:publish-public
```

That command must pass:

- git hook verification
- repository standards verification
- silo and open-core boundary checks
- internal asset checks
- commit hygiene checks

## Release Notes Discipline

The GitHub release page may use generated notes, but `CHANGELOG.md` is still the curated summary of notable public-facing changes. If a change matters to downstream users, contributors, or operators of the open-source stack, it should be reflected there.

## Security Releases

- handle security-sensitive work privately first
- coordinate disclosure through [SECURITY.md](SECURITY.md)
- publish the public fix and advisory only when the mitigation path is ready

## Rollback

If a tagged release is wrong:

- fix forward when possible
- publish a follow-up patch tag for public consumers
- if the mirror itself was published incorrectly, republish the sanitized mirror from the corrected canonical commit

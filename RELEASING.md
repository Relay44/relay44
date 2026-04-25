# Releasing Relay44

## Versioning

- Tags use `vMAJOR.MINOR.PATCH`.
- Prereleases may use `vMAJOR.MINOR.PATCH-suffix`.

## Pre-Release Checklist

Before tagging:

- Update `CHANGELOG.md` with all notable changes.
- Confirm CI is green on `main`.
- Confirm documentation matches the behavior being released.
- Confirm `NPM_TOKEN` exists in GitHub Actions secrets.
- Confirm `.github/release-notes/vX.Y.Z.md` includes package names, contract addresses, production endpoints, and known limitations.

Validation suite:

```bash
npm run ops:repo-standards
npm run ops:silo-check:strict
npm run ops:no-internal-assets:tracked
npm run ops:commit-hygiene
npm --prefix web run lint
npm --prefix web run build
npm run sdk:check
npm --workspace @relay44/protocol pack --dry-run
npm --workspace @relay44/agent-sdk pack --dry-run
cargo test --manifest-path app/Cargo.toml --release
forge test --root evm
```

## Tagging and GitHub Release

1. Merge release-ready changes to `main`.
2. Update `CHANGELOG.md`.
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin vX.Y.Z`
5. `.github/workflows/release.yml` creates the GitHub Release from the tag.
6. `.github/workflows/publish-npm.yml` publishes `@relay44/protocol` and `@relay44/agent-sdk` from the GitHub Release event.

## Release Notes

The GitHub Release page may use auto-generated notes. `CHANGELOG.md` is the curated summary of notable changes. If a change matters to users, contributors, or operators, it should appear there.

## Security Releases

- Handle security-sensitive work privately.
- Coordinate disclosure through [SECURITY.md](SECURITY.md).
- Publish the fix and advisory only after the mitigation is ready.

## Rollback

If a tagged release introduces a defect:

### Fix Forward (preferred)

1. Land the fix on `main` through normal PR review.
2. Run the full validation suite.
3. Tag a new patch release (`vX.Y.Z+1`).
4. Update `CHANGELOG.md` with the fix and the defect it addresses.

Fix forward is preferred because it preserves a linear release history and avoids revert conflicts.

### Revert and Patch

Use this when the defect is severe enough that the fix cannot be ready quickly (data corruption, fund risk, security vulnerability).

1. Identify the merge commit that introduced the defect.
2. `git revert <commit>` on `main`. Do not force-push or rewrite history.
3. Run the validation suite to confirm the revert is clean.
4. Tag a patch release from the reverted state.
5. Deploy the patch and verify the defect is resolved in production.
6. Follow up with a proper fix in a subsequent release.

### Service Rollback

If the defect is in a deployed service and a code fix is not immediately available:

1. In the Render dashboard, select the last known-good deploy for the affected service.
2. Trigger a manual deploy from that commit.
3. Verify health checks pass and the defect is no longer present.
4. Open an incident issue documenting the rollback, root cause, and follow-up plan.

### When Not to Roll Back

- Test failures that do not affect production behavior.
- Documentation-only regressions.
- CI configuration issues that do not block deploys.

In these cases, fix forward with a normal PR.

## Summary

Describe what changed and why.

## Scope

- [ ] API (`app/`)
- [ ] Contracts (`evm/`)
- [ ] Solana programs (`programs/`)
- [ ] Web (`web/`)
- [ ] SDK (`sdk/`)
- [ ] Docs/ops

## Validation

- [ ] `npm run ops:silo-check:strict`
- [ ] `npm run ops:open-core-check`
- [ ] `npm run ops:no-internal-assets:tracked`
- [ ] `cargo test --manifest-path app/Cargo.toml --release`
- [ ] `forge test --root evm`

## Security

- [ ] No secrets/credentials/internal assets added
- [ ] Access control/auth implications reviewed
- [ ] Dependency risk reviewed for new packages/crates

## Compatibility

- [ ] No breaking API changes
- [ ] Breaking changes documented
- [ ] Migration/update notes included (if needed)

## Linked Issues

Reference issue numbers and related PRs.

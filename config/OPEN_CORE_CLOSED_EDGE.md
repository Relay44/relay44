# Open-Core Boundary

Relay44 ships from a private canonical repository and publishes a sanitized public mirror.

Open-core code in this repository includes:
- `app/`
- `evm/`
- `programs/`
- `sdk/`
- `web/`
- `migrations/`
- public `scripts/`, `config/`, and `services/`

Closed-edge code stays out of the public snapshot:
- `edge/` runtime internals, except the boundary placeholders required by policy
- `services/xmtp-bridge/`
- `scripts/dx-terminal-pro.sh`
- `scripts/render-deploy.mjs`
- internal deployment state such as `.render-workspace-lock.json`
- root `docs/`, which are kept in the canonical repo and stripped during publication

Publication flow:
1. Run the boundary and hygiene checks from the canonical repo.
2. Build a public snapshot from `config/public-mirror.json`.
3. Strip private package scripts and closed-edge paths from that snapshot.
4. Force-push the sanitized tree to the public mirror.

Rule: open-core paths must never import or reference closed-edge runtime code.

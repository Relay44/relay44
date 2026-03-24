# relay44 closed edge boundary

This repository is open core. Closed edge runtime code is not stored here.

Allowed contents under `edge/`:
- `README.md`
- `LICENSE`
- `interfaces/` (public interface contracts only)

Not allowed in this repo:
- private execution logic
- production operator code
- internal runbooks or incident evidence

Rules:
- Open core must never import closed edge runtime paths.
- Closed edge may depend on open core, never the reverse.
- Private edge code stays in a separate private repository/workspace.

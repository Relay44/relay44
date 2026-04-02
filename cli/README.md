# r44

Relay44 CLI — prediction markets from the terminal.

Browse markets, place orders, manage agents, and trade across Polymarket, Limitless, and relay44's native venue from one unified tool.

## Install

```bash
cargo install r44
```

Or build from source:

```bash
git clone https://github.com/relay44/relay44
cd relay44
cargo build --release -p r44
# binary at target/release/r44
```

## Quick start

```bash
# guided setup
r44 setup

# browse markets
r44 markets list
r44 markets list --query "bitcoin" --status open

# inspect a market
r44 markets get <MARKET_ID>
r44 markets orderbook <MARKET_ID>

# authenticate
r44 login solana --wallet <PUBKEY> --private-key <KEY>

# trade
r44 orders place --market <ID> --side buy --price 0.65 --size 100
r44 orders list
r44 positions list

# agents
r44 agents public
r44 agents list

# wallet
r44 wallet balance

# interactive mode
r44 shell
```

## Output formats

Default output is a styled table. Use `--output json` (or `R44_OUTPUT=json`) for machine-readable output — ideal for piping to `jq` or consuming from scripts and agents.

```bash
r44 markets list --output json | jq '.[0].id'
```

## Environment variables

| Variable | Description |
|---|---|
| `R44_API_URL` | API base URL (default: relay44 production) |
| `R44_ACCESS_TOKEN` | JWT token (overrides stored config) |
| `R44_OUTPUT` | Output format: `table` or `json` |
| `R44_QUIET` | Suppress non-essential output |
| `R44_WALLET` | Solana wallet address for login |
| `R44_PRIVATE_KEY` | Ed25519 private key for agent login |

## Shell completions

```bash
# bash
r44 completions bash >> ~/.bashrc

# zsh
r44 completions zsh > ~/.zfunc/_r44

# fish
r44 completions fish > ~/.config/fish/completions/r44.fish
```

## License

Apache-2.0

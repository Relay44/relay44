# r44

Relay44 CLI for market ops, trading, and agent execution.

It stays a regular CLI. The new pieces are there to make repeated operator work less brittle: named profiles, a real shared shell parser, local workflows, local hooks, and shell session logs.

## Install

```bash
cargo install r44
```

Or build from source:

```bash
git clone https://github.com/relay44/relay44
cd relay44
cargo build --release -p r44
```

## Quick start

```bash
# configure a profile
r44 setup

# check the profile
r44 doctor
r44 profile list

# browse markets
r44 markets list
r44 markets get <MARKET_ID>

# authenticate
r44 login solana --wallet <PUBKEY> --private-key <KEY>

# trade
r44 orders place --market <ID> --side buy --price 0.65 --size 100
r44 positions list

# interactive mode
r44 shell
```

## Profiles

Profiles isolate API URL, auth state, wallet, and default output mode.

```bash
r44 profile list
r44 profile show
r44 --profile prod doctor
r44 --profile staging config set-url http://localhost:3000/v1
r44 profile use prod
```

The active profile is stored in the config file. Override it per command with `--profile <name>`.

## Doctor

`r44 doctor` checks:

- API reachability
- authenticated request viability
- wallet presence
- profile completeness
- shell completion installation

Use it after setup, after switching profiles, or when an operator environment feels off.

## Shell

`r44 shell` uses the same clap parser as the normal CLI. If a command works at the top level, it works in the shell.

Inside the shell:

- omit the `r44` prefix
- use `help` or `help <command>`
- use `Ctrl-R` for history search
- use `exit` or `quit` to leave

The shell persists:

- command history
- session logs as JSONL

## Workflows

Workflows are local command sequences stored in CLI config. They are not plugins and they are not repo-managed.

```bash
r44 workflow list
r44 workflow validate
r44 workflow run morning-check
r44 workflow run market-check -- market-123
r44 workflow run market-check --dry-run -- market-123
```

Supported placeholders inside workflow steps:

- `{{1}}`, `{{2}}`, ... for positional args
- `{{args}}` for all args
- `{{profile}}`
- `{{api_url}}`

Example workflow config:

```json
{
  "workflows": {
    "market-check": {
      "description": "Inspect one market",
      "steps": [
        "markets get {{1}}",
        "markets trades {{1}} --limit 10",
        "markets orderbook {{1}}"
      ]
    }
  }
}
```

## Hooks

Hooks are local shell commands that run before or after specific command paths.

Supported stages:

- `pre`
- `post`

Hook matching is exact on command path, for example:

- `orders place`
- `orders cancel-all`
- `workflow run`

Example:

```json
{
  "hooks": [
    {
      "command": "orders place",
      "stage": "pre",
      "run": "echo about to place order >&2",
      "required": true,
      "enabled": true
    }
  ]
}
```

Available hook env vars:

- `R44_HOOK_STAGE`
- `R44_COMMAND_PATH`
- `R44_PROFILE`
- `R44_API_URL`
- `R44_SOURCE`
- `R44_EXIT_STATUS`
- `R44_DURATION_MS`

## Sessions

Shell commands are logged locally as JSONL.

```bash
r44 session export
r44 --output json session export --limit 20
r44 session replay ~/.config/r44/sessions.jsonl
r44 session replay ~/.config/r44/sessions.jsonl --execute
```

`session replay` is dry-run by default. Use `--execute` to actually rerun commands.

## Config layout

The CLI config stores:

- `active_profile`
- `profiles`
- `workflows`
- `hooks`
- `session_log`
- `aliases`

Example:

```json
{
  "active_profile": "prod",
  "profiles": {
    "prod": {
      "api_url": "https://relay44-api.onrender.com/v1",
      "wallet": "wallet-1",
      "output": "table"
    },
    "staging": {
      "api_url": "http://localhost:3000/v1",
      "output": "json"
    }
  },
  "aliases": {
    "ob": "markets orderbook",
    "bal": "wallet balance"
  }
}
```

## Environment variables

| Variable | Description |
|---|---|
| `R44_API_URL` | Override API base URL |
| `R44_PROFILE` | Override selected profile |
| `R44_ACCESS_TOKEN` | Override stored access token |
| `R44_OUTPUT` | Output format: `table` or `json` |
| `R44_QUIET` | Suppress non-essential output |
| `R44_WALLET` | Solana wallet address for login |
| `R44_PRIVATE_KEY` | Ed25519 private key for agent login |

## Shell completions

```bash
r44 completions bash >> ~/.bashrc
r44 completions zsh > ~/.zfunc/_r44
r44 completions fish > ~/.config/fish/completions/r44.fish
```

## License

Apache-2.0

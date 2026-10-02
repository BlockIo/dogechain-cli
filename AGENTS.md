# Using `dogechain` from scripts and AI agents

`dogechain` reads the Dogecoin blockchain from Dogechain.com. It is read-only:
it never needs keys and never sends transactions.

## Rules of thumb

- Add `--json` to any command (or set `DOGECHAIN_JSON=1`). stdout is then the
  API's reply, unchanged: `{"status":"success","data":{...}}`.
- On failure, stdout is empty and stderr has
  `{"status":"fail","data":{"error_message":"...","code":"..."}}`.
- Branch on the exit code:

  | Exit | Code           | Meaning                                             |
  |------|----------------|-----------------------------------------------------|
  | 0    |                | ok                                                  |
  | 1    | `OTHER`        | anything else                                       |
  | 2    | `BAD_INPUT`    | invalid arguments, or the API rejected the input    |
  | 3    | `NOT_FOUND`    | no such block, transaction or address               |
  | 4    | `UNAVAILABLE`  | Dogechain.com unreachable or busy; retry later      |
  | 5    | `RATE_LIMITED` | still rate limited after waiting and retrying       |

- Amounts are decimal strings in DOGE (up to 8 decimals). Parse them as
  decimals, not floats. Times are unix seconds.
- Hashes, transaction ids and addresses are always printed in full.
- `dogechain schema` prints every command, its API endpoint and its output
  shape as JSON.

## Common tasks

```sh
dogechain find <anything> --json          # what is this? kind + value
dogechain address <address> --json        # balance + 10 latest transactions
dogechain address <address> --page 2 --json
dogechain tx <txid> --explain --json      # plain-English explanation only
dogechain tx <txid> --json                # full transaction, explanation, warnings
dogechain block latest --json
dogechain fees --json                     # what sending costs right now
dogechain richlist --page 1 --json        # top 100 holders
dogechain chart tx_count --interval day --json
dogechain labels <address> <address> --json
dogechain watch all --json                # one JSON event per line, runs until killed
dogechain watch txs --min 100000 --json   # only transactions moving 100,000+ DOGE
```

`watch` exits with code 4 if the stream drops. Run it again to reconnect;
the stream starts by resending the current state.

## Teach your agent

`dogechain skill install` installs the Dogechain skill for Claude Code and
Codex (`--project` for the current project only). `dogechain skill status`
shows whether it is current.

`dogechain mcp install` adds the Dogechain MCP server
(`https://dogechain.com/mcp`) to Claude Code and Codex; `--print` shows the
setup for other apps.

## Safety

The CLI refuses input that looks like a private key or recovery phrase (exit
2, `BAD_INPUT`) without sending it anywhere. Never pass one to it, or to any
website or API.

`tx` replies can carry `warnings`. A `lookalike_sender` warning means a
transaction likely comes from an address made to look like one the receiver
really uses (address poisoning). Surface it to the user, and never suggest
copying an address from transaction history.

## Limits

Dogechain.com allows about 120 requests a minute per IP address. When it asks
the CLI to slow down, the CLI waits (up to 30 seconds, 3 times) and retries
before exiting with code 5. Keep at most a few `watch` streams open at once.

# dogechain

The Dogecoin blockchain from your terminal, via [Dogechain.com](https://dogechain.com).

```console
$ dogechain block latest
$ dogechain tx <txid>
$ dogechain address <address>
$ dogechain fees
$ dogechain watch
```

Output is plain English by default. Add `--json` to any command for the
Dogechain.com API's reply, unchanged, for scripts and AI agents.

`dogechain` is read-only. It needs no account and no keys, and it never sends
transactions.

## Install

**Homebrew (macOS and Linux)**

```sh
brew install blockio/tap/dogechain
```

**npm (any platform with Node.js)**

```sh
npm install -g dogechain    # or run once with: npx dogechain
```

**macOS and Linux, without Homebrew**

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/BlockIo/dogechain-cli/releases/latest/download/dogechain-cli-installer.sh | sh
```

**Windows (PowerShell)**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/BlockIo/dogechain-cli/releases/latest/download/dogechain-cli-installer.ps1 | iex"
```

**From source** (Rust 1.88 or newer)

```sh
cargo install --git https://github.com/BlockIo/dogechain-cli --locked
```

Prebuilt binaries for each platform are attached to every
[release](https://github.com/BlockIo/dogechain-cli/releases).

## Commands

| Command | Shows |
|---|---|
| `find <query>` | Whatever a block height or hash, transaction id or address points to |
| `block <height\|hash\|latest>` | A block |
| `blocks` | The 10 newest blocks |
| `tx <txid> [--explain]` | A transaction: who paid whom, in plain English |
| `address <address> [--page N]` | Balance and history, 10 transactions per page (alias `addr`) |
| `labels <address>...` | Known labels, such as mining pools, for up to 100 addresses |
| `network` | The chain tip, the mempool and the price |
| `fees` | What sending DOGE costs right now |
| `supply` | How many DOGE exist and how fast the supply grows |
| `richlist [--page N]` | The top 1,000 addresses by balance, 100 per page (alias `top`) |
| `chart <series> [--interval I]` | A chart series as a table |
| `watch [blocks\|mempool\|txs\|all] [--min DOGE]` | Live events until interrupted |
| `schema` | Every command's API endpoint and output shape, as JSON |
| `guide` | A usage guide for scripts and AI agents |

Run `dogechain <command> --help` for details and examples.

## Scripts and AI agents

- `--json` (or `DOGECHAIN_JSON=1`) prints the API's reply on stdout:
  `{"status":"success","data":{...}}`. `watch --json` prints one
  `{"event":...,"data":...}` object per line.
- Errors go to stderr, as
  `{"status":"fail","data":{"error_message":"...","code":"..."}}` with `--json`.
- Amounts are decimal strings in DOGE; times are unix seconds.
- `dogechain schema` describes every command as JSON, and `dogechain guide`
  (also [AGENTS.md](AGENTS.md)) is a short guide for agents.

### Exit codes

| Exit | Code | Meaning |
|---|---|---|
| 0 | | Success |
| 1 | `OTHER` | Any other error |
| 2 | `BAD_INPUT` | Invalid arguments, or the API rejected the input |
| 3 | `NOT_FOUND` | No such block, transaction or address |
| 4 | `UNAVAILABLE` | Dogechain.com could not be reached or is busy; try again later |
| 5 | `RATE_LIMITED` | Still rate limited after waiting and retrying |

### Environment

| Variable | Effect |
|---|---|
| `DOGECHAIN_JSON` | Same as `--json` when set to anything but `0`, `false`, `no`, `off` or empty |
| `DOGECHAIN_TIMEOUT` | Same as `--timeout`: seconds before a request gives up (default 30) |

## Rate limits

Dogechain.com allows about 120 requests a minute per IP address. When asked to
slow down, `dogechain` waits as long as the server says (up to 30 seconds, 3
times) and retries, then exits with code 5.

## Versioning

`dogechain` follows [Semantic Versioning](https://semver.org). Command and flag
names, environment variables, exit codes, error codes and the `--json` output
are its public interface. See [CHANGELOG.md](CHANGELOG.md).

## Development

```sh
cargo test                                   # unit and end-to-end tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The end-to-end tests run the binary against a local mock server. They only run
in debug builds.

### Releasing

1. Update the version in `Cargo.toml` and move the `Unreleased` changelog
   entries under the new version.
2. Commit, then tag and push: `git tag v0.1.0 && git push origin v0.1.0`.
3. The release workflow builds every platform and publishes a GitHub release
   with installers.

## License

[MIT](LICENSE)

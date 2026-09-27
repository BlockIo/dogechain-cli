# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The public interface covered by semantic versioning is: command and flag
names, environment variables, exit codes, error codes, and the shape of
`--json` output (which is the Dogechain.com API's reply).

## [Unreleased]

### Added

- First release of `dogechain`, reading the Dogechain.com JSON API (/api/v3).
- Commands: `find`, `block`, `blocks`, `tx`, `address` (`addr`), `labels`,
  `network`, `fees`, `supply`, `richlist` (`top`), `chart`, `watch`, `schema`,
  `guide`.
- Plain-English output by default; `--json` / `DOGECHAIN_JSON` for the API's
  reply unchanged, with JSON errors on stderr.
- Exit codes: 0 ok, 1 other, 2 bad input, 3 not found, 4 unavailable,
  5 rate limited.
- Waits out rate limits (Retry-After, up to 30 s, 3 retries).
- Address-poisoning warnings shown first on transactions, and as they happen
  in `watch`.
- `watch --min` to show only transactions above an amount.
- Installers: Homebrew (`blockio/tap/dogechain`), shell script, PowerShell,
  and prebuilt binaries for macOS, Linux and Windows.

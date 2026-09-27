# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The public interface covered by semantic versioning is: command and flag
names, environment variables, exit codes, error codes, and the shape of
`--json` output (which is the Dogechain.com API's reply).

## [Unreleased]

## [0.4.0] - 2026-09-27

### Added

- `dogechain skill install`, `status` and `uninstall`: installs the Dogechain
  agent skill for Claude Code (`~/.claude/skills`) and Codex
  (`~/.agents/skills`), or into the current project with `--project`, or any
  folder with `--dir`. The skill is downloaded from dogechain.com and checked
  against its published SHA-256 digest before anything is written; copies
  changed since installation are kept unless `--force`.

## [0.3.1] - 2026-09-27

### Fixed

- `labels`, `address` and `richlist` no longer fail on an address whose pool
  labels conflict; it shows as "conflicting pool labels".
- `fees` no longer fails when the median or maximum fee rate, or the
  next-block estimate, is unavailable.

## [0.3.0] - 2026-09-27

### Added

- `address` and `richlist` mark coins paid to an address's public key
  ("paid to a public key").
- `transfers_sent` listed among the chart series in `chart --help` and
  `schema`.

## [0.2.0] - 2026-09-27

### Added

- `chart --last N`: text output shows the most recent 30 points by default
  (`--last 0` for all). `--json` still returns every point.

### Fixed

- `chart` printed no dates for daily, hourly and per-minute series, and failed
  for `pool_share`, `transfers` and `holder_share`.

### Changed

- Hashrate is shown with units (e.g. `2.84 PH/s`) and difficulty with
  thousands separators.
- `holder_share` is shown as percentages, and `transfers` as a count and DOGE
  amount per size bucket.

## [0.1.2] - 2026-09-27

### Changed

- Packaging updates. No changes to the `dogechain` command.

## [0.1.1] - 2026-09-27

### Security

- Text output now strips terminal control characters (such as escape
  sequences) from everything it prints, including API error messages, so
  data from the network cannot drive the terminal. `--json` output is
  unchanged.
- Release artifacts carry GitHub build provenance attestations; verify one
  with `gh attestation verify <file> -R BlockIo/dogechain-cli`.
- Release and CI workflows pin every third-party action to an exact commit.

## [0.1.0] - 2026-09-27

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
- Installers: Homebrew (`blockio/tap/dogechain`), npm (`dogechain`), shell script, PowerShell,
  and prebuilt binaries for macOS, Linux and Windows.

[Unreleased]: https://github.com/BlockIo/dogechain-cli/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/BlockIo/dogechain-cli/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/BlockIo/dogechain-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/BlockIo/dogechain-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/BlockIo/dogechain-cli/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/BlockIo/dogechain-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/BlockIo/dogechain-cli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/BlockIo/dogechain-cli/releases/tag/v0.1.0

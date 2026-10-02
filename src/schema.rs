//! `dogechain schema`: a machine-readable description of every command, for
//! tool-calling agents. Keep it in step with cli.rs and AGENTS.md.

use serde_json::{Value, json};

pub fn schema() -> Value {
    json!({
        "name": "dogechain",
        "version": env!("CARGO_PKG_VERSION"),
        "api": "https://dogechain.com/api/v3",
        "terms_of_service": "https://dogechain.com/terms",
        "output": {
            "text": "plain English on stdout (default)",
            "json": "--json or DOGECHAIN_JSON=1: the API's reply, unchanged, on stdout",
            "json_stream": "`watch --json`: one {\"event\": string, \"data\": object} per line",
        },
        "envelope": {
            "success": { "status": "success", "data": "<per command>" },
            "failure": {
                "status": "fail",
                "data": {
                    "error_message": "string",
                    "code": "BAD_INPUT | NOT_FOUND | UNAVAILABLE | RATE_LIMITED | OTHER",
                    "retry_after_secs": "integer, RATE_LIMITED only"
                }
            },
            "failure_stream": "stderr"
        },
        "exit_codes": {
            "0": "ok",
            "1": "other error (OTHER)",
            "2": "bad input: invalid arguments, or rejected by the API (BAD_INPUT)",
            "3": "not found (NOT_FOUND)",
            "4": "Dogechain.com unreachable or unavailable; retry later (UNAVAILABLE)",
            "5": "rate limited after waiting and retrying (RATE_LIMITED)"
        },
        "conventions": {
            "amounts": "decimal strings in DOGE with up to 8 decimal places, e.g. \"12400.5\"; never floats",
            "times": "unix seconds",
            "fee_rates": "koinu per byte (1 DOGE = 100,000,000 koinu)",
            "pagination": "--page N, starting at 1"
        },
        "global_flags": {
            "--json": "JSON output (env DOGECHAIN_JSON)",
            "--timeout <SECS>": "per-request timeout, default 30 (env DOGECHAIN_TIMEOUT)"
        },
        "commands": {
            "find <query>": {
                "does": "resolve a block height or hash, transaction id or address",
                "api": "GET /api/v3/find?q=<query>",
                "data": { "kind": "block | transaction | address", "value": "string or number" },
                "notes": "no match exits 3 (data.kind none, without value); text mode shows the matching block, transaction or address. Input that looks like a private key or recovery phrase is refused locally with exit 2 and never sent; this applies to every lookup command"
            },
            "block <height|hash|latest>": {
                "api": "GET /api/v3/block/<id>",
                "data": "height, hash, time, num_txs, size, weight, version, bits, nonce, difficulty, merkleroot, previous_block_hash, next_block_hash|null, confirmations, is_orphan, reward, fees, value_out, price|null, txs[txid]"
            },
            "blocks": {
                "api": "GET /api/v3/blocks/latest",
                "data": "blocks[{height, time, num_txs, size, weight|null, version, miner|null, difficulty, reward_and_fees, price|null}], newest first"
            },
            "tx <txid> [--explain]": {
                "api": "GET /api/v3/tx/<txid>",
                "data": {
                    "transaction": "hash, confirmations, block_hash, block_height|null, time, fee, size, vsize, locktime, inputs[{index, address|null, value, previous_output{hash,index}}], outputs[{index, address|null, value, is_change, spent}]",
                    "explain": "headline, detail, sent, change, fee, change_outputs[index]",
                    "warnings": "[{kind: \"lookalike_sender\", sender, imitates, receiver, shared_prefix, sender_kind, real_txid}]"
                },
                "notes": "--explain --json returns only the explain object as data"
            },
            "address <address> [--page N]": {
                "aliases": ["addr"],
                "api": "GET /api/v3/address/<address>?page=N",
                "data": "address, page, label|null, summary{confirmed_balance, unconfirmed_balance, confirmed_received, txs_received, txs_sent, txs_total}, transactions[{hash, block|null, time, value_received, value_sent, balance_change, price, p2pk?}]",
                "notes": "10 transactions per page, newest first; block null means unconfirmed; p2pk: true (absent otherwise) means paid to the address's public key"
            },
            "labels <address>...": {
                "api": "GET /api/v3/labels?a=<address>&a=...",
                "data": { "labels": "{<address>: {label, kind, source, evidence, conflict}}" },
                "notes": "up to 100 addresses; unlabelled addresses are absent"
            },
            "network": {
                "api": "GET /api/v3/network",
                "data": "info{block_count, best_block_hash, hashrate, mempool{mempool_txs, mempool_size, updated_at, blocks[{block_num, num_txs, size, min_fee_rate, median_fee_rate, max_fee_rate}]}}, network, price_usd|null"
            },
            "fees": {
                "api": "GET /api/v3/fees",
                "data": "fee_rate_koinu_per_byte{min, median, max}, simple_payment{bytes, min_fee}, median_fee_paid, median_fee_paid_blocks, mempool{txs, bytes, next_block_txs}, price_usd"
            },
            "supply": {
                "api": "GET /api/v3/supply",
                "data": "supply, height, per_block, per_year and inflation_next_12_months (fraction; both assume a block every 60 s), recent_pace{block_seconds, per_year, inflation_next_12_months} (at the past 365 days' actual pace; text output uses this)"
            },
            "richlist [--page N]": {
                "aliases": ["top"],
                "api": "GET /api/v3/richlist?page=N",
                "data": "height, time, supply, page, total_rows, rows[{rank, address, balance, share_of_supply, tx_count, received, sent, first_seen{height,time}, last_seen, delta_24h|null, delta_7d|null, label|null, p2pk?}]",
                "notes": "100 rows per page, pages 1 to 10"
            },
            "chart <series> [--interval I] [--last N]": {
                "api": "GET /api/v3/chart/<series>?interval=I",
                "series": {
                    "numbers": ["tx_count", "tx_per_min", "block_size", "block_interval", "fees_median", "fees_total", "hashrate", "difficulty", "supply", "price_usd", "active_addresses", "new_addresses", "lookalikes", "lookalike_senders"],
                    "objects": ["pool_share", "transfers", "transfers_sent", "holder_share"]
                },
                "intervals": ["minute", "hour", "day", "block", "week", "month"],
                "data": "series, interval, points[{t, label, v}]; v is a number, an object (for the object series) or null",
                "notes": "default interval day; --last limits text output to the most recent N points (default 30, 0 for all) and does not affect --json; point labels may be empty, so use t; minute only for tx_per_min; object series take day (some also hour); other series may be added"
            },
            "watch [blocks|mempool|txs|all] [--min DOGE]": {
                "api": "GET /api/v3/live (server-sent events)",
                "line": { "event": "tip | mempool | price | tx | tx_gone | lookalike | richlist", "data": "object, per event below" },
                "events": {
                    "tip": "height, hash, time, num_txs, miner|null, pool|null",
                    "mempool": "txs, bytes, min_fee_rate, median_fee_rate, max_fee_rate (koinu per byte), next_block_txs",
                    "price": "usd",
                    "tx": "txid, value_out, fee, size, n_in, n_out, from_address|null (first input), to_address|null (first output), time_seen",
                    "tx_gone": "txid, reason (mined | dropped | replaced; replaced means a fee bump or double-spend took its place; other values may be added)",
                    "lookalike": "txid, sender, imitates",
                    "richlist": "height, addresses[] (top-1,000 entries changed by this block)"
                },
                "notes": "blocks=tip, mempool=mempool, txs=tx, all=every event; the current state is sent first; --min keeps tx events with value_out at least DOGE (value_out totals all outputs, including change; for the amount actually paid use `tx <txid>`, data.explain.sent); on connect the stream first sends the current tip, mempool and price, and recent tx events; exits 4 when the stream drops, so rerun to reconnect"
            },
            "skill install [--project | --dir DIR] [--print] [--force]": {
                "does": "download the Dogechain agent skill (SKILL.md) from dogechain.com, verify its published sha256 digest, and install it for each agent found: ~/.claude/skills/dogechain (Claude Code), ~/.agents/skills/dogechain (Codex); --project uses .claude/skills and .agents/skills in the current directory; --print only prints it",
                "source": "https://dogechain.com/.well-known/agent-skills/index.json",
                "data": { "skill": { "name": "dogechain", "digest": "sha256:<hex>" }, "targets": "[{agent, path, result: installed | updated | unchanged | skipped: …}]" },
                "notes": "never overwrites a copy changed since it was installed unless --force; exits 1 if any copy was skipped"
            },
            "skill status [--project | --dir DIR]": {
                "data": { "published_digest": "sha256:<hex> | null (offline)", "targets": "[{agent, path, state: not_installed | up_to_date | outdated | modified}]" }
            },
            "skill uninstall [--project | --dir DIR] [--force]": {
                "data": { "targets": "[{agent, path, result: removed | not installed | skipped: …}]" }
            },
            "mcp install [--project] [--print] [--force]": {
                "does": "add the hosted Dogechain MCP server (https://dogechain.com/mcp, Streamable HTTP, no auth) to Claude Code and Codex using their own `mcp add` commands; --project uses Claude Code's project scope; --print shows setup for Claude Code, Codex, Cursor, VS Code, Gemini CLI, Claude.ai and ChatGPT without changing anything",
                "data": { "server": "https://dogechain.com/mcp", "agents": "[{agent: claude | codex, result}]" },
                "notes": "never replaces a server named dogechain that points elsewhere unless --force; exits 1 if any agent was skipped; exits 3 if neither agent is on PATH"
            },
            "mcp status": { "data": { "server": "https://dogechain.com/mcp", "agents": "[{agent, result}]" } },
            "mcp uninstall [--project] [--force]": { "data": { "agents": "[{agent, result}]" } },
            "schema": { "does": "print this description" },
            "guide": { "does": "print the usage guide for scripts and AI agents" }
        }
    })
}

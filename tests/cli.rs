//! End-to-end tests: the real binary against a local mock of the API.
//!
//! The binary only accepts a different API base in debug builds, so these
//! tests are skipped under `--release`.
#![cfg(debug_assertions)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};

const HASH: &str = "3f2a9c1d4e5b6a7980f1e2d3c4b5a6978877665544332211ffeeddccbbaa0099";
const TXID: &str = "9d0bc1f3e8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3b2c1";
const ADDR: &str = "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L";
const LOOKALIKE: &str = "DH5yaVbeSWk3uu4BbaH3JQnLDbJgZ5qNaG";

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn reply(status: u16, body: Value) -> Reply {
    Reply {
        status,
        headers: vec![],
        body: body.to_string(),
    }
}

fn ok(data: Value) -> Reply {
    reply(200, json!({ "status": "success", "data": data }))
}

fn fail(status: u16, message: &str) -> Reply {
    reply(
        status,
        json!({ "status": "fail", "data": { "error_message": message } }),
    )
}

/// Serves `replies` in order, one per request, and records each request path.
struct Mock {
    base: String,
    paths: Arc<Mutex<Vec<String>>>,
}

fn mock(replies: Vec<Reply>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let paths = Arc::new(Mutex::new(Vec::new()));
    let seen = paths.clone();
    thread::spawn(move || {
        for r in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .to_owned();
            seen.lock().unwrap().push(path);
            // Drain the headers.
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            let content_type = if r.body.starts_with("event:") || r.body.starts_with(':') {
                "text/event-stream"
            } else {
                "application/json"
            };
            let mut head = format!(
                "HTTP/1.1 {} X\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n",
                r.status,
                r.body.len()
            );
            for (k, v) in &r.headers {
                head += &format!("{k}: {v}\r\n");
            }
            let _ = stream.write_all(format!("{head}\r\n{}", r.body).as_bytes());
        }
    });
    Mock { base, paths }
}

fn dogechain(m: &Mock) -> Command {
    let mut cmd = Command::cargo_bin("dogechain").unwrap();
    cmd.env("DOGECHAIN_TEST_API_BASE", &m.base)
        .env_remove("DOGECHAIN_JSON")
        .env_remove("DOGECHAIN_TIMEOUT");
    cmd
}

fn block() -> Value {
    json!({
        "height": 5000000, "hash": HASH, "time": 1_700_000_000, "num_txs": 12, "size": 4321,
        "weight": 17284, "version": 6422788, "bits": "1a01b9c4", "nonce": 0,
        "difficulty": "25071243.12", "merkleroot": HASH, "previous_block_hash": HASH,
        "next_block_hash": null, "confirmations": 3, "is_orphan": false,
        "reward": "10000.00000000", "fees": "0.52000000", "value_out": "1234567.89000000",
        "price": null, "txs": [TXID]
    })
}

fn tx(warnings: Value) -> Value {
    json!({
        "transaction": {
            "hash": TXID, "confirmations": 0, "block_hash": null, "block_height": null,
            "time": 1_700_000_000, "fee": "0.01000000", "size": 226, "vsize": 226, "locktime": 0,
            "inputs_n": 1, "inputs_value": "100.01000000", "outputs_n": 2, "outputs_value": "100.00000000",
            "price": null,
            "inputs": [{ "index": 0, "address": ADDR, "value": "100.01000000",
                         "previous_output": { "hash": TXID, "index": 1 } }],
            "outputs": [
                { "index": 0, "address": LOOKALIKE, "value": "60.00000000", "is_change": false, "spent": false },
                { "index": 1, "address": ADDR, "value": "40.00000000", "is_change": true, "spent": false }
            ]
        },
        "explain": {
            "headline": "Someone sent 60 DOGE.",
            "detail": "40 DOGE went back to the sender as change.",
            "sent": "60.00000000", "change": "40.00000000", "fee": "0.01000000",
            "change_outputs": [1]
        },
        "warnings": warnings
    })
}

#[test]
fn block_text_output_shows_full_hashes() {
    let m = mock(vec![ok(block())]);
    dogechain(&m)
        .args(["block", "5000000"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Block 5,000,000 · 12 transactions",
        ))
        .stdout(predicate::str::contains(format!("Hash {HASH}")))
        .stdout(predicate::str::contains("1,234,567.89 DOGE moved"))
        .stdout(predicate::str::contains("Next block not mined yet"));
    assert_eq!(*m.paths.lock().unwrap(), ["/api/v3/block/5000000"]);
}

#[test]
fn json_output_is_the_api_reply_unchanged() {
    let m = mock(vec![ok(block())]);
    let out = dogechain(&m)
        .args(["block", "latest", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v, json!({ "status": "success", "data": block() }));
    assert!(out.stderr.is_empty());
}

#[test]
fn json_env_var_turns_on_json() {
    let m = mock(vec![ok(block())]);
    let out = dogechain(&m)
        .env("DOGECHAIN_JSON", "1")
        .args(["block", "latest"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "success");
}

#[test]
fn user_input_cannot_escape_the_api_path() {
    let m = mock(vec![fail(404, "no such block")]);
    dogechain(&m)
        .args(["block", "../../x?y=1"])
        .assert()
        .code(3);
    let paths = m.paths.lock().unwrap();
    assert!(paths[0].starts_with("/api/v3/block/"), "{paths:?}");
    assert!(!paths[0].contains('?'), "{paths:?}");
}

#[test]
fn not_found_exits_3() {
    let m = mock(vec![fail(404, "no such transaction")]);
    dogechain(&m)
        .args(["tx", TXID])
        .assert()
        .code(3)
        .stdout("")
        .stderr("dogechain: no such transaction\n");
}

#[test]
fn errors_are_json_on_stderr_with_json() {
    let m = mock(vec![fail(404, "no such transaction")]);
    let out = dogechain(&m).args(["tx", TXID, "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    let v: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(
        v,
        json!({ "status": "fail", "data": { "error_message": "no such transaction", "code": "NOT_FOUND" } })
    );
}

#[test]
fn bad_input_exits_2() {
    let m = mock(vec![fail(400, "not a valid transaction id")]);
    dogechain(&m)
        .args(["tx", "zz"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not a valid"));
}

#[test]
fn unavailable_exits_4() {
    let m = mock(vec![fail(503, "busy, try again shortly")]);
    dogechain(&m).arg("network").assert().code(4);
    let m = mock(vec![reply(502, json!("bad gateway"))]);
    dogechain(&m)
        .arg("network")
        .assert()
        .code(4)
        .stderr(predicate::str::contains("unavailable right now (HTTP 502)"));
}

#[test]
fn unreachable_exits_4() {
    // Bind then drop, so nothing is listening on the port.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    Command::cargo_bin("dogechain")
        .unwrap()
        .env(
            "DOGECHAIN_TEST_API_BASE",
            format!("http://127.0.0.1:{port}"),
        )
        .arg("fees")
        .assert()
        .code(4)
        .stderr(predicate::str::contains("could not reach dogechain.com"));
}

fn limited() -> Reply {
    Reply {
        status: 429,
        headers: vec![("retry-after", "0".into())],
        body: json!({ "status": "fail", "data": { "error_message": "too many requests" } })
            .to_string(),
    }
}

#[test]
fn waits_out_a_rate_limit_then_succeeds() {
    let m = mock(vec![limited(), ok(block())]);
    dogechain(&m)
        .args(["block", "latest"])
        .assert()
        .success()
        .stderr(predicate::str::contains("rate limited; retrying"));
    assert_eq!(m.paths.lock().unwrap().len(), 2);
}

#[test]
fn gives_up_on_rate_limits_with_exit_5() {
    let m = mock(vec![limited(), limited(), limited(), limited()]);
    let out = dogechain(&m)
        .args(["block", "latest", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
    let last = out
        .stderr
        .split(|b| *b == b'\n')
        .rfind(|l| l.starts_with(b"{"))
        .unwrap();
    let v: Value = serde_json::from_slice(last).unwrap();
    assert_eq!(v["data"]["code"], "RATE_LIMITED");
    assert_eq!(v["data"]["retry_after_secs"], 0);
}

#[test]
fn does_not_wait_out_long_rate_limits() {
    let long = Reply {
        headers: vec![("retry-after", "3600".into())],
        ..limited()
    };
    dogechain(&mock(vec![long])).arg("supply").assert().code(5);
}

#[test]
fn tx_shows_poisoning_warning_first() {
    let warning = json!([{
        "kind": "lookalike_sender", "sender": LOOKALIKE, "imitates": ADDR, "receiver": ADDR,
        "shared_prefix": 5, "sender_kind": "new", "real_txid": TXID
    }]);
    let m = mock(vec![ok(tx(warning))]);
    let out = dogechain(&m).args(["tx", TXID]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with("WARNING: likely address poisoning."),
        "{text}"
    );
    assert!(text.contains(&format!("The sender {LOOKALIKE} imitates {ADDR}")));
    assert!(text.contains("How the trick works: https://dogechain.com/guides/copycat"));
    assert!(text.contains("waiting in the mempool"));
    assert!(text.contains(&format!(
        "{ADDR}  40 DOGE  (likely change back to the sender)"
    )));
    assert!(text.contains(&format!("Transaction id {TXID}")));
}

#[test]
fn tx_explain_json_is_only_the_explanation() {
    let m = mock(vec![ok(tx(json!([])))]);
    let out = dogechain(&m)
        .args(["tx", TXID, "--explain", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "success");
    assert_eq!(v["data"]["headline"], "Someone sent 60 DOGE.");
    assert!(v["data"].get("transaction").is_none());
}

#[test]
fn find_shows_what_it_found() {
    let address = json!({
        "address": ADDR, "page": 1, "label": null,
        "summary": { "confirmed_balance": "1500.25000000", "unconfirmed_balance": "0.00000000",
                     "confirmed_received": "90000.00000000", "txs_received": 20, "txs_sent": 5, "txs_total": 25 },
        "transactions": [
            { "hash": TXID, "block": null, "time": 1_700_000_000, "value_received": "0.00000000",
              "value_sent": "60.01000000", "balance_change": "-60.01000000", "price": null }
        ]
    });
    let m = mock(vec![
        ok(json!({ "kind": "address", "value": ADDR })),
        ok(address),
    ]);
    dogechain(&m)
        .args(["find", ADDR])
        .assert()
        .success()
        .stdout(predicate::str::contains("Balance 1,500.25 DOGE"))
        .stdout(predicate::str::contains(format!(
            "-60.01 DOGE  in the mempool  {TXID}"
        )))
        .stdout(predicate::str::contains("History, page 1 of 3"))
        .stdout(predicate::str::contains(format!(
            "More: dogechain address {ADDR} --page 2"
        )));
    let paths = m.paths.lock().unwrap();
    assert_eq!(paths[0], format!("/api/v3/find?q={ADDR}"));
    assert_eq!(paths[1], format!("/api/v3/address/{ADDR}"));
}

#[test]
fn find_with_no_match_exits_3() {
    let m = mock(vec![ok(json!({ "kind": "none", "value": "nope" }))]);
    dogechain(&m).args(["find", "nope"]).assert().code(3);
}

#[test]
fn fees_in_plain_english() {
    let fees = json!({
        "fee_rate_koinu_per_byte": { "min": 1000.0, "median": 1500.0, "max": 50000.0 },
        "simple_payment": { "bytes": 226, "min_fee": "0.00226000" },
        "median_fee_paid": "0.01130000", "median_fee_paid_blocks": 100,
        "mempool": { "txs": 42, "bytes": 12000, "next_block_txs": 40 },
        "price_usd": 0.1
    });
    dogechain(&mock(vec![ok(fees)]))
        .arg("fees")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A simple payment (226 bytes) needs at least 0.00226 DOGE in fees ($0.0002).",
        ))
        .stdout(predicate::str::contains("min 0.01, median 0.015, max 0.5"));
}

#[test]
fn labels_sends_every_address() {
    let m = mock(vec![ok(
        json!({ "labels": { ADDR: { "label": "Example Pool", "kind": "mining_pool" } } }),
    )]);
    dogechain(&m)
        .args(["labels", ADDR, LOOKALIKE])
        .assert()
        .success()
        .stdout(format!(
            "{ADDR}  Example Pool (mining pool)\n{LOOKALIKE}  no label\n"
        ));
    assert_eq!(
        m.paths.lock().unwrap()[0],
        format!("/api/v3/labels?a={ADDR}&a={LOOKALIKE}")
    );
}

#[test]
fn watch_streams_one_json_object_per_line() {
    let stream = "event: tip\ndata: {\"height\":5000001}\n\n\
                  : keep-alive\n\n\
                  event: price\ndata: {\"usd\":0.1}\n\n\
                  event: tip\ndata: {\"height\":5000002}\n\n";
    let m = mock(vec![Reply {
        status: 200,
        headers: vec![],
        body: stream.into(),
    }]);
    let out = dogechain(&m)
        .args(["watch", "blocks", "--json"])
        .output()
        .unwrap();
    // The mock closes the stream, which the CLI reports as unavailable.
    assert_eq!(out.status.code(), Some(4));
    let lines: Vec<Value> = out
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect();
    assert_eq!(
        lines,
        [
            json!({ "event": "tip", "data": { "height": 5000001 } }),
            json!({ "event": "tip", "data": { "height": 5000002 } }),
        ]
    );
    assert_eq!(m.paths.lock().unwrap()[0], "/api/v3/live");
}

#[test]
fn usage_errors_exit_2_as_json_with_json() {
    let out = Command::cargo_bin("dogechain")
        .unwrap()
        .args(["address", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["data"]["code"], "BAD_INPUT");
}

#[test]
fn help_and_version_exit_0() {
    Command::cargo_bin("dogechain")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    Command::cargo_bin("dogechain")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("dogechain {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn schema_is_json_and_guide_is_text() {
    let out = Command::cargo_bin("dogechain")
        .unwrap()
        .arg("schema")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["exit_codes"]["5"],
        "rate limited after waiting and retrying (RATE_LIMITED)"
    );
    Command::cargo_bin("dogechain")
        .unwrap()
        .arg("guide")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Using `dogechain` from scripts and AI agents",
        ));
}

#[test]
fn blocks_lists_summaries() {
    let blocks = json!({ "blocks": [{
        "height": 6391032, "time": 1_790_472_687, "num_txs": 3, "size": 1431, "weight": null,
        "version": 6422788, "miner": ADDR, "difficulty": "43088356.51902277",
        "reward_and_fees": "10000.17560000", "price": null
    }]});
    dogechain(&mock(vec![ok(blocks)]))
        .arg("blocks")
        .assert()
        .success()
        .stdout(predicate::str::contains("Block 6,391,032 · "))
        .stdout(predicate::str::contains(format!(
            "3 transactions · 10,000.18 DOGE reward and fees · mined by {ADDR}"
        )));
}

#[test]
fn chart_shows_numbers_and_breakdowns() {
    let chart = json!({ "series": "pool_share", "interval": "day", "points": [
        { "t": 1, "label": "2026-09-25", "v": { "F2Pool": 46, "unknown": 28 } },
        { "t": 2, "label": "2026-09-26", "v": null }
    ]});
    let m = mock(vec![ok(chart)]);
    dogechain(&m)
        .args(["chart", "pool_share"])
        .assert()
        .success()
        .stdout("pool_share by day\n  2026-09-25  F2Pool 46, unknown 28\n  2026-09-26  no data\n");
    assert_eq!(
        m.paths.lock().unwrap()[0],
        "/api/v3/chart/pool_share?interval=day"
    );
}

#[test]
fn watch_min_filters_small_transactions() {
    let stream = format!(
        "event: tx\ndata: {{\"txid\":\"small\",\"value_out\":\"5.00000000\",\"n_out\":1}}\n\n\
         event: tx\ndata: {{\"txid\":\"{TXID}\",\"value_out\":\"250000.00000000\",\
         \"from_address\":\"{ADDR}\",\"to_address\":\"{LOOKALIKE}\",\"n_out\":2}}\n\n"
    );
    let m = mock(vec![Reply {
        status: 200,
        headers: vec![],
        body: stream,
    }]);
    dogechain(&m)
        .args(["watch", "txs", "--min", "100000"])
        .assert()
        .code(4)
        .stdout(format!(
            "New transaction: 250,000 DOGE in outputs, including change, from {ADDR} to {LOOKALIKE} and 1 more output · {TXID}\n"
        ));
}

#[test]
fn watch_min_rejects_non_amounts() {
    Command::cargo_bin("dogechain")
        .unwrap()
        .args(["watch", "txs", "--min", "lots"])
        .assert()
        .code(2);
}

#[test]
fn non_api_replies_count_as_unavailable() {
    for status in [200, 404] {
        let page = Reply {
            status,
            headers: vec![],
            body: "<html>coming soon</html>".into(),
        };
        dogechain(&mock(vec![page]))
            .arg("network")
            .assert()
            .code(4)
            .stderr(predicate::str::contains("did not return API data"));
    }
}

#[test]
fn text_output_strips_terminal_control_characters() {
    let labels = json!({ "labels": { ADDR: { "label": "\u{1b}]0;pwned\u{7}\u{1b}[2JPool", "kind": null } } });
    dogechain(&mock(vec![ok(labels.clone())]))
        .args(["labels", ADDR])
        .assert()
        .success()
        .stdout(format!("{ADDR}  ]0;pwned[2JPool\n"));
    // JSON output keeps the data exactly, escaped by the JSON encoder.
    let out = dogechain(&mock(vec![ok(labels)]))
        .args(["labels", ADDR, "--json"])
        .output()
        .unwrap();
    assert!(!out.stdout.contains(&0x1b));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["data"]["labels"][ADDR]["label"],
        "\u{1b}]0;pwned\u{7}\u{1b}[2JPool"
    );
    // Error messages from the API are cleaned too.
    dogechain(&mock(vec![fail(404, "gone\u{1b}[31m")]))
        .args(["tx", TXID])
        .assert()
        .code(3)
        .stderr("dogechain: gone[31m\n");
}

#[test]
fn chart_labels_come_from_t_when_missing_and_last_limits_text() {
    let points: Vec<Value> = (0..40)
        .map(|i| json!({ "label": "", "t": 1_790_380_800 - 86_400 * (39 - i), "v": i }))
        .collect();
    let chart = json!({ "series": "tx_count", "interval": "day", "points": points });
    let out = dogechain(&mock(vec![ok(chart.clone())]))
        .args(["chart", "tx_count", "--last", "2"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "tx_count by day, last 2 of 40 points (--last 0 for all)\n  2026-09-25  38\n  2026-09-26  39\n"
    );
    // --json is the API reply, every point included.
    let out = dogechain(&mock(vec![ok(chart)]))
        .args(["chart", "tx_count", "--last", "2", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["points"].as_array().unwrap().len(), 40);

    let holders = json!({ "series": "holder_share", "interval": "day",
        "points": [{ "t": 1_790_467_200, "v": { "top10": 0.44522163 } }] });
    dogechain(&mock(vec![ok(holders)]))
        .args(["chart", "holder_share"])
        .assert()
        .success()
        .stdout("holder_share by day\n  2026-09-27  top10 44.52%\n");
}

#[test]
fn marks_coins_paid_to_a_public_key() {
    let address = json!({
        "address": ADDR, "page": 1, "label": null,
        "summary": { "confirmed_balance": "10000.00000000", "unconfirmed_balance": "0.00000000",
                     "confirmed_received": "10000.00000000", "txs_received": 1, "txs_sent": 0, "txs_total": 1 },
        "transactions": [
            { "hash": TXID, "block": 1000, "time": 1_390_000_000, "value_received": "10000.00000000",
              "value_sent": "0.00000000", "balance_change": "10000.00000000", "price": null, "p2pk": true }
        ]
    });
    dogechain(&mock(vec![ok(address)]))
        .args(["address", ADDR])
        .assert()
        .success()
        .stdout(predicate::str::contains("+10,000 DOGE  "))
        .stdout(predicate::str::contains(format!(
            "{TXID}  (paid to a public key)\n"
        )));
}

#[test]
fn handles_null_fields_the_api_allows() {
    let labels = json!({ "labels": { ADDR: {
        "address": ADDR, "label": null, "kind": "mining_pool", "source": null,
        "evidence": null, "conflict": true } } });
    dogechain(&mock(vec![ok(labels)]))
        .args(["labels", ADDR])
        .assert()
        .success()
        .stdout(format!("{ADDR}  conflicting pool labels (mining pool)\n"));

    let fees = json!({
        "fee_rate_koinu_per_byte": { "min": 1000.0, "median": null, "max": null },
        "simple_payment": { "bytes": 226, "min_fee": "0.00226000" },
        "median_fee_paid": null, "median_fee_paid_blocks": 100,
        "mempool": { "txs": 0, "bytes": 0, "next_block_txs": null },
        "price_usd": null
    });
    dogechain(&mock(vec![ok(fees)]))
        .arg("fees")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "min 0.01, median unknown, max unknown.",
        ))
        .stdout(predicate::str::contains(
            "0 transactions waiting in the mempool (0 bytes).\n",
        ));
}

const SKILL_MD: &str = "---\nname: dogechain\ndescription: Look up Dogecoin.\n---\n# Dogechain\n";

fn sha256_hex(bytes: &[u8]) -> String {
    // Computed with the same crate the binary uses.
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn skill_index(url: &str, digest: &str) -> Reply {
    reply(
        200,
        json!({ "skills": [
            { "name": "other", "type": "skill-md", "url": "/x", "digest": "sha256:00" },
            { "name": "dogechain", "type": "skill-md", "url": url, "digest": digest,
              "description": "Look up Dogecoin." }
        ]}),
    )
}

fn skill_file(body: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![],
        body: body.into(),
    }
}

fn good_skill() -> Vec<Reply> {
    vec![
        skill_index(
            "/.well-known/agent-skills/dogechain/SKILL.md",
            &format!("sha256:{}", sha256_hex(SKILL_MD.as_bytes())),
        ),
        skill_file(SKILL_MD),
    ]
}

#[test]
fn skill_install_status_and_uninstall() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let skill_md = dir.path().join("dogechain/SKILL.md");

    let m = mock(good_skill());
    dogechain(&m)
        .args(["skill", "install", "--dir", root])
        .assert()
        .success()
        .stdout(predicate::str::contains("Folder: installed"));
    assert_eq!(std::fs::read_to_string(&skill_md).unwrap(), SKILL_MD);
    assert_eq!(
        *m.paths.lock().unwrap(),
        [
            "/.well-known/agent-skills/index.json",
            "/.well-known/agent-skills/dogechain/SKILL.md"
        ]
    );

    // Installing again changes nothing.
    dogechain(&mock(good_skill()))
        .args(["skill", "install", "--dir", root])
        .assert()
        .success()
        .stdout(predicate::str::contains("Folder: unchanged"));

    let out = dogechain(&mock(good_skill()))
        .args(["skill", "status", "--dir", root, "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["targets"][0]["state"], "up_to_date");

    // A local edit is never overwritten or removed without --force.
    std::fs::write(&skill_md, "my notes").unwrap();
    dogechain(&mock(good_skill()))
        .args(["skill", "install", "--dir", root])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("skipped: changed since installed"));
    assert_eq!(std::fs::read_to_string(&skill_md).unwrap(), "my notes");
    dogechain(&mock(vec![]))
        .args(["skill", "uninstall", "--dir", root])
        .assert()
        .code(1);
    assert!(skill_md.exists());
    dogechain(&mock(good_skill()))
        .args(["skill", "install", "--dir", root, "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Folder: updated"));
    assert_eq!(std::fs::read_to_string(&skill_md).unwrap(), SKILL_MD);

    dogechain(&mock(vec![]))
        .args(["skill", "uninstall", "--dir", root])
        .assert()
        .success()
        .stdout(predicate::str::contains("Folder: removed"));
    assert!(!dir.path().join("dogechain").exists());
}

#[test]
fn skill_install_finds_agents_in_home() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".claude")).unwrap();
    std::fs::create_dir(home.path().join(".codex")).unwrap();
    dogechain(&mock(good_skill()))
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .args(["skill", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude Code: installed"))
        .stdout(predicate::str::contains("Codex: installed"));
    assert!(
        home.path()
            .join(".claude/skills/dogechain/SKILL.md")
            .exists()
    );
    assert!(
        home.path()
            .join(".agents/skills/dogechain/SKILL.md")
            .exists()
    );
}

#[test]
fn skill_install_refuses_bad_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    // Digest mismatch: nothing is written.
    let tampered = vec![
        skill_index(
            "/.well-known/agent-skills/dogechain/SKILL.md",
            "sha256:0000",
        ),
        skill_file(SKILL_MD),
    ];
    dogechain(&mock(tampered))
        .args(["skill", "install", "--dir", root])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "does not match its published digest",
        ));
    assert!(!dir.path().join("dogechain").exists());
    // A skill hosted anywhere but dogechain.com is refused before fetching.
    let elsewhere = vec![skill_index("https://example.com/SKILL.md", "sha256:00")];
    let m = mock(elsewhere);
    dogechain(&m)
        .args(["skill", "install", "--dir", root])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not a path on dogechain.com"));
    assert_eq!(m.paths.lock().unwrap().len(), 1);
    // --print writes nothing and prints the verified file.
    dogechain(&mock(good_skill()))
        .args(["skill", "install", "--print"])
        .assert()
        .success()
        .stdout(SKILL_MD);
}

/// Stand-ins for the `claude` and `codex` tools: they log their arguments and
/// keep the configured URL in a file, like the real ones keep their config.
#[cfg(unix)]
fn fake_agents(dir: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = r#"#!/bin/sh
echo "claude $*" >> "$FAKE_DIR/log"
state="$FAKE_DIR/claude-url"
case "$1 $2" in
  "mcp get") [ -f "$state" ] || { echo 'No MCP server named "dogechain".' >&2; exit 1; }
             printf 'dogechain:\n  Scope: User config\n  Type: http\n  URL: %s\n' "$(cat "$state")" ;;
  "mcp add") for a; do last="$a"; done; echo "$last" > "$state" ;;
  "mcp remove") rm -f "$state" ;;
esac
"#;
    let codex = r#"#!/bin/sh
echo "codex $*" >> "$FAKE_DIR/log"
state="$FAKE_DIR/codex-url"
case "$1 $2" in
  "mcp get") [ -f "$state" ] || { echo "Error: No MCP server named 'dogechain' found." >&2; exit 1; }
             printf '{"name":"dogechain","transport":{"type":"streamable_http","url":"%s"}}\n' "$(cat "$state")" ;;
  "mcp add") for a; do last="$a"; done; echo "$last" > "$state" ;;
  "mcp remove") rm -f "$state" ;;
esac
"#;
    for (name, body) in [("claude", claude), ("codex", codex)] {
        let path = bin.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

#[cfg(unix)]
fn with_agents(dir: &std::path::Path, bin: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("dogechain").unwrap();
    // The stand-ins need the system's `cat` and `rm`; `bin` comes first.
    cmd.env("PATH", format!("{}:/bin:/usr/bin", bin.display()))
        .env("FAKE_DIR", dir)
        .env_remove("DOGECHAIN_JSON");
    cmd
}

#[cfg(unix)]
#[test]
fn mcp_install_status_and_uninstall() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_agents(dir.path());
    let url = "https://dogechain.com/mcp";
    let log = || std::fs::read_to_string(dir.path().join("log")).unwrap_or_default();

    with_agents(dir.path(), &bin)
        .args(["mcp", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Claude Code: added\nCodex: added\n",
        ));
    assert!(log().contains(&format!(
        "claude mcp add --transport http --scope user dogechain {url}"
    )));
    assert!(log().contains(&format!("codex mcp add dogechain --url {url}")));

    with_agents(dir.path(), &bin)
        .args(["mcp", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Claude Code: already added\nCodex: already added\n",
        ));

    let out = with_agents(dir.path(), &bin)
        .args(["mcp", "status", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["agents"][0]["result"], format!("added ({url})"));

    with_agents(dir.path(), &bin)
        .args(["mcp", "uninstall"])
        .assert()
        .success()
        .stdout("Claude Code: removed\nCodex: removed\n");
    assert!(!dir.path().join("claude-url").exists());
}

#[cfg(unix)]
#[test]
fn mcp_install_keeps_a_different_dogechain_server() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_agents(dir.path());
    std::fs::write(dir.path().join("codex-url"), "https://example.com/mcp\n").unwrap();
    with_agents(dir.path(), &bin)
        .args(["mcp", "install"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "Codex: skipped: a server named dogechain already points to https://example.com/mcp",
        ));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("codex-url")).unwrap(),
        "https://example.com/mcp\n"
    );
    with_agents(dir.path(), &bin)
        .args(["mcp", "install", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Codex: replaced"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("codex-url"))
            .unwrap()
            .trim(),
        "https://dogechain.com/mcp"
    );
}

#[cfg(unix)]
#[test]
fn mcp_install_project_scope_and_missing_agents() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_agents(dir.path());
    with_agents(dir.path(), &bin)
        .args(["mcp", "install", "--project"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Claude Code: added\nCodex: skipped: no per-project setting",
        ));
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap();
    assert!(log.contains("claude mcp add --transport http --scope project dogechain"));
    assert!(!log.contains("codex mcp add"));

    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    with_agents(dir.path(), &empty)
        .args(["mcp", "install"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("dogechain mcp install --print"));
}

#[test]
fn mcp_print_changes_nothing() {
    let out = Command::cargo_bin("dogechain")
        .unwrap()
        .env("PATH", "")
        .args(["mcp", "install", "--print", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["server"], "https://dogechain.com/mcp");
    assert_eq!(
        v["data"]["codex"]["command"],
        "codex mcp add dogechain --url https://dogechain.com/mcp"
    );
}

#[test]
fn shows_dollar_values_at_the_time() {
    let mut priced = tx(json!([]));
    priced["transaction"]["price"] = json!({ "currency": "USD", "value": "0.00156501" });
    dogechain(&mock(vec![ok(priced)]))
        .args(["tx", TXID])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Worth about $0.09 at the time (1 DOGE = $0.001565)",
        ))
        .stdout(predicate::str::contains("Fee 0.01 DOGE (<$0.01)"));

    let mut b = block();
    b["price"] = json!({ "currency": "USD", "value": "0.09387710" });
    dogechain(&mock(vec![ok(b)]))
        .args(["block", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Price then $0.0939 per DOGE · reward worth about $938.77",
        ));
}

#[test]
fn never_sends_private_keys_or_recovery_phrases() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let wif = "6KbBFk8U7sxzfajBnm1JJWqLvdcpd97K9Sf5GPgP2z8A1nR4GZ7";
    for args in [
        vec!["find", phrase],
        vec!["address", wif],
        vec!["labels", ADDR, wif],
    ] {
        let m = mock(vec![]);
        dogechain(&m)
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains(
                "looks like a private key or recovery phrase",
            ));
        assert!(
            m.paths.lock().unwrap().is_empty(),
            "nothing may be sent: {args:?}"
        );
    }
    let m = mock(vec![]);
    let out = dogechain(&m)
        .args(["find", wif, "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["data"]["code"], "BAD_INPUT");
}

#[test]
fn supply_uses_the_recent_pace() {
    let supply = json!({
        "supply": "156155592865.81425251", "height": 6398309, "per_block": "10000.00000000",
        "per_year": "5259600000.00000000", "inflation_next_12_months": 0.0337,
        "recent_pace": { "block_seconds": 63.4, "per_year": "4977336801.00000000",
                         "inflation_next_12_months": 0.031874 }
    });
    dogechain(&mock(vec![ok(supply)]))
        .arg("supply")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "At the past year's pace (a block every 63.4 seconds), that is about 4,977,336,801 DOGE a year: 3.19% inflation",
        ));
}

#[test]
fn find_kind_none_without_value() {
    dogechain(&mock(vec![ok(json!({ "kind": "none" }))]))
        .args(["find", "0xabc"])
        .assert()
        .code(3);
}

#[test]
fn fees_lead_with_the_suggested_fee() {
    let fees = json!({
        "fee_rate_koinu_per_byte": { "min": 1000.0, "median": 1500.0, "max": 50000.0 },
        "simple_payment": { "bytes": 226, "min_fee": "0.00226452", "suggested_fee": "0.01000000" },
        "suggested_fee_rate_koinu_per_byte": 1250.0,
        "suggested_fee_basis": "the next block has room for everything waiting",
        "median_fee_paid": null, "median_fee_paid_blocks": null,
        "mempool": { "txs": 3, "bytes": 700, "next_block_txs": 3 },
        "price_usd": null
    });
    let out = dogechain(&mock(vec![ok(fees)]))
        .arg("fees")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.starts_with(
            "Suggested fee for a simple payment (226 bytes): 0.01 DOGE. Why: the next block has room for everything waiting.\nThe least it can pay: 0.00226452 DOGE.\n"
        ),
        "{text}"
    );
}

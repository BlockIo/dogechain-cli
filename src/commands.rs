//! Runs each command: one API call (two for `find`), then either the API's
//! JSON reply unchanged (`--json`) or plain-English text.
//!
//! House rules for text output: addresses, transaction ids and hashes are
//! always printed in full, never shortened; amounts are in DOGE.

use std::io::Write;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::amount::{Doge, group_thousands};
use crate::api::Api;
use crate::cli::{Cli, Command, SkillCommand, SkillLocation, WatchWhat};
use crate::error::{CliError, Result};
use crate::model::*;
use crate::skill::{self, Scope};
use crate::time::{ago, now, utc, utc_date, utc_minute};

const KOINU_PER_DOGE: f64 = 100_000_000.0;
const ADDRESS_PAGE_SIZE: u64 = 10;
const RICHLIST_PAGE_SIZE: u64 = 100;

pub fn run(cli: Cli, out: &mut dyn Write) -> Result<()> {
    let json = cli.json;
    let api = || Api::new(Duration::from_secs(cli.timeout));
    match cli.command {
        Command::Find { query } => find(&api()?, &query, json, out),
        Command::Block { id } => {
            let env = api()?.get(&["block", &id], &[])?;
            if json {
                return print_json(out, &env);
            }
            show_block(out, &data(&env)?)
        }
        Command::Blocks => {
            let env = api()?.get(&["blocks", "latest"], &[])?;
            if json {
                return print_json(out, &env);
            }
            show_blocks(out, &data(&env)?)
        }
        Command::Tx { txid, explain } => {
            let env = api()?.get(&["tx", &txid], &[])?;
            if json && explain {
                return print_json(
                    out,
                    &json!({ "status": "success", "data": env["data"]["explain"] }),
                );
            }
            if json {
                return print_json(out, &env);
            }
            show_tx(out, &data(&env)?, explain)
        }
        Command::Address { address, page } => {
            let env = api()?.get(&["address", &address], &[("page", page.to_string())])?;
            if json {
                return print_json(out, &env);
            }
            show_address(out, &data(&env)?)
        }
        Command::Labels { addresses } => {
            let query: Vec<(&str, String)> = addresses.iter().map(|a| ("a", a.clone())).collect();
            let env = api()?.get(&["labels"], &query)?;
            if json {
                return print_json(out, &env);
            }
            show_labels(out, &addresses, &data(&env)?)
        }
        Command::Network => {
            let env = api()?.get(&["network"], &[])?;
            if json {
                return print_json(out, &env);
            }
            show_network(out, &data(&env)?)
        }
        Command::Fees => {
            let env = api()?.get(&["fees"], &[])?;
            if json {
                return print_json(out, &env);
            }
            show_fees(out, &data(&env)?)
        }
        Command::Supply => {
            let env = api()?.get(&["supply"], &[])?;
            if json {
                return print_json(out, &env);
            }
            show_supply(out, &data(&env)?)
        }
        Command::Richlist { page } => {
            let env = api()?.get(&["richlist"], &[("page", page.to_string())])?;
            if json {
                return print_json(out, &env);
            }
            show_richlist(out, &data(&env)?)
        }
        Command::Chart {
            series,
            interval,
            last,
        } => {
            let env = api()?.get(
                &["chart", &series],
                &[("interval", interval.as_str().into())],
            )?;
            if json {
                return print_json(out, &env);
            }
            show_chart(out, &data(&env)?, last)
        }
        Command::Watch { what, min } => watch(&api()?, what, min, json, out),
        Command::Skill(cmd) => match cmd {
            SkillCommand::Install { print: true, .. } => {
                let s = skill::fetch(&api()?)?;
                if json {
                    let text = String::from_utf8_lossy(&s.bytes);
                    print_json(
                        out,
                        &json!({ "status": "success", "data": { "digest": s.digest, "content": text } }),
                    )
                } else {
                    Ok(out.write_all(&s.bytes)?)
                }
            }
            SkillCommand::Install {
                location, force, ..
            } => skill::install(&api()?, &scope(location), force, json, out),
            SkillCommand::Status { location } => {
                skill::status(&api()?, &scope(location), json, out)
            }
            SkillCommand::Uninstall { location, force } => {
                skill::uninstall(&scope(location), force, json, out)
            }
        },
        Command::Schema => print_json(out, &crate::schema::schema()),
        Command::Guide => Ok(out.write_all(crate::GUIDE.as_bytes())?),
    }
}

fn scope(location: SkillLocation) -> Scope {
    match (location.project, location.dir) {
        (_, Some(dir)) => Scope::Dir(dir),
        (true, None) => Scope::Project,
        (false, None) => Scope::User,
    }
}

fn data<T: DeserializeOwned>(envelope: &Value) -> Result<T> {
    serde_json::from_value(envelope["data"].clone())
        .map_err(|e| CliError::Other(format!("unexpected response from dogechain.com: {e}")))
}

fn print_json(out: &mut dyn Write, v: &Value) -> Result<()> {
    serde_json::to_writer_pretty(&mut *out, v)
        .map_err(|e| CliError::Other(format!("could not write output: {e}")))?;
    writeln!(out)?;
    Ok(())
}

fn find(api: &Api, query: &str, json: bool, out: &mut dyn Write) -> Result<()> {
    let env = api.get(&["find"], &[("q", query.to_owned())])?;
    let found: Found = data(&env)?;
    let segment = match found.kind.as_str() {
        "block" => "block",
        "transaction" => "tx",
        "address" => "address",
        _ => {
            return Err(CliError::NotFound(format!(
                "nothing on the Dogecoin blockchain matches {query:?}"
            )));
        }
    };
    if json {
        return print_json(out, &env);
    }
    let id = found.id();
    let env = api.get(&[segment, &id], &[])?;
    match segment {
        "block" => show_block(out, &data(&env)?),
        "tx" => show_tx(out, &data(&env)?, false),
        _ => show_address(out, &data(&env)?),
    }
}

fn show_block(out: &mut dyn Write, b: &Block) -> Result<()> {
    let t = now();
    writeln!(
        out,
        "Block {} · {} · {} bytes",
        group_thousands(b.height.into()),
        plural(b.num_txs, "transaction"),
        group_thousands(b.size.into())
    )?;
    if b.is_orphan {
        writeln!(out, "Orphaned: this block is not part of the main chain.")?;
    }
    writeln!(out, "Mined {} ({})", utc(b.time), ago(b.time, t))?;
    writeln!(
        out,
        "Reward {} DOGE plus {} DOGE in fees · {} DOGE moved",
        b.reward,
        b.fees,
        b.value_out.display(2)
    )?;
    writeln!(
        out,
        "Confirmations {}",
        group_thousands(b.confirmations.into())
    )?;
    writeln!(out, "Difficulty {}", format_difficulty(&b.difficulty))?;
    writeln!(out, "Hash {}", b.hash)?;
    if let Some(prev) = &b.previous_block_hash {
        writeln!(out, "Previous block {prev}")?;
    }
    match &b.next_block_hash {
        Some(next) => writeln!(out, "Next block {next}")?,
        None => writeln!(out, "Next block not mined yet")?,
    }
    Ok(())
}

fn show_blocks(out: &mut dyn Write, r: &BlocksReply) -> Result<()> {
    let t = now();
    for b in &r.blocks {
        let miner = b
            .miner
            .as_deref()
            .map(|m| format!(" · mined by {m}"))
            .unwrap_or_default();
        writeln!(
            out,
            "Block {} · {} · {} · {} DOGE reward and fees{miner}",
            group_thousands(b.height.into()),
            ago(b.time, t),
            plural(b.num_txs, "transaction"),
            b.reward_and_fees.display(2)
        )?;
    }
    Ok(())
}

fn show_tx(out: &mut dyn Write, r: &TxReply, explain_only: bool) -> Result<()> {
    // Safety warnings come first so they cannot be missed.
    for w in &r.warnings {
        show_warning(out, w)?;
    }
    writeln!(out, "{}", r.explain.headline)?;
    writeln!(out, "{}", r.explain.detail)?;
    if explain_only {
        return Ok(());
    }
    let tx = &r.transaction;
    let status = match tx.block_height {
        Some(h) if tx.confirmations > 0 => format!(
            "{} in block {}",
            plural(tx.confirmations as u64, "confirmation"),
            group_thousands(h.into())
        ),
        _ => "waiting in the mempool (unconfirmed)".into(),
    };
    writeln!(out, "Status {status} · {}", utc(tx.time))?;
    writeln!(out, "From:")?;
    for i in &tx.inputs {
        let who = i.address.as_deref().unwrap_or("newly mined coins");
        match i.value {
            Some(v) => writeln!(out, "  {who}  {v} DOGE")?,
            None => writeln!(out, "  {who}")?,
        }
    }
    writeln!(out, "To:")?;
    for o in &tx.outputs {
        let who = o.address.as_deref().unwrap_or("(no address)");
        let change = if r.explain.change_outputs.contains(&o.index) {
            "  (likely change back to the sender)"
        } else {
            ""
        };
        writeln!(out, "  {who}  {} DOGE{change}", o.value)?;
    }
    writeln!(
        out,
        "Fee {} DOGE · {} bytes",
        tx.fee,
        group_thousands(tx.size.into())
    )?;
    writeln!(out, "Transaction id {}", tx.hash)?;
    Ok(())
}

fn show_warning(out: &mut dyn Write, w: &Warning) -> Result<()> {
    match (w.kind.as_str(), &w.sender, &w.imitates) {
        ("lookalike_sender", Some(sender), Some(imitates)) => {
            let victim = w
                .receiver
                .as_deref()
                .map(|r| format!(" that {r} has really dealt with"))
                .unwrap_or_default();
            writeln!(
                out,
                "WARNING: likely address poisoning. The sender {sender} imitates {imitates}, \
                 an address{victim}. Never copy an address from transaction history."
            )?;
        }
        (kind, _, _) => writeln!(out, "WARNING: {kind}")?,
    }
    Ok(())
}

fn show_address(out: &mut dyn Write, r: &AddressReply) -> Result<()> {
    match &r.label {
        Some(l) => writeln!(out, "{} ({})", r.address, l.text())?,
        None => writeln!(out, "{}", r.address)?,
    }
    let s = &r.summary;
    writeln!(out, "Balance {} DOGE", s.confirmed_balance)?;
    if s.unconfirmed_balance != Doge(0) {
        writeln!(
            out,
            "Pending {} DOGE, not yet confirmed",
            s.unconfirmed_balance
        )?;
    }
    writeln!(
        out,
        "Received {} DOGE in total · {}",
        s.confirmed_received,
        plural(s.txs_total, "transaction")
    )?;
    if r.transactions.is_empty() {
        writeln!(out, "No transactions on page {}.", r.page)?;
        return Ok(());
    }
    let pages = s.txs_total.div_ceil(ADDRESS_PAGE_SIZE).max(1);
    writeln!(out, "History, page {} of {pages}, newest first:", r.page)?;
    let t = now();
    for h in &r.transactions {
        let when = match h.block {
            None => "in the mempool".to_owned(),
            Some(_) => ago(h.time, t),
        };
        let sign = if h.balance_change.is_negative() {
            "-"
        } else {
            "+"
        };
        let p2pk = if h.p2pk {
            "  (paid to a public key)"
        } else {
            ""
        };
        writeln!(
            out,
            "  {sign}{} DOGE  {when}  {}{p2pk}",
            h.balance_change.abs(),
            h.hash
        )?;
    }
    if u64::from(r.page) < pages {
        writeln!(
            out,
            "More: dogechain address {} --page {}",
            r.address,
            r.page + 1
        )?;
    }
    Ok(())
}

fn show_labels(out: &mut dyn Write, asked: &[String], r: &LabelsReply) -> Result<()> {
    for a in asked {
        match r.labels.get(a) {
            Some(Label { label, kind }) => {
                let name = label.as_deref().unwrap_or(CONFLICTING_LABELS);
                match kind {
                    Some(kind) => writeln!(out, "{a}  {name} ({})", kind.replace('_', " "))?,
                    None => writeln!(out, "{a}  {name}")?,
                }
            }
            None => writeln!(out, "{a}  no label")?,
        }
    }
    Ok(())
}

fn show_network(out: &mut dyn Write, r: &NetworkReply) -> Result<()> {
    let i = &r.info;
    writeln!(
        out,
        "Latest block {}",
        group_thousands(i.block_count.into())
    )?;
    writeln!(out, "Hash {}", i.best_block_hash)?;
    writeln!(
        out,
        "Mempool {} waiting, {} bytes",
        plural(i.mempool.mempool_txs, "transaction"),
        group_thousands(i.mempool.mempool_size.into())
    )?;
    if let Some(next) = i.mempool.blocks.first() {
        writeln!(
            out,
            "Next block about {}, median fee {} DOGE per kB",
            plural(next.num_txs, "transaction"),
            doge_per_kb(next.median_fee_rate)
        )?;
    }
    writeln!(out, "Hashrate {}", format_hashrate(&i.hashrate))?;
    if let Some(p) = r.price_usd {
        writeln!(out, "Price ${p:.4}")?;
    }
    Ok(())
}

fn show_fees(out: &mut dyn Write, r: &FeesReply) -> Result<()> {
    let usd = |d: Doge| {
        r.price_usd
            .map(|p| format!(" (${:.4})", d.0 as f64 / KOINU_PER_DOGE * p))
            .unwrap_or_default()
    };
    let sp = &r.simple_payment;
    writeln!(
        out,
        "A simple payment ({} bytes) needs at least {} DOGE in fees{}.",
        sp.bytes,
        sp.min_fee,
        usd(sp.min_fee)
    )?;
    if let Some(paid) = r.median_fee_paid {
        let over = r
            .median_fee_paid_blocks
            .map(|n| format!(", median over the last {}", plural(n, "block")))
            .unwrap_or_default();
        writeln!(
            out,
            "People recently paid {} DOGE{}{over}.",
            paid.display(4),
            usd(paid)
        )?;
    }
    let f = &r.fee_rate_koinu_per_byte;
    writeln!(
        out,
        "Next-block fee rates in DOGE per kB: min {}, median {}, max {}.",
        doge_per_kb(f.min),
        f.median.map(doge_per_kb).as_deref().unwrap_or("unknown"),
        f.max.map(doge_per_kb).as_deref().unwrap_or("unknown")
    )?;
    let next = r
        .mempool
        .next_block_txs
        .map(|n| format!("; the next block takes about {}", group_thousands(n.into())))
        .unwrap_or_default();
    writeln!(
        out,
        "{} waiting in the mempool ({} bytes){next}.",
        plural(r.mempool.txs, "transaction"),
        group_thousands(r.mempool.bytes.into()),
    )?;
    Ok(())
}

fn show_supply(out: &mut dyn Write, r: &SupplyReply) -> Result<()> {
    writeln!(
        out,
        "{} DOGE exist as of block {}.",
        r.supply.display(0),
        group_thousands(r.height.into())
    )?;
    writeln!(
        out,
        "Each block adds {} DOGE, about {} DOGE a year: {:.2}% inflation over the next 12 months.",
        r.per_block.display(0),
        r.per_year.display(0),
        r.inflation_next_12_months * 100.0
    )?;
    Ok(())
}

fn show_richlist(out: &mut dyn Write, r: &RichlistReply) -> Result<()> {
    let pages = r.total_rows.div_ceil(RICHLIST_PAGE_SIZE).max(1);
    writeln!(
        out,
        "Top Doges at block {}, page {} of {pages} · supply {} DOGE",
        group_thousands(r.height.into()),
        r.page,
        r.supply.display(0)
    )?;
    for row in &r.rows {
        let who = row.address.as_deref().unwrap_or("(non-standard script)");
        let label = row
            .label
            .as_ref()
            .map(|l| format!("  {}", l.text()))
            .unwrap_or_default();
        let p2pk = if row.p2pk {
            "  (paid to a public key)"
        } else {
            ""
        };
        writeln!(
            out,
            "{:>5}  {who}  {} DOGE  {:.4}%{label}{p2pk}",
            row.rank,
            row.balance.display(0),
            row.share_of_supply * 100.0
        )?;
    }
    Ok(())
}

fn show_chart(out: &mut dyn Write, r: &ChartReply, last: usize) -> Result<()> {
    let total = r.points.len();
    let shown = if last == 0 { total } else { last.min(total) };
    let points = &r.points[total - shown..];
    if shown < total {
        writeln!(
            out,
            "{} by {}, last {} of {} points (--last 0 for all)",
            r.series,
            r.interval,
            group_thousands(shown as i128),
            group_thousands(total as i128)
        )?;
    } else {
        writeln!(out, "{} by {}", r.series, r.interval)?;
    }
    let rows: Vec<(String, String)> = points
        .iter()
        .map(|p| {
            (
                point_label(p, &r.interval),
                format_chart_value(&r.series, &p.v),
            )
        })
        .collect();
    let width = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    for (label, v) in rows {
        writeln!(out, "  {label:<width$}  {v}")?;
    }
    Ok(())
}

/// The API's label, or a UTC date or time from `t` when it sends none.
fn point_label(p: &ChartPoint, interval: &str) -> String {
    if !p.label.is_empty() {
        return p.label.clone();
    }
    match interval {
        "minute" | "hour" => utc_minute(p.t),
        _ => utc_date(p.t),
    }
}

fn watch(
    api: &Api,
    what: WatchWhat,
    min: Option<Doge>,
    json: bool,
    out: &mut dyn Write,
) -> Result<()> {
    let wanted = |event: &str| match what {
        WatchWhat::Blocks => event == "tip",
        WatchWhat::Mempool => event == "mempool",
        WatchWhat::Txs => event == "tx",
        WatchWhat::All => true,
    };
    let mut result = Ok(());
    api.live(|event, payload| {
        if !wanted(event) {
            return true;
        }
        let data: Value = serde_json::from_str(payload).unwrap_or(Value::String(payload.into()));
        if let (Some(min), "tx") = (min, event) {
            let value = data["value_out"].as_str().and_then(Doge::parse);
            if value.is_none_or(|v| v < min) {
                return true;
            }
        }
        let written = if json {
            writeln!(out, "{}", json!({ "event": event, "data": data }))
        } else {
            writeln!(out, "{}", describe_event(event, &data))
        };
        // Flush per event so pipes see each line as it happens.
        match written.and_then(|()| out.flush()) {
            Ok(()) => true,
            Err(e) => {
                result = Err(e.into());
                false
            }
        }
    })?;
    result
}

/// One line of text for a live event. Unknown shapes fall back to compact JSON.
fn describe_event(event: &str, d: &Value) -> String {
    let text = |k: &str| d[k].as_str().map(str::to_owned);
    let num = |k: &str| d[k].as_u64();
    let doge = |k: &str| d[k].as_str().and_then(Doge::parse);
    let line = match event {
        "tip" => num("height").map(|h| {
            let mut s = format!("New block {}", group_thousands(h.into()));
            if let Some(n) = num("num_txs") {
                s += &format!(" · {}", plural(n, "transaction"));
            }
            match (text("pool"), text("miner")) {
                (Some(pool), Some(miner)) => s += &format!(" · mined by {pool} ({miner})"),
                (Some(who), None) | (None, Some(who)) => s += &format!(" · mined by {who}"),
                (None, None) => {}
            }
            if let Some(hash) = text("hash") {
                s += &format!(" · {hash}");
            }
            s
        }),
        "mempool" => num("txs").map(|txs| {
            let mut s = format!("Mempool {} waiting", plural(txs, "transaction"));
            if let Some(bytes) = num("bytes") {
                s += &format!(", {} bytes", group_thousands(bytes.into()));
            }
            if let Some(rate) = d["median_fee_rate"].as_f64() {
                s += &format!(" · median fee {} DOGE per kB", doge_per_kb(rate));
            }
            s
        }),
        "price" => d["usd"].as_f64().map(|p| format!("Price ${p:.4}")),
        "tx" => text("txid").map(|txid| {
            let value = doge("value_out")
                .map(|v| format!("{} DOGE", v.display(2)))
                .unwrap_or_else(|| "New transaction".into());
            let from = text("from_address").unwrap_or_else(|| "(no address)".into());
            let to = text("to_address").unwrap_or_else(|| "(no address)".into());
            let more = match num("n_out") {
                Some(2) => " and 1 more output".into(),
                Some(n) if n > 2 => {
                    format!(" and {} more outputs", group_thousands((n - 1).into()))
                }
                _ => String::new(),
            };
            format!("{value} from {from} to {to}{more} · {txid}")
        }),
        "tx_gone" => text("txid").map(|txid| match text("reason").as_deref() {
            Some("mined") => format!("Confirmed in a block {txid}"),
            Some("dropped") => format!("Dropped from the mempool {txid}"),
            _ => format!("Left the mempool {txid}"),
        }),
        "lookalike" => match (text("sender"), text("imitates")) {
            (Some(sender), Some(imitates)) => Some(format!(
                "WARNING: likely address poisoning. The sender {sender} imitates {imitates}{}",
                text("txid").map(|t| format!(" · {t}")).unwrap_or_default()
            )),
            _ => None,
        },
        "richlist" => d["addresses"].as_array().map(|a| {
            let at = num("height")
                .map(|h| format!(" at block {}", group_thousands(h.into())))
                .unwrap_or_default();
            format!(
                "Top Doges changed{at}: {}",
                plural(a.len() as u64, "address")
            )
        }),
        _ => None,
    };
    line.unwrap_or_else(|| format!("{event} {d}"))
}

/// Chart values are numbers for most series and objects for breakdowns:
/// pool_share (blocks per pool), holder_share (fractions of supply) and
/// transfers (count and DOGE sent per size bucket).
fn format_chart_value(series: &str, v: &Value) -> String {
    match v {
        Value::Null => "no data".into(),
        Value::Number(n) => n
            .as_f64()
            .map(format_number)
            .unwrap_or_else(|| n.to_string()),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| match (series, v) {
                ("holder_share", Value::Number(n)) => {
                    format!("{k} {:.2}%", n.as_f64().unwrap_or(0.0) * 100.0)
                }
                (_, Value::Object(o)) if o.contains_key("count") => {
                    let count = o["count"].as_f64().map(format_number).unwrap_or_default();
                    match o.get("sent").and_then(Value::as_str).and_then(Doge::parse) {
                        Some(sent) => format!("{k} {count} ({} DOGE)", sent.display(0)),
                        None => format!("{k} {count}"),
                    }
                }
                _ => format!("{k} {}", format_chart_value(series, v)),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Hashes per second with an SI prefix: `2.79 PH/s`. Falls back to the raw
/// text if it is not a number.
fn format_hashrate(raw: &str) -> String {
    let Ok(h) = raw.trim().parse::<f64>() else {
        return raw.to_owned();
    };
    const UNITS: [&str; 7] = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s"];
    let mut value = h;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.2} {}", UNITS[unit])
}

/// `47,952,738.33`. Falls back to the raw text if it is not a number.
fn format_difficulty(raw: &str) -> String {
    match raw.trim().parse::<f64>() {
        Ok(d) if d.is_finite() => {
            let cents = (d * 100.0).round() as i128;
            format!(
                "{}.{:02}",
                group_thousands(cents / 100),
                (cents % 100).abs()
            )
        }
        _ => raw.to_owned(),
    }
}

fn plural(n: u64, noun: &str) -> String {
    format!(
        "{} {noun}{}",
        group_thousands(n.into()),
        match (n, noun.ends_with('s')) {
            (1, _) => "",
            (_, true) => "es",
            (_, false) => "s",
        }
    )
}

/// Koinu per byte to DOGE per kB (1,000 bytes), the unit wallets show.
fn doge_per_kb(koinu_per_byte: f64) -> String {
    let doge = koinu_per_byte * 1000.0 / KOINU_PER_DOGE;
    Doge((doge * KOINU_PER_DOGE).round() as i128).display(4)
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e18 {
        group_thousands(v as i128)
    } else if v.abs() >= 1000.0 {
        group_thousands(v.round() as i128)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_hashrate_and_difficulty() {
        assert_eq!(format_hashrate("2789460872749466"), "2.79 PH/s");
        assert_eq!(format_hashrate("999"), "999.00 H/s");
        assert_eq!(format_hashrate("n/a"), "n/a");
        assert_eq!(format_difficulty("47952738.33432145"), "47,952,738.33");
        assert_eq!(format_difficulty("0.5"), "0.50");
    }

    #[test]
    fn formats_breakdown_series() {
        let holders = json!({"top10": 0.44522163, "top100": 0.66201739});
        assert_eq!(
            format_chart_value("holder_share", &holders),
            "top10 44.52%, top100 66.20%"
        );
        let transfers = json!({"lt_1": {"count": 11407, "sent": "227.928"}});
        assert_eq!(
            format_chart_value("transfers", &transfers),
            "lt_1 11,407 (228 DOGE)"
        );
        let pools = json!({"F2Pool": 446, "unknown": 243});
        assert_eq!(
            format_chart_value("pool_share", &pools),
            "F2Pool 446, unknown 243"
        );
    }

    #[test]
    fn converts_fee_rates() {
        // 1,000 koinu/byte = 1,000,000 koinu/kB = 0.01 DOGE/kB.
        assert_eq!(doge_per_kb(1000.0), "0.01");
        assert_eq!(doge_per_kb(0.0), "0");
    }

    #[test]
    fn formats_chart_numbers() {
        assert_eq!(format_number(1234.0), "1,234");
        assert_eq!(format_number(1234.6), "1,235");
        assert_eq!(format_number(0.12345), "0.1235");
        assert_eq!(format_number(2.5), "2.5");
    }

    #[test]
    fn describes_events() {
        assert_eq!(
            describe_event("price", &json!({"usd": 0.1})),
            "Price $0.1000"
        );
        assert_eq!(
            describe_event("surprise", &json!({"a": 1})),
            r#"surprise {"a":1}"#
        );
        assert_eq!(
            describe_event(
                "tx",
                &json!({"txid": "t", "value_out": "16106.82459707", "from_address": "A",
                        "to_address": null, "n_out": 3})
            ),
            "16,106.82 DOGE from A to (no address) and 2 more outputs · t"
        );
        assert_eq!(
            describe_event("richlist", &json!({"height": 7, "addresses": ["a", "b"]})),
            "Top Doges changed at block 7: 2 addresses"
        );
        assert_eq!(
            describe_event("tip", &json!({"height": 5000000, "num_txs": 1})),
            "New block 5,000,000 · 1 transaction"
        );
    }
}

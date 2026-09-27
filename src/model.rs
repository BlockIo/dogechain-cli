//! Response types for the /api/v3 endpoints the CLI renders as text.
//!
//! Only the fields the text output uses are declared; unknown fields are
//! ignored, so the API can add fields without breaking the CLI. `--json`
//! output does not go through these types at all: it is the API's own reply.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::amount::Doge;

#[derive(Debug, Deserialize)]
pub struct Block {
    pub height: u64,
    pub hash: String,
    pub time: i64,
    pub num_txs: u64,
    pub size: u64,
    pub confirmations: i64,
    #[serde(default)]
    pub is_orphan: bool,
    pub reward: Doge,
    pub fees: Doge,
    pub value_out: Doge,
    pub difficulty: String,
    pub previous_block_hash: Option<String>,
    pub next_block_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlocksReply {
    pub blocks: Vec<BlockSummary>,
}

#[derive(Debug, Deserialize)]
pub struct BlockSummary {
    pub height: u64,
    pub time: i64,
    pub num_txs: u64,
    pub miner: Option<String>,
    pub reward_and_fees: Doge,
}

#[derive(Debug, Deserialize)]
pub struct TxReply {
    pub transaction: Transaction,
    pub explain: Explain,
    #[serde(default)]
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub confirmations: i64,
    pub block_height: Option<u64>,
    pub time: i64,
    pub fee: Doge,
    pub size: u64,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
}

#[derive(Debug, Deserialize)]
pub struct Input {
    pub address: Option<String>,
    /// Absent for coinbase inputs (newly mined coins).
    pub value: Option<Doge>,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub index: u32,
    pub address: Option<String>,
    pub value: Doge,
}

#[derive(Debug, Deserialize)]
pub struct Explain {
    pub headline: String,
    pub detail: String,
    #[serde(default)]
    pub change_outputs: Vec<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Warning {
    pub kind: String,
    pub sender: Option<String>,
    pub imitates: Option<String>,
    pub receiver: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddressReply {
    pub address: String,
    pub page: u32,
    pub label: Option<LabelField>,
    pub summary: AddressSummary,
    pub transactions: Vec<AddressTx>,
}

#[derive(Debug, Deserialize)]
pub struct AddressSummary {
    pub confirmed_balance: Doge,
    pub unconfirmed_balance: Doge,
    pub confirmed_received: Doge,
    pub txs_total: u64,
}

#[derive(Debug, Deserialize)]
pub struct AddressTx {
    pub hash: String,
    /// None while the transaction is still in the mempool.
    pub block: Option<u64>,
    pub time: i64,
    pub balance_change: Doge,
}

#[derive(Debug, Deserialize)]
pub struct Label {
    pub label: String,
    pub kind: Option<String>,
}

/// A label given either as plain text or as a full label object.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LabelField {
    Text(String),
    Full(Label),
}

impl LabelField {
    pub fn text(&self) -> &str {
        match self {
            LabelField::Text(s) => s,
            LabelField::Full(l) => &l.label,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LabelsReply {
    pub labels: BTreeMap<String, Label>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkReply {
    pub info: NetworkInfo,
    pub price_usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkInfo {
    pub block_count: u64,
    pub best_block_hash: String,
    pub hashrate: String,
    pub mempool: Mempool,
}

#[derive(Debug, Deserialize)]
pub struct Mempool {
    pub mempool_txs: u64,
    pub mempool_size: u64,
    #[serde(default)]
    pub blocks: Vec<ProjectedBlock>,
}

/// A projected upcoming block. Fee rates are in koinu per byte.
#[derive(Debug, Deserialize)]
pub struct ProjectedBlock {
    pub num_txs: u64,
    pub median_fee_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct FeesReply {
    pub fee_rate_koinu_per_byte: FeeRates,
    pub simple_payment: SimplePayment,
    pub median_fee_paid: Option<Doge>,
    pub median_fee_paid_blocks: Option<u64>,
    pub mempool: FeesMempool,
    pub price_usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct FeeRates {
    pub min: f64,
    pub median: f64,
    pub max: f64,
}

#[derive(Debug, Deserialize)]
pub struct SimplePayment {
    pub bytes: u64,
    pub min_fee: Doge,
}

#[derive(Debug, Deserialize)]
pub struct FeesMempool {
    pub txs: u64,
    pub bytes: u64,
    pub next_block_txs: u64,
}

#[derive(Debug, Deserialize)]
pub struct SupplyReply {
    pub supply: Doge,
    pub height: u64,
    pub per_block: Doge,
    pub per_year: Doge,
    pub inflation_next_12_months: f64,
}

#[derive(Debug, Deserialize)]
pub struct RichlistReply {
    pub height: u64,
    pub supply: Doge,
    pub page: u32,
    pub total_rows: u64,
    pub rows: Vec<RichRow>,
}

#[derive(Debug, Deserialize)]
pub struct RichRow {
    pub rank: u64,
    pub address: Option<String>,
    pub balance: Doge,
    pub share_of_supply: f64,
    pub label: Option<LabelField>,
}

#[derive(Debug, Deserialize)]
pub struct ChartReply {
    pub series: String,
    pub interval: String,
    pub points: Vec<ChartPoint>,
}

#[derive(Debug, Deserialize)]
pub struct ChartPoint {
    pub label: String,
    /// A number for most series, an object for breakdowns (e.g. pool_share),
    /// null where there is no data.
    #[serde(default)]
    pub v: serde_json::Value,
}

/// `/find`: `kind` is `block`, `transaction`, `address` or `none`.
#[derive(Debug, Deserialize)]
pub struct Found {
    pub kind: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

impl Found {
    /// The value as a path segment: a block height arrives as a number.
    pub fn id(&self) -> String {
        match &self.value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }
}

//! Command-line surface. Command names, flag names and environment variable
//! names are public interface: renaming or removing any of them is a
//! breaking change.

use clap::builder::FalseyValueParser;

use crate::amount::Doge;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "dogechain",
    version,
    about = "The Dogecoin blockchain from your terminal, via Dogechain.com",
    long_about = "The Dogecoin blockchain from your terminal, via Dogechain.com.\n\n\
        Output is plain English by default. Pass --json (or set DOGECHAIN_JSON=1) for the \
        API's JSON reply, unchanged; errors are then JSON on stderr.\n\n\
        Exit codes: 0 ok, 1 other error, 2 bad input, 3 not found, 4 Dogechain.com \
        unavailable, 5 rate limited.",
    after_help = "Examples:\n  \
        dogechain find 5000000\n  \
        dogechain tx <txid> --explain\n  \
        dogechain address <address> --json\n  \
        dogechain watch txs --min 100000\n\n\
        Run `dogechain guide` for a guide aimed at scripts and AI agents.\n\n\
        Data comes from Dogechain.com and is subject to its Terms of Service: https://dogechain.com/terms"
)]
pub struct Cli {
    /// Print the API's JSON reply instead of text (errors as JSON on stderr)
    #[arg(
        long,
        global = true,
        env = "DOGECHAIN_JSON",
        action = ArgAction::SetTrue,
        value_parser = FalseyValueParser::new()
    )]
    pub json: bool,

    /// Give up on a request after this many seconds
    #[arg(
        long,
        global = true,
        env = "DOGECHAIN_TIMEOUT",
        default_value_t = 30,
        value_name = "SECS"
    )]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Look up a block height, block hash, transaction id or address and show it
    #[command(after_help = "Examples:\n  dogechain find 5000000\n  dogechain find <address>")]
    Find {
        /// A block height or hash, a transaction id, or an address
        query: String,
    },

    /// Show a block
    #[command(after_help = "Examples:\n  dogechain block latest\n  dogechain block 5000000")]
    Block {
        /// Block height, block hash, or `latest`
        id: String,
    },

    /// List the 10 newest blocks
    Blocks,

    /// Show a transaction: who paid whom, in plain English
    #[command(
        after_help = "Examples:\n  dogechain tx <txid>\n  dogechain tx <txid> --explain --json"
    )]
    Tx {
        /// Transaction id (64 hex characters)
        txid: String,

        /// Only the plain-English explanation
        #[arg(long)]
        explain: bool,
    },

    /// Show an address: balance and history, newest first, 10 per page
    #[command(
        visible_alias = "addr",
        after_help = "Examples:\n  dogechain address <address>\n  dogechain address <address> --page 2"
    )]
    Address {
        address: String,

        /// Page of history to show, starting at 1
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        page: u32,
    },

    /// Show known labels (such as mining pools) for up to 100 addresses
    Labels {
        #[arg(required = true, num_args = 1..=100)]
        addresses: Vec<String>,
    },

    /// Show the chain tip, the mempool and the price
    Network,

    /// Show what it costs to send DOGE right now
    Fees,

    /// Show how many DOGE exist and how fast the supply grows
    Supply,

    /// Show the addresses holding the most DOGE, 100 per page (top 1,000)
    #[command(visible_alias = "top")]
    Richlist {
        /// Page to show, 1 to 10
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=10))]
        page: u32,
    },

    /// Show a chart series as a table of values
    #[command(
        after_help = "Examples:\n  dogechain chart tx_count --interval day\n  dogechain chart pool_share --last 7\n  dogechain chart tx_per_min --interval minute --json"
    )]
    Chart {
        /// Series name, e.g. tx_count, block_size, block_interval, fees_median,
        /// fees_total, hashrate, difficulty, supply, price_usd, tx_per_min,
        /// active_addresses, new_addresses, lookalikes, lookalike_senders,
        /// pool_share, transfers, transfers_sent, holder_share
        series: String,

        /// Time bucket for each point. `minute` works only with tx_per_min;
        /// pool_share, transfers, transfers_sent and holder_share take `day`
        /// (some also `hour`)
        #[arg(long, value_enum, default_value_t = Interval::Day)]
        interval: Interval,

        /// Text output shows only the most recent N points (0 for all).
        /// `--json` always returns every point.
        #[arg(long, value_name = "N", default_value_t = 30)]
        last: usize,
    },

    /// Stream live events until interrupted (one JSON object per line with --json)
    #[command(
        after_help = "Examples:\n  dogechain watch\n  dogechain watch txs --min 100000\n  dogechain watch all --json"
    )]
    Watch {
        /// Which events to show
        #[arg(value_enum, default_value_t = WatchWhat::Blocks)]
        what: WatchWhat,

        /// Only transactions whose outputs total at least this many DOGE,
        /// including the sender's change (for `txs` and `all`)
        #[arg(long, value_name = "DOGE", value_parser = parse_doge)]
        min: Option<Doge>,
    },

    /// Install the Dogechain skill for AI agents (Claude Code, Codex)
    #[command(subcommand)]
    Skill(SkillCommand),

    /// Add the Dogechain MCP server to AI agents (Claude Code, Codex)
    #[command(subcommand)]
    Mcp(McpCommand),

    /// Print every command with its API endpoint and output shape, as JSON
    Schema,

    /// Print a usage guide for scripts and AI agents
    Guide,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Interval {
    Minute,
    Hour,
    Day,
    Block,
    Week,
    Month,
}

impl Interval {
    pub fn as_str(self) -> &'static str {
        match self {
            Interval::Minute => "minute",
            Interval::Hour => "hour",
            Interval::Day => "day",
            Interval::Block => "block",
            Interval::Week => "week",
            Interval::Month => "month",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WatchWhat {
    /// New blocks
    Blocks,
    /// Mempool size and fee changes
    Mempool,
    /// Each transaction as it enters the mempool
    Txs,
    /// Every event
    All,
}

fn parse_doge(s: &str) -> Result<Doge, String> {
    match Doge::parse(s) {
        Some(d) if !d.is_negative() => Ok(d),
        _ => Err("expected an amount of DOGE, such as 1000 or 0.5".into()),
    }
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Download the skill from dogechain.com, verify it and install it
    #[command(
        after_help = "Installs into ~/.claude/skills (Claude Code) and ~/.agents/skills (Codex) for\n\
            each agent found. The skill is fetched from dogechain.com and checked against\n\
            its published SHA-256 digest before anything is written.\n\n\
            Examples:\n  dogechain skill install\n  dogechain skill install --project\n  \
            dogechain skill install --dir ~/my-agent/skills\n  dogechain skill install --print"
    )]
    Install {
        #[command(flatten)]
        location: SkillLocation,

        /// Print the verified SKILL.md instead of installing it
        #[arg(long)]
        print: bool,

        /// Replace copies that were changed since they were installed
        #[arg(long)]
        force: bool,
    },

    /// Show where the skill is installed and whether it is up to date
    Status {
        #[command(flatten)]
        location: SkillLocation,
    },

    /// Remove the installed skill
    Uninstall {
        #[command(flatten)]
        location: SkillLocation,

        /// Also remove copies that were changed since they were installed
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, clap::Args)]
pub struct SkillLocation {
    /// Use the agents' folders in the current project (.claude/skills and
    /// .agents/skills) instead of your home directory
    #[arg(long, conflicts_with = "dir")]
    pub project: bool,

    /// Use this skills folder instead; the skill goes in <DIR>/dogechain
    #[arg(long, value_name = "DIR")]
    pub dir: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    /// Add https://dogechain.com/mcp to Claude Code and Codex
    #[command(
        after_help = "Uses each agent's own command (`claude mcp add`, `codex mcp add`); nothing\n\
            runs locally. For Cursor, VS Code, Gemini CLI, Claude.ai and ChatGPT, --print\n\
            shows what to add.\n\n\
            Examples:\n  dogechain mcp install\n  dogechain mcp install --project\n  dogechain mcp install --print"
    )]
    Install {
        /// Add it to the current project only (Claude Code's project scope)
        #[arg(long)]
        project: bool,

        /// Show the setup for every supported app instead of changing anything
        #[arg(long)]
        print: bool,

        /// Replace a server named `dogechain` that points somewhere else
        #[arg(long)]
        force: bool,
    },

    /// Show which agents have the Dogechain MCP server
    Status,

    /// Remove the Dogechain MCP server from Claude Code and Codex
    Uninstall {
        /// Remove it from the current project only (Claude Code)
        #[arg(long)]
        project: bool,

        /// Also remove a server named `dogechain` that points somewhere else
        #[arg(long)]
        force: bool,
    },
}

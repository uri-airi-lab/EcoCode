//! CLI argument definitions for the EcoCode binary.

use clap::{Args, Parser, Subcommand};

/// Top-level CLI entry point.
#[derive(Parser, Debug)]
#[command(name = "ecocode", author, version, about = "EcoCode - Monitor your code's energy consumption", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: TopCommand,
}

/// Top-level subcommands. Used in main.rs to dispatch commands, either `run`, `monitor`, `monitor-local`, or `config`.
/// clap handles the matching and validation of arguments.
#[derive(Subcommand, Debug)]
pub enum TopCommand {
    /// Monitor the energy consumption of a command.
    Run(RunArgs),
    /// Headless energy monitor for an existing run (online mode, used by the Python SDK).
    /// Reads semantic markers from stdin and posts combined records to the EcoCode backend.
    Monitor(MonitorArgs),
    /// Headless energy monitor for an existing run (local mode, used by the Python SDK).
    /// Reads semantic markers from stdin and writes records to a local exporter (terminal, CSV, JSON, or logfile).
    MonitorLocal(MonitorLocalArgs),
    /// Manage EcoCode configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

/// Subcommands available under `ecocode config`.
#[derive(Subcommand, Debug)] // Also matched by clap: SetApiKey → "set-api-key" (camelCase words become kebab-case)
pub enum ConfigAction {
    /// Persist an API key to `~/.ecocode/config.json` for the online exporter.
    SetApiKey {
        /// The API key to save (obtained from the web platform under Settings > Developer).
        api_key: String,
    },
    /// Remove a previously stored API key from the config file.
    ClearApiKey,
    /// Print the current configuration (API key is masked).
    Show,
}

/// CLI arguments for a monitoring run (`ecocode run [OPTIONS] -- <COMMAND>`).
#[derive(Args, Debug)]
#[command(arg_required_else_help = true)]
pub struct RunArgs {
    /// Project name used to group runs in the database.
    #[arg(short, long, default_value = "ecocode")]
    pub project: String,

    /// Human-readable label for this specific run (defaults to a timestamp-based name).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Name of the person running the experiment.
    #[arg(long)]
    pub collaborator: Option<String>,

    /// Export destination: `terminal`, `csv`, `json`, `online`, `prometheus`, `logfile`.
    #[arg(short, long, default_value = "terminal")]
    pub output: String,

    /// Path to the output file (required for `csv`, `json`, and `logfile` formats).
    #[arg(short, long)]
    pub file: Option<String>,

    /// Sampling interval in seconds. Supports sub-second values (e.g. `0.5`).
    #[arg(short, long, default_value = "1.0")]
    pub interval: f64,

    /// Override carbon intensity in gCO2eq/kWh.
    #[arg(long)]
    pub intensity: Option<f64>,

    /// Electricity cost in the local currency per kWh.
    #[arg(long, alias = "cost")]
    pub price: Option<f64>,

    /// Physical address or city used to resolve the grid carbon intensity automatically.
    #[arg(long)]
    pub address: Option<String>,

    /// Fetch electricity prices from an external API instead of using a fixed cost.
    #[arg(long)]
    pub use_api_cost: bool,

    /// API key for the `online` exporter.
    #[arg(long, alias = "apikey")]
    pub api_key: Option<String>,

    /// The command to monitor, passed after `--`.
    #[arg(trailing_var_arg = true, required = true)]
    pub command: Vec<String>,
}

/// CLI arguments for the headless monitor (`ecocode monitor`).
/// This subcommand is spawned by the Python SDK as a subprocess.
/// It reads markers from stdin and posts combined energy + marker records to the backend.
#[derive(Args, Debug)]
pub struct MonitorArgs {
    /// Run ID (already created by the Python SDK).
    #[arg(long)]
    pub run_id: i64,

    /// PID of the process to monitor (the Python training process).
    #[arg(long)]
    pub pid: u32,

    /// Sampling interval in seconds.
    #[arg(long, default_value = "1.0")]
    pub interval: f64,
}

/// CLI arguments for the local headless monitor (`ecocode monitor-local`).
/// Spawned by the Python SDK when the user selects a local exporter (no API key needed).
/// Reads markers from stdin and writes records to a local exporter.
#[derive(Args, Debug)]
pub struct MonitorLocalArgs {
    /// Run ID, or 0 to have `ecocode monitor-local` create the project/run itself.
    #[arg(long)]
    pub run_id: i64,

    /// PID of the process to monitor.
    #[arg(long)]
    pub pid: u32,

    /// Sampling interval in seconds.
    #[arg(long, default_value = "1.0")]
    pub interval: f64,

    /// Local exporter type: `terminal`, `csv`, `json`, or `logfile`.
    #[arg(long, default_value = "terminal")]
    pub exporter: String,

    /// Output file path (required for `csv`, `json`, and `logfile` exporters).
    #[arg(long)]
    pub file: Option<String>,
}

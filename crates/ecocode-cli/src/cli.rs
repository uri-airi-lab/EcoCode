//! CLI argument definitions for the EcoCode binary.

use clap::{Args, Parser, Subcommand};
use ecocode_sensors::SensorMode;

/// Top-level CLI entry point.
#[derive(Parser, Debug)]
#[command(name = "ecocode", author, version, about = "EcoCode - Monitor your code's energy consumption", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: TopCommand,
}

/// Top-level subcommands, dispatched in main.rs. clap handles the matching and
/// validation of arguments.
#[derive(Subcommand, Debug)]
pub enum TopCommand {
    /// Monitor the energy consumption of a command.
    Run(RunArgs),
    /// Headless energy monitor for an existing run (used by the Python SDK).
    /// Reads semantic markers from stdin; records go to the EcoCode backend by default.
    Monitor(MonitorArgs),
    /// Same as `monitor`, but records go to a local exporter (terminal by default).
    MonitorLocal(MonitorArgs),
    /// Show which energy sensors work on this machine, and how to fix those that don't.
    Doctor(DoctorArgs),
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

    /// Sensor selection: auto (real sensors), fake (testing), none.
    #[arg(long, hide = true, env = "ECOCODE_SENSORS", default_value = "auto")]
    pub sensors: SensorMode,

    /// The command to monitor, passed after `--`.
    #[arg(trailing_var_arg = true, required = true)]
    pub command: Vec<String>,
}

/// CLI arguments for the headless monitors (`ecocode monitor` / `ecocode monitor-local`).
/// Spawned by the Python SDK as a subprocess; reads markers from stdin.
#[derive(Args, Debug)]
pub struct MonitorArgs {
    /// Run ID (already created by the Python SDK), or 0 to have a local
    /// exporter create the project/run itself.
    #[arg(long)]
    pub run_id: i64,

    /// PID of the process to monitor (the Python training process).
    #[arg(long)]
    pub pid: u32,

    /// Sampling interval in seconds.
    #[arg(long, default_value = "1.0")]
    pub interval: f64,

    /// Exporter: `online`, `terminal`, `csv`, `json`, or `logfile`.
    /// Defaults to `online` for `monitor` and `terminal` for `monitor-local`.
    #[arg(long)]
    pub exporter: Option<String>,

    /// Output file path (required for `csv`, `json`, and `logfile` exporters).
    #[arg(long)]
    pub file: Option<String>,

    /// Sensor selection: auto (real sensors), fake (testing), none.
    #[arg(long, hide = true, env = "ECOCODE_SENSORS", default_value = "auto")]
    pub sensors: SensorMode,
}

/// CLI arguments for `ecocode doctor`.
#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Print the report as JSON (for scripts).
    #[arg(long)]
    pub json: bool,

    /// Sensor selection: auto (real sensors), fake (testing), none.
    #[arg(long, hide = true, env = "ECOCODE_SENSORS", default_value = "auto")]
    pub sensors: SensorMode,
}

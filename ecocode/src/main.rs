//! EcoCode CLI — per-process energy and carbon-footprint monitor.
//!
//! Wraps any command and continuously samples:
//! - CPU energy via Intel RAPL (`/sys/class/powercap/intel-rapl/`)
//! - GPU energy and per-process utilization via NVIDIA NVML
//! - System memory consumption via `sysinfo`
//!
//! Measurements are attributed to the target PID using proportional resource
//! usage and exported in one of six formats (terminal, CSV, JSON, online, Prometheus, logfile).
//!
//! Exporter selection is explicit — no data is ever sent to the network without
//! the user choosing `--output online` (CLI) or `exporter="online"` (Python SDK).

use clap::Parser;

mod cli;
mod configs;
mod exporter;
mod monitor;
mod monitor_headless;
mod monitor_local;

use cli::{Cli, TopCommand};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        TopCommand::Run(args) => monitor::run_monitoring(args).await,
        TopCommand::Monitor(args) => monitor_headless::run_headless_monitor(args).await,
        TopCommand::MonitorLocal(args) => monitor_local::run_local_monitor(args).await,
        TopCommand::Config { action } => configs::handle_config_command(action),
    }
}

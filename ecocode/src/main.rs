//! EcoCode CLI — per-process energy and carbon-footprint monitor.
//!
//! Wraps any command and continuously samples:
//! - CPU energy via Intel RAPL (`/sys/class/powercap/intel-rapl/`)
//! - GPU energy and per-process utilization via NVIDIA NVML
//! - System memory consumption via `sysinfo`
//!
//! Measurements are attributed to the target PID using proportional resource
//! usage and exported in one of seven formats (terminal, CSV, JSON, SQLite, Online, Prometheus, logfile).

use clap::Parser;

mod carbon;
mod cli;
mod configs;
mod exporter;
mod monitor;

use cli::{Cli, TopCommand};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        TopCommand::Run(args) => monitor::run_monitoring(args).await,
        TopCommand::Config { action } => configs::handle_config_command(action),
    }
}

//! EcoCode CLI: per-process energy and carbon-footprint monitor.
//!
//! Wraps any command and continuously samples the machine's energy sensors
//! (RAPL, NVML, …) and the process tree's CPU and memory usage, attributes
//! energy to the process proportionally, and exports records in one of six
//! formats (terminal, CSV, JSON, online, Prometheus, logfile).
//!
//! Exporter selection is explicit: no data is ever sent to the network without
//! the user choosing `--output online` (CLI) or `exporter="online"` (Python SDK).

use clap::Parser;

mod cli;
mod commands;
mod config;
mod exporters;
mod platform;

use cli::{Cli, TopCommand};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        TopCommand::Run(args) => commands::run::run_monitoring(args),
        TopCommand::Monitor(args) => commands::monitor::run_monitor(args, "online"),
        TopCommand::MonitorLocal(args) => commands::monitor::run_monitor(args, "terminal"),
        TopCommand::Doctor(args) => commands::doctor::run_doctor(args),
        TopCommand::Config { action } => commands::config::handle_config_command(action),
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

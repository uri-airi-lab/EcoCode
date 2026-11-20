//! EcoCode CLI: per-process energy and carbon-footprint monitor.
//!
//! Wraps any command and continuously samples the machine's energy sensors
//! (RAPL, NVML, …) and the process tree's CPU and memory usage, attributes
//! energy to the process proportionally, and exports records in one of six
//! formats (terminal, CSV, JSON, online, Prometheus, logfile).
//!
//! Exporter selection is explicit: no data is ever sent to the network without
//! the user choosing `--output online` (CLI) or `exporter="online"` (Python SDK).

use std::io::Write;

use clap::Parser;

mod cli;
mod commands;
mod config;
mod exporters;
mod platform;

use cli::{Cli, TopCommand};

fn main() {
    let cli = Cli::parse();
    // `run` passes on the measured command's exit code; the other commands exit 0.
    let result = match cli.command {
        TopCommand::Run(args) => commands::run::run_monitoring(args),
        TopCommand::Monitor(args) => commands::monitor::run_monitor(args, "online").map(|()| 0),
        TopCommand::MonitorLocal(args) => commands::monitor::run_monitor(args, "terminal").map(|()| 0),
        TopCommand::Doctor(args) => commands::doctor::run_doctor(args).map(|()| 0),
        TopCommand::Config { action } => commands::config::handle_config_command(action).map(|()| 0),
    };
    match result {
        Ok(0) => {}
        Ok(code) => {
            let _ = std::io::stdout().flush();
            std::process::exit(code);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

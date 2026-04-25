//! EcoCode CLI — per-process energy and carbon-footprint monitor.
//!
//! Wraps any command and continuously samples:
//! - CPU energy via Intel RAPL (`/sys/class/powercap/intel-rapl/`)
//! - GPU energy and per-process utilization via NVIDIA NVML
//! - System memory consumption via `sysinfo`
//!
//! Measurements are attributed to the target PID using proportional resource
//! usage and exported in one of seven formats (terminal, CSV, JSON, SQLite,
//! PostgreSQL, Prometheus, logfile).

use clap::Parser;

mod carbon;
mod cli;
mod exporter;
mod monitor;
mod sensor;

use cli::{ConfigAction, RunArgs};
use exporter::online::OnlineExporter;

/// Dispatches `ecocode config` subcommands (set-api-key, clear-api-key, show).
fn handle_config_command(action: ConfigAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::SetApiKey { api_key } => {
            OnlineExporter::save_api_key(&api_key)?;
            println!("API key saved successfully to ~/.ecocode/config.json");
            println!("You can now use the online exporter without --api-key flag");
            Ok(())
        }
        ConfigAction::ClearApiKey => {
            OnlineExporter::clear_api_key()?;
            println!("API key cleared from ~/.ecocode/config.json");
            Ok(())
        }
        ConfigAction::Show => {
            let config_path = OnlineExporter::config_path()?;
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                let config: serde_json::Value = serde_json::from_str(&content)?;

                println!("EcoCode Configuration");
                println!("Location: {}", config_path.display());
                println!();

                if let Some(api_key) = config.get("api_key").and_then(|v| v.as_str()) {
                    let masked = if api_key.len() > 8 {
                        format!("{}...{}", &api_key[..4], &api_key[api_key.len()-4..])
                    } else {
                        "***".to_string()
                    };
                    println!("API Key: {}", masked);
                } else {
                    println!("API Key: (not set)");
                }

                if let Some(api_url) = config.get("api_url").and_then(|v| v.as_str()) {
                    println!("API URL: {}", api_url);
                } else {
                    println!("API URL: (using default)");
                }
            } else {
                println!("EcoCode Configuration");
                println!("Location: {}", config_path.display());
                println!();
                println!("No configuration file found.");
                println!("Use 'ecocode config set-api-key <KEY>' to save your API key.");
            }
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args_vec: Vec<String> = std::env::args().collect();
    if args_vec.len() > 1 && args_vec[1] == "config" {
        #[derive(Parser)]
        #[command(name = "ecocode config")]
        struct ConfigCli {
            #[command(subcommand)]
            action: ConfigAction,
        }

        let config_args: Vec<String> = std::iter::once("ecocode".to_string())
            .chain(args_vec.iter().skip(2).cloned())
            .collect();

        let config_cli = ConfigCli::parse_from(config_args);
        return handle_config_command(config_cli.action);
    }

    let args = RunArgs::parse();
    monitor::run_monitoring(args).await
}

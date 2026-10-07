//! One module per `ecocode` subcommand, plus helpers they share.

pub mod config;
pub mod doctor;
pub mod monitor;
pub mod run;

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ecocode_sensors::{Detection, Domain, EnergySensor, Status};
use sysinfo::System;

use crate::exporters::SystemInfo;
use crate::exporters::csv::CsvExporter;
use crate::exporters::json::JsonExporter;
use crate::exporters::logfile::LogFileExporter;
use crate::exporters::terminal::TerminalExporter;
use crate::exporters::Exporter;

/// Creates a local (file or terminal) exporter. `None` for unknown kinds.
pub fn local_exporter(kind: &str, file: Option<&str>) -> Option<Result<Box<dyn Exporter>, Box<dyn Error>>> {
    let require_file = |format: &str| -> Result<String, Box<dyn Error>> {
        file.map(str::to_string)
            .ok_or_else(|| format!("--file is required for the '{}' exporter.", format).into())
    };
    let exporter: Result<Box<dyn Exporter>, Box<dyn Error>> = match kind {
        "terminal" => Ok(Box::new(TerminalExporter::new())),
        "csv" => require_file("csv").and_then(|f| Ok(Box::new(CsvExporter::new(f)?) as Box<dyn Exporter>)),
        "json" => require_file("json").and_then(|f| Ok(Box::new(JsonExporter::new(f)?) as Box<dyn Exporter>)),
        "logfile" => {
            require_file("logfile").and_then(|f| Ok(Box::new(LogFileExporter::new(f)?) as Box<dyn Exporter>))
        }
        _ => return None,
    };
    Some(exporter)
}

/// Hardware summary sent once per run.
pub fn system_info(sys: &System, sensors: &[Box<dyn EnergySensor>]) -> SystemInfo {
    let gpu_names: Vec<String> = gpu_channels(sensors).map(|(_, name)| name.to_string()).collect();
    SystemInfo {
        cpu_name: sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string()),
        cpu_cores: sys.cpus().len() as u32,
        ram_gb: sys.total_memory() as f64 / 1_073_741_824.0,
        gpu_count: gpu_names.len() as u32,
        gpu_names,
    }
}

/// (device index, name) of every GPU channel.
pub fn gpu_channels(sensors: &[Box<dyn EnergySensor>]) -> impl Iterator<Item = (u32, &str)> {
    sensors.iter().flat_map(|s| s.channels()).filter_map(|c| match c.domain {
        Domain::Gpu(i) => Some((i, c.name.as_str())),
        _ => None,
    })
}

/// Prints sensors that exist but could not be used, and a note when nothing
/// can measure energy. Missing hardware is silent (see `ecocode doctor`).
pub fn warn_about_sensors(detection: &Detection) {
    for d in &detection.diagnostics {
        if matches!(d.status, Status::PermissionDenied | Status::Error) {
            eprintln!("Warning: {}: {}", d.source, d.detail);
            if let Some(hint) = &d.hint {
                eprintln!("  Fix: {hint}");
            }
        }
    }
    if !detection.has_energy() {
        eprintln!(
            "Notice: no energy sensors available on this machine; energy will be reported as 0. \
             Run `ecocode doctor` for details."
        );
    }
}

/// Sets the returned flag on the first Ctrl+C (or SIGTERM); a second Ctrl+C
/// exits immediately.
pub fn install_stop_flag() -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let _ = ctrlc::set_handler(move || {
        if flag.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
    });
    stop
}

/// Validates the sampling interval.
pub fn check_interval(interval: f64) -> Result<std::time::Duration, Box<dyn Error>> {
    if interval.is_nan() || interval < 0.1 {
        return Err("Sampling interval must be at least 0.1 seconds".into());
    }
    Ok(std::time::Duration::from_secs_f64(interval))
}

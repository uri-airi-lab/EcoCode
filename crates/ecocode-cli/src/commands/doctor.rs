//! `ecocode doctor`: what this machine can measure, and how to fix what it can't.

use std::error::Error;

use ecocode_sensors::{Detection, Status, detect};
use serde_json::{Value, json};
use sysinfo::{RefreshKind, System};

use crate::cli::DoctorArgs;

pub fn run_doctor(args: DoctorArgs) -> Result<(), Box<dyn Error>> {
    let detection = detect(args.sensors);
    let sys = System::new_with_specifics(RefreshKind::everything());
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report_json(&detection, &sys))?);
        return Ok(());
    }

    println!("EcoCode doctor (ecocode {})", env!("CARGO_PKG_VERSION"));
    println!();
    println!(
        "  OS:      {} {} ({})",
        System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        std::env::consts::ARCH,
        System::kernel_version().unwrap_or_default()
    );
    println!(
        "  CPU:     {} ({} logical cores)",
        sys.cpus().first().map(|c| c.brand().trim()).unwrap_or("Unknown"),
        sys.cpus().len()
    );
    println!("  RAM:     {:.1} GB", sys.total_memory() as f64 / 1_073_741_824.0);
    println!(
        "  Process: {} (CPU time; memory as {}{})",
        detection.inspector.name(),
        detection.inspector.memory_kind(),
        if detection.inspector.has_cpu_energy() { "; per-process CPU energy" } else { "" }
    );
    println!();
    println!("Energy sensors:");
    if detection.diagnostics.is_empty() {
        println!("  (none probed)");
    }
    for d in &detection.diagnostics {
        let mark = match d.status {
            Status::Available => "[ok]",
            Status::Unavailable => "[--]",
            Status::PermissionDenied | Status::Error => "[!!]",
        };
        println!("  {mark} {:<9} {}", d.source, d.detail);
        if let Some(hint) = &d.hint {
            println!("       fix: {hint}");
        }
    }
    println!();
    let channels: Vec<String> = detection
        .channels()
        .map(|(sensor, c)| format!("{} ({}, {:?})", c.domain, sensor, c.quality))
        .collect();
    if channels.is_empty() {
        println!("Result: no energy can be measured on this machine; runs will report 0 J.");
        println!("        CPU and memory usage are still recorded.");
    } else {
        println!("Result: measuring {}", channels.join(", "));
    }
    Ok(())
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Available => "available",
        Status::Unavailable => "unavailable",
        Status::PermissionDenied => "permission_denied",
        Status::Error => "error",
    }
}

/// The same report as the text output, for scripts (`ecocode doctor --json`).
fn report_json(detection: &Detection, sys: &System) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "os_version": System::long_os_version(),
        "arch": std::env::consts::ARCH,
        "cpu": sys.cpus().first().map(|c| c.brand().trim().to_string()),
        "logical_cores": sys.cpus().len(),
        "ram_gb": (sys.total_memory() as f64 / 1_073_741_824.0 * 10.0).round() / 10.0,
        "process_inspector": {
            "name": detection.inspector.name(),
            "memory": detection.inspector.memory_kind(),
            "per_process_cpu_energy": detection.inspector.has_cpu_energy(),
        },
        "diagnostics": detection.diagnostics.iter().map(|d| json!({
            "source": d.source,
            "status": status_name(d.status),
            "detail": d.detail,
            "hint": d.hint,
        })).collect::<Vec<_>>(),
        "channels": detection.channels().map(|(sensor, c)| json!({
            "sensor": sensor,
            "domain": c.domain.label(),
            "socket": c.socket,
            "name": c.name,
            "quality": format!("{:?}", c.quality).to_lowercase(),
        })).collect::<Vec<_>>(),
    })
}

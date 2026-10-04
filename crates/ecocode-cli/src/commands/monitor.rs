//! `ecocode monitor` / `ecocode monitor-local`: headless monitor spawned by the
//! Python SDK as a subprocess (standalone mode).
//!
//! Samples an existing process (`--pid`), reads semantic markers from stdin
//! (newline-delimited JSON) and sends combined records to one exporter:
//! - `monitor` defaults to `online`: the SDK has already created the run and
//!   passes its ID; records go to the backend (key in `ECOCODE_API_KEY`). The
//!   SDK completes the run itself.
//! - `monitor-local` defaults to `terminal` (also `csv`, `json`, `logfile`):
//!   no account or network. With `--run-id 0` the monitor creates the
//!   project and run itself.
//!
//! Exits once the target process is gone, or after one final sample once stdin
//! is closed (the SDK's run ended).

use std::error::Error;
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use ecocode_monitor::markers::{self, MarkerState};
use ecocode_monitor::{Control, Sampler, run_loop};
use ecocode_sensors::detect;

use crate::cli::MonitorArgs;
use crate::commands::{install_stop_flag, local_exporter, system_info, warn_about_sensors};
use crate::exporters::online::OnlineExporter;
use crate::exporters::terminal::TerminalExporter;
use crate::exporters::{Exporter, ExporterSink, ExporterType};

const LOG_PREFIX: &str = "[ecocode monitor]";

/// Entry point for `ecocode monitor` (`default_exporter = "online"`) and
/// `ecocode monitor-local` (`default_exporter = "terminal"`).
pub fn run_monitor(args: MonitorArgs, default_exporter: &str) -> Result<(), Box<dyn Error>> {
    if args.interval.is_nan() || args.interval <= 0.0 {
        return Err("Sampling interval must be greater than 0 seconds".into());
    }
    let kind = args.exporter.as_deref().unwrap_or(default_exporter).to_lowercase();

    let mut exporter: Box<dyn Exporter> = match kind.as_str() {
        "online" => {
            let api_key = std::env::var("ECOCODE_API_KEY")
                .map_err(|_| "ECOCODE_API_KEY environment variable is required")?;
            Box::new(OnlineExporter::with_key(&api_key)?)
        }
        other => match local_exporter(other, args.file.as_deref()) {
            Some(exporter) => exporter.map_err(|e| format!("{LOG_PREFIX} {e}"))?,
            None => {
                eprintln!("{LOG_PREFIX} Unknown exporter '{}', falling back to 'terminal'.", other);
                Box::new(TerminalExporter::new())
            }
        },
    };

    // --- Marker reader thread (reads newline-delimited JSON from stdin) ---
    let marker_state = Arc::new(Mutex::new(MarkerState::default()));
    let marker_state_for_stdin = Arc::clone(&marker_state);
    let stdin_closed = Arc::new(AtomicBool::new(false));
    let stdin_closed_writer = Arc::clone(&stdin_closed);

    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break }; // stdin closed (EOF or pipe broken)
            if let Some(event) = markers::parse_line(&line) {
                marker_state_for_stdin.lock().unwrap().apply(&event);
            }
        }
        stdin_closed_writer.store(true, Ordering::SeqCst);
    });

    let detection = detect(args.sensors);
    warn_about_sensors(&detection);
    let stop = install_stop_flag();

    let mut sampler = Sampler::new(detection.sensors, detection.inspector, args.pid, args.run_id);

    if args.run_id == 0 && exporter.exporter_type() != ExporterType::Online {
        let project = exporter.get_or_create_project("ecocode", Some("unassigned"))?;
        let run_name = format!("run_{}", Utc::now().format("%Y-%m-%d_%H-%M-%S"));
        let run_id = exporter.create_run(
            &run_name,
            project.id,
            Some("unassigned"),
            Some(Utc::now().to_rfc3339().as_str()),
            None,
        )?;
        exporter.save_system_info(run_id, system_info(sampler.system(), sampler.sensors()))?;
        sampler.set_run_id(run_id);
    }

    run_loop(
        &mut sampler,
        Duration::from_secs_f64(args.interval),
        &stop,
        &marker_state,
        &mut ExporterSink { exporter: exporter.as_mut(), log_errors_as: Some(LOG_PREFIX) },
        |sampler| {
            if !sampler.root_alive() {
                Control::Stop
            } else if stdin_closed.load(Ordering::SeqCst) {
                Control::LastSample
            } else {
                Control::Continue
            }
        },
    )?;

    // Flush final export
    if let Err(e) = exporter.export() {
        eprintln!("{LOG_PREFIX} Final export error: {}", e);
    }

    Ok(())
}

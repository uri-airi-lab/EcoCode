//! `ecocode run -- <command>`: spawn a command and measure it until it exits.
//!
//! Creates the run with the selected exporter, spawns the target process with
//! the marker pipe attached, samples it on every tick, and writes the final run
//! summary.

use std::collections::BTreeMap;
use std::error::Error;
use std::io::BufRead;
use std::process::Command;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use ecocode_monitor::markers::{self, MarkerState};
use ecocode_monitor::{Control, LoopEnd, Sampler, run_loop};
use ecocode_sensors::{Domain, detect};

use crate::cli::RunArgs;
use crate::commands::{check_interval, gpu_channels, install_stop_flag, local_exporter, system_info, warn_about_sensors};
use crate::exporters::online::OnlineExporter;
use crate::exporters::prometheus::PrometheusExporter;
use crate::exporters::terminal::TerminalExporter;
use crate::exporters::{Exporter, ExporterSink, ExporterType, box_row};
use crate::platform;

/// Core monitoring loop for `ecocode run`.
pub fn run_monitoring(args: RunArgs) -> Result<(), Box<dyn Error>> {
    let period = check_interval(args.interval)?;

    let output = args.output.to_lowercase();
    let project_name = args.project;
    let collaborator = args
        .collaborator
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // --- Exporter setup ---
    // Each exporter is independent. No silent data mirroring.
    // Use `--output online` (or exporter="online" in the Python SDK) to send data
    // to the EcoCode platform. All other exporters store data locally.
    let mut exporter: Box<dyn Exporter> = match output.as_str() {
        "online" => Box::new(OnlineExporter::new(args.api_key)?),
        "prometheus" => Box::new(PrometheusExporter::new()?),
        other => match local_exporter(other, args.file.as_deref()) {
            Some(exporter) => exporter?,
            None => {
                eprintln!("Warning: unknown exporter '{}', falling back to 'terminal'.", other);
                Box::new(TerminalExporter::new())
            }
        },
    };
    if output == "online" && args.collaborator.is_some() {
        println!(
            "Notice: In 'online' mode, the collaborator name is automatically linked to your EcoCode account. The provided --collaborator value will be ignored."
        );
    }

    println!("Exporter type: {:?}", exporter.exporter_type());

    let project = exporter.get_or_create_project(&project_name, collaborator.as_deref())?;

    let run_collaborator = collaborator
        .clone()
        .or(project.selected_collaborator_name.clone())
        .unwrap_or_else(|| "unassigned".to_string());

    let run_started_at = Utc::now().to_rfc3339();
    let run_name = args
        .name
        .unwrap_or_else(|| format!("run_{}", Utc::now().format("%Y-%m-%d_%H-%M-%S")));
    let full_command = args.command.join(" ");
    let run_id = exporter.create_run(
        &run_name,
        project.id,
        Some(run_collaborator.as_str()),
        Some(run_started_at.as_str()),
        Some(&full_command),
    )?;

    if args.intensity.is_some() || args.price.is_some() || args.address.is_some() || args.use_api_cost {
        exporter.update_project_config(
            project.id,
            args.intensity,
            args.price,
            args.address.clone(),
            if args.use_api_cost { Some(true) } else { None },
        )?;
    }

    let carbon_config = exporter.get_carbon_config(project.id)?;

    if exporter.exporter_type() == ExporterType::Online
        && (project.is_new || (carbon_config.manual_cost_per_kwh.is_none() && !carbon_config.use_api_for_cost))
    {
        if carbon_config.manual_cost_per_kwh.is_none()
            && !carbon_config.use_api_for_cost
            && args.price.is_none()
            && !args.use_api_cost
        {
            return Err(
                "Financial cost per kWh (--price or --cost) or --use-api-cost is required for new projects or if not already set."
                    .into(),
            );
        }
        if project.is_new && args.intensity.is_none() {
            println!(
                "Notice: No --intensity provided. Using default global average ({:.1} gCO2/kWh).",
                carbon_config.resolved_intensity_g_per_kwh
            );
        }
    }

    let detection = detect(args.sensors);
    warn_about_sensors(&detection);

    // --- Pipe-based marker IPC (attached mode) ---
    // The Python SDK running inside the child process sends semantic marker
    // events (phase, tokens, step, loss, lr, seq_len, …) as newline-delimited
    // JSON to the inherited write end.
    let marker_state = Arc::new(Mutex::new(MarkerState::default()));

    // Marker pipe transit latency (nanoseconds), one sample per marker received,
    // used to report SDK-to-monitor marker overhead at the end of the run.
    let marker_latencies_ns: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));

    let (pipe_reader, pipe_writer) = os_pipe::pipe()?;
    let marker_state_for_task = Arc::clone(&marker_state);
    let marker_latencies_for_task = Arc::clone(&marker_latencies_ns);

    // Blocking reader thread for the pipe read end; ends when the child (and
    // every process that inherited the write end) has exited.
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(pipe_reader);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Some(event) = markers::parse_line(&line) {
                // Every marker carries a Python-side send timestamp (nanoseconds
                // since UNIX epoch); comparing it against our own wall-clock time
                // here gives the pipe transit latency for this specific marker.
                if let Some(sent_at_ns) = event.get("sent_at_ns").and_then(|v| v.as_u64()) {
                    let now_ns = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(sent_at_ns);
                    if now_ns >= sent_at_ns {
                        marker_latencies_for_task.lock().unwrap().push(now_ns - sent_at_ns);
                    }
                }
                marker_state_for_task.lock().unwrap().apply(&event);
            }
        }
    });

    // --- Spawn the target process ---
    // ECOCODE_RUN_ID plus the marker pipe variable let the Python SDK running
    // inside the child attach to this run (attached mode).
    let mut command = Command::new(&args.command[0]);
    command.args(&args.command[1..]).env("ECOCODE_RUN_ID", run_id.to_string());
    platform::pass_marker_pipe(&mut command, &pipe_writer)?;
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to execute '{}': {}", args.command[0], e))?;

    // Close the write end in the parent — only the child writes markers.
    drop(pipe_writer);
    drop(command);

    let child_pid = child.id();
    let tree_guard = match platform::TreeGuard::attach(&child) {
        Ok(guard) => Some(guard),
        Err(e) => {
            eprintln!("Warning: could not track the command's process tree ({e}); only the command itself will be stopped on cancel.");
            None
        }
    };

    // Install a panic hook that kills the child process to avoid orphans.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        platform::kill(child_pid);
        prev_hook(info);
    }));

    let stop = install_stop_flag();

    let mut sampler = Sampler::new(detection.sensors, detection.inspector, child_pid, run_id);

    exporter.save_system_info(run_id, system_info(sampler.system(), sampler.sensors()))?;

    print_system_summary(&sampler, exporter.exporter_type(), args.interval, &carbon_config);

    let loop_result = run_loop(
        &mut sampler,
        period,
        &stop,
        &marker_state,
        &mut ExporterSink { exporter: exporter.as_mut(), log_errors_as: None },
        |_| {
            // The child handle itself tells whether the root process exited
            // (avoids PID-reuse false positives from a process-table lookup).
            match child.try_wait() {
                Ok(Some(status)) => {
                    println!("Process {} finished (status: {:?})", child_pid, status);
                    Control::Stop
                }
                Ok(None) => Control::Continue,
                Err(e) => {
                    eprintln!("Warning: failed to check child status ({e})");
                    Control::Continue
                }
            }
        },
    );

    let canceled = matches!(loop_result, Ok(LoopEnd::Interrupted));
    if canceled {
        eprintln!("\nCtrl+C received — saving run data...");
    }
    let execution_result = loop_result.and_then(|_| exporter.export());

    if canceled {
        sampler.refresh();
        match &tree_guard {
            Some(guard) => guard.stop(&mut child, &sampler.tree()),
            None => {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let run_ended_at = Utc::now().to_rfc3339();
    let final_status = if canceled {
        "canceled"
    } else if execution_result.is_ok() {
        "finished"
    } else {
        "failed"
    };

    exporter.complete_run(run_id, final_status, Some(run_ended_at.as_str()))?;
    exporter.run_summary(run_id)?;

    {
        let latencies = marker_latencies_ns.lock().unwrap();
        if !latencies.is_empty() {
            let count = latencies.len();
            let sum_ns: u64 = latencies.iter().sum();
            let mean_us = (sum_ns as f64 / count as f64) / 1000.0;
            let min_us = *latencies.iter().min().unwrap() as f64 / 1000.0;
            let max_us = *latencies.iter().max().unwrap() as f64 / 1000.0;
            println!(
                "Marker pipe latency: {} samples, mean {:.1}\u{b5}s, min {:.1}\u{b5}s, max {:.1}\u{b5}s",
                count, mean_us, min_us, max_us
            );
        }
    }

    // Restore the default panic hook now that the child is done.
    let _ = std::panic::take_hook();

    execution_result
}

fn print_system_summary(
    sampler: &Sampler,
    exporter_type: ExporterType,
    interval: f64,
    carbon_config: &crate::exporters::CarbonConfig,
) {
    let sys = sampler.system();
    let sensors = sampler.sensors();

    println!();
    println!("╔════════════════════════════════════════════════════╗");
    println!("║              EcoCode System Summary                ║");
    println!("╠════════════════════════════════════════════════════╣");
    if let Some(cpu) = sys.cpus().first() {
        println!("{}", box_row(&format!("  CPU: {}", cpu.brand().trim())));
    }
    println!("{}", box_row(&format!("  Cores / Threads: {}", sys.cpus().len())));
    println!("{}", box_row(&format!("  Total RAM: {:.1} GB", sys.total_memory() as f64 / 1_073_741_824.0)));

    let names: Vec<&str> = sensors.iter().map(|s| s.name()).collect();
    let sensor_list = if names.is_empty() { "none (see `ecocode doctor`)".to_string() } else { names.join(", ") };
    println!("{}", box_row(&format!("  Energy sensors: {}", sensor_list)));

    let mut domains_per_socket: BTreeMap<i16, Vec<String>> = BTreeMap::new();
    for channel in sensors.iter().flat_map(|s| s.channels()) {
        if !matches!(channel.domain, Domain::Gpu(_)) {
            domains_per_socket.entry(channel.socket).or_default().push(channel.domain.label());
        }
    }
    println!("{}", box_row(&format!("  CPU Sockets: {}", domains_per_socket.len())));
    for (socket, domains) in &domains_per_socket {
        println!("{}", box_row(&format!("    Socket {}: {}", socket, domains.join(", "))));
    }

    let gpus: Vec<(u32, &str)> = gpu_channels(sensors).collect();
    println!("{}", box_row(&format!("  GPUs: {}", gpus.len())));
    for (i, name) in gpus {
        println!("{}", box_row(&format!("    GPU {}: {}", i, name)));
    }

    println!("{}", box_row(&format!("  Exporter: {:?}", exporter_type)));
    println!("{}", box_row(&format!("  Interval: {:.1}s", interval)));
    println!("╠════════════════════════════════════════════════════╣");
    println!("{}", box_row(&format!("  Carbon Intensity: {:.1} gCO2/kWh", carbon_config.resolved_intensity_g_per_kwh)));
    println!("{}", box_row(&format!("  Intensity Source: {}", carbon_config.intensity_source)));
    println!("{}", box_row(&format!("  PUE Factor: {:.2}", carbon_config.pue_factor)));
    println!("╚════════════════════════════════════════════════════╝");
    println!();
}

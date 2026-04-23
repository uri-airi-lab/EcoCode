use clap::Parser;
use nvml_wrapper::Nvml;
use sqlx::types::chrono::Utc;
use std::io;
use std::process::Command;
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, RefreshKind, System};
use tokio::time::{self, Instant, MissedTickBehavior};
mod carbon;
mod exporter;
mod sensor;
use sensor::RAPL_PATH;

use sensor::gpu::{get_all_gpu_devices, get_all_gpu_energies, get_all_gpu_energies_by_pid};
use sensor::rapl::scan_rapl_files;

use exporter::csv::CsvExporter;
use exporter::json::JsonExporter;
use exporter::local::SqliteExporter;
use exporter::logfile::LogFileExporter;
use exporter::online::OnlineExporter;
use exporter::terminal::TerminalExporter;
use exporter::{Exporter, ExporterType, GpuMetric, Record, SocketMetric};

use crate::exporter::prometheus::PrometheusExporter;
use crate::sensor::rapl::{delta_cpu_energy_per_pid_w, get_all_energies};

// TODO: Make it prettier dynamic with Rattatui
// --- Command-line argument parsing ---
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "ecocode")]
    project: String,

    /// Optional custom name for this run
    #[arg(short, long)]
    name: Option<String>,

    /// Collaborator name to save/reuse for local project runs
    #[arg(long)]
    collaborator: Option<String>,

    /// Output format: "terminal", "csv", "json", "local", "online", "prometheus", "logfile"
    #[arg(short, long, default_value = "terminal")]
    output: String,

    /// Output file path (required if output=csv, json, logfile)
    #[arg(short, long)]
    file: Option<String>,

    /// Measurement interval in seconds
    #[arg(short, long, default_value = "1")]
    interval: u64,
    /// Command to monitor (with its arguments)
    #[arg(trailing_var_arg = true, required = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Args = Args::parse();

    let interval = args.interval;
    let output = args.output.to_lowercase();
    let project_name = args.project;
    let collaborator = match args.collaborator {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.len() > 0 {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        None => None,
    };

    // Idiomatic Rust way.
    // let collaborator = args
    //     .collaborator
    //     // Option<String> → Option<&str>
    //     // Converts from owned String to a string slice reference
    //     // This avoids cloning or moving the String at this stage
    //     .as_deref()

    //     // If Some(&str), apply trim()
    //     // Removes leading and trailing whitespace
    //     // Example: "  Alice  " → "Alice"
    //     .map(str::trim)

    //     // Keep only non-empty strings
    //     // If the string is "" after trimming, convert it to None
    //     // This acts as a validation step
    //     .filter(|name| !name.is_empty())

    //     // Convert &str back to owned String
    //     // Needed because final variable likely requires ownership
    //     .map(str::to_owned);

    // Before running the command run sudo chmod -R +r /sys/class/powercap/intel-rapl/
    //promp the user for their password to access
    println!(
        "Running: sudo chmod -R +r /sys/class/powercap/intel-rapl/ , please enter your password if needed."
    );
    let _access = Command::new("sudo")
        .arg("chmod")
        .arg("-R")
        .arg("+r")
        .arg(RAPL_PATH) // FIXED: will be changed for multi-socket support
        .status();

    // --- Exporter setup ---
    let require_file = |format: &str| -> String {
        args.file.clone().unwrap_or_else(|| {
            eprintln!(
                "Error: --file (-f) is required for the '{}' exporter.",
                format
            );
            std::process::exit(1);
        })
    };

    let mut exporter: Box<dyn Exporter> = match output.as_str() {
        "terminal" => Box::new(TerminalExporter::new()),
        "csv" => Box::new(CsvExporter::new(require_file("csv"))?),
        "json" => Box::new(JsonExporter::new(require_file("json"))?),
        "local" => Box::new(SqliteExporter::new().await?),
        "online" => Box::new(OnlineExporter::new().await?),
        "prometheus" => Box::new(PrometheusExporter::new()),
        "logfile" => Box::new(LogFileExporter::new(require_file("logfile"))?),
        _ => Box::new(TerminalExporter::new()),
    };
    println!("Exporter type: {:?}", exporter.exporter_type());

    let project = exporter
        .get_or_create_project(&project_name, collaborator.as_deref())
        .await?;

    let available_collaborators = if project.collaborators.is_empty() {
        "no saved collaborators".to_string()
    } else {
        format!("saved collaborators: {}", project.collaborators.join(", "))
    };

    let run_collaborator = if exporter.exporter_type() == ExporterType::Local {
        project.selected_collaborator_name.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Project '{project_name}' needs an explicit collaborator selection ({available_collaborators}). Re-run with --collaborator <name>."
                ),
            )
        })?
    } else {
        collaborator
            .clone()
            .or(project.selected_collaborator_name.clone())
            .unwrap_or_else(|| "unassigned".to_string())
    };

    let run_started_at = Utc::now().to_rfc3339();
    let run_name = args.name.unwrap_or_else(|| {
        format!("run_{}", Utc::now().format("%Y-%m-%d_%H-%M-%S"))
    });
    let full_command = args.command.join(" ");
    let run_id = exporter
        .create_run(
            &run_name,
            project.id,
            Some(run_collaborator.as_str()),
            Some(run_started_at.as_str()),
            Some(&full_command),
        )
        .await?;

    let carbon_config = exporter.get_carbon_config(project.id).await?;

    // Open sys/class/powercap dir and look for intel-rapl files and make a buffer for them
    let mut rapl_readers = scan_rapl_files()?; //sorted by domain

    // --- Spawn the target process ---
    let command = Command::new(&args.command[0])
        .args(&args.command[1..])
        .spawn()
        .expect("failed to execute process");

    // Get the PID of the spawned process
    let pid = Pid::from(command.id() as usize);

    // --- System setup ---
    let mut sys = System::new_with_specifics(RefreshKind::everything());
    let num_cores = sys.cpus().len();
    let sys_memo = sys.total_memory();
    let mut cpu_usage = 0.0_f32;
    let mut mem_usage = 0.0_f64;

    // --- NVML / GPU setup (multi-GPU) ---
    let nvml = Nvml::init()?;
    let devices = get_all_gpu_devices(&nvml)?;
    let gpu_count = devices.len();

    let mut gpu_names = Vec::new();
    for dev in &devices {
        if let Ok(name) = dev.name() {
            gpu_names.push(name);
        }
    }

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let sys_info = crate::exporter::SystemInfo {
        cpu_name,
        cpu_cores: num_cores as u32,
        ram_gb: sys_memo as f64 / 1_073_741_824.0,
        gpu_count: gpu_count as u32,
        gpu_names,
    };
    exporter.save_system_info(run_id, sys_info).await?;

    // --- Print system info summary ---
    println!();
    println!("╔════════════════════════════════════════════════════╗");
    println!("║              EcoCode System Summary                ║");
    println!("╠════════════════════════════════════════════════════╣");
    // CPU info
    if let Some(cpu) = sys.cpus().first() {
        println!("║  CPU: {:44}║", cpu.brand().trim());
    }
    println!("║  Cores / Threads: {:32}║", format!("{}", num_cores));
    println!(
        "║  Total RAM: {:38}║",
        format!("{:.1} GB", sys_memo as f64 / 1_073_741_824.0)
    );

    // Socket / RAPL summary
    let mut sockets: std::collections::BTreeSet<i16> = std::collections::BTreeSet::new();
    let mut domains_per_socket: std::collections::BTreeMap<i16, Vec<String>> =
        std::collections::BTreeMap::new();
    for r in &rapl_readers {
        sockets.insert(r.socket);
        domains_per_socket
            .entry(r.socket)
            .or_default()
            .push(r.domain.clone());
    }
    println!("║  CPU Sockets (RAPL): {:29}║", sockets.len());
    for (socket, domains) in &domains_per_socket {
        println!("║    Socket {}: {:37}║", socket, domains.join(", "));
    }

    // GPU info
    println!("║  NVIDIA GPUs: {:36}║", gpu_count);
    for (i, dev) in devices.iter().enumerate() {
        if let Ok(name) = dev.name() {
            println!("║    GPU {}: {:40}║", i, name);
        }
    }

    println!(
        "║  Exporter: {:39}║",
        format!("{:?}", exporter.exporter_type())
    );
    println!("║  Interval: {:39}║", format!("{}s", args.interval));
    println!("╠════════════════════════════════════════════════════╣");
    println!("║  Carbon Intensity: {:31}║", format!("{:.1} gCO2/kWh", carbon_config.resolved_intensity_g_per_kwh));
    println!("║  Intensity Source: {:31}║", carbon_config.intensity_source);
    println!("║  PUE Factor: {:36}║", format!("{:.2}", carbon_config.pue_factor));
    println!("╚════════════════════════════════════════════════════╝");
    println!();

    // --- Measurement state ---
    let mut iteration = 0;

    // Per-GPU timestamps for process utilization sampling
    let mut gpu_timestamps: Vec<u64> = vec![0; gpu_count];

    // Take initial energy readings before the loop
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    let mut cpu_energy_1 = get_all_energies(&mut rapl_readers)?;

    let mut gpu_energies_1 = get_all_gpu_energies(&devices)?;
    // Running maximum energy (mJ) observed per GPU — used for rollover correction
    // when the NVML counter resets (driver reload / GPU reset).
    let mut gpu_max_energies: Vec<f64> = gpu_energies_1.clone();

    let period = Duration::from_secs(interval);
    let mut last_sample = Instant::now();
    let mut ticker = time::interval_at(last_sample + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let execution_result: Result<(), Box<dyn std::error::Error>> = async {
        // --- Main measurement loop ---
        loop {
            iteration += 1;

            // Wait for the next sample interval, accounting for any delays to maintain consistent timing
            ticker.tick().await;
            let now = Instant::now();
            let elapsed_secs = now.duration_since(last_sample).as_secs_f64();
            last_sample = now;

            // Refresh process data
            sys.refresh_processes(ProcessesToUpdate::All, true);

            // Check if the process is still running
            cpu_usage = if sys.process(pid).is_none() {
                println!("Process {} finished", pid);
                break;
            } else {
                // cpu_usage() returns total % across all cores; divide by num_cores
                // to get a normalized 0–100% value for the whole system
                sys.process(pid).unwrap().cpu_usage() / num_cores as f32
            };
            // Done: Add memory and Igpu handeling
            mem_usage = (sys.process(pid).unwrap().memory() as f64 / sys_memo as f64) * 100.0;

            // --- CPU energy calculation ---
            let cpu_energy_2 = get_all_energies(&mut rapl_readers)?;

            // Calculate delta energy per domain (cpu, dram, igpu) in Joules.
            // Each domain is weighted by its own process-level resource usage.
            // Domains not present on this system produce no entries, so mem_w / igpu_w
            // remain 0.0 automatically when those RAPL domains are unavailable.
            let delta_cpu_energy_j = delta_cpu_energy_per_pid_w(
                &cpu_energy_1,
                &cpu_energy_2,
                elapsed_secs,
                cpu_usage as f64,
                mem_usage,
                cpu_usage as f64, // igpu proxy: no per-process iGPU sensor available
            );

            // Map the delta energies to their respective domains (cpu, dram, igpu).
            // mem_j and igpu_j are None when those RAPL domains are absent on this system,
            // so the exported record clearly distinguishes "not available" from "zero usage".
            let (cpu_j, mem_j, igpu_j): (f64, Option<f64>, Option<f64>) = delta_cpu_energy_j
                .iter()
                .fold((0.0_f64, None, None), |mut acc, d| {
                    match d.domain.as_str() {
                        "cpu" => acc.0 += d.delta_energy,
                        "dram" => acc.1 = Some(acc.1.unwrap_or(0.0) + d.delta_energy),
                        "igpu" => acc.2 = Some(acc.2.unwrap_or(0.0) + d.delta_energy),
                        _ => {}
                    }
                    acc
                });

            // Build per-socket metrics from the RAPL deltas
            let socket_metrics: Vec<SocketMetric> = delta_cpu_energy_j
                .iter()
                .map(|d| SocketMetric {
                    socket: d.socket,
                    domain: d.domain.clone(),
                    energy: d.delta_energy,
                })
                .collect();

            // --- GPU energy calculation (multi-GPU) ---
            let gpu_energies_2 = get_all_gpu_energies(&devices)?;

            let gpu_results = get_all_gpu_energies_by_pid(
                &devices,
                pid.as_u32(),
                &gpu_energies_1, // in mJ
                &gpu_energies_2, // in mJ
                &gpu_timestamps,
                elapsed_secs,
                &gpu_max_energies,
            );

            // Build per-GPU metrics and compute aggregates
            let mut total_gpu_energy_j: f64 = 0.0;
            let mut total_gpu_util: f64 = 0.0;
            let mut gpu_metrics: Vec<GpuMetric> = Vec::with_capacity(gpu_count);

            for result in &gpu_results {
                total_gpu_energy_j += result.energy_j;
                total_gpu_util += result.util_pct;
                gpu_metrics.push(GpuMetric {
                    device_index: result.device_index,
                    gpu_usage: result.util_pct,
                    gpu_energy: result.energy_j,
                });
                gpu_timestamps[result.device_index as usize] = result.next_timestamp;
            }

            // Average utilization across GPUs for the aggregate field
            let avg_gpu_util = if gpu_count > 0 {
                total_gpu_util / gpu_count as f64
            } else {
                0.0
            };

            // Carry forward: the end-of-interval reading becomes the start of the next interval.
            // Update the per-GPU running maximum with the latest readings (only when no reset
            // occurred — a reset leaves gpu_energies_2[i] < gpu_energies_1[i], so max() keeps
            // the pre-reset peak intact for the next rollover correction).
            gpu_max_energies
                .iter_mut()
                .zip(gpu_energies_2.iter())
                .for_each(|(max, &e2)| *max = max.max(e2));
            cpu_energy_1 = cpu_energy_2;
            gpu_energies_1 = gpu_energies_2;

            // --- Export the measurement record ---
            let record = Record::new(
                iteration,
                run_id,
                pid.as_u32(),
                Utc::now().to_rfc3339(),
                cpu_usage as f64,
                cpu_j,
                avg_gpu_util,
                total_gpu_energy_j,
                Some((mem_usage * 100.0).round() / 100.0),
                mem_j,
                None, // igpu_usage — no per-process iGPU sensor available
                igpu_j,
                gpu_metrics,
                socket_metrics,
            );

            exporter.add_record(record).await?;
            exporter.export_line().await?;

            if cpu_usage <= 0.0 && total_gpu_util <= 0.0 && total_gpu_energy_j <= 0.0 {
                println!("Process finished");
                break;
            }
        }

        exporter.export().await?;
        Ok(())
    }
    .await;

    let run_ended_at = Utc::now().to_rfc3339();
    let final_status = if execution_result.is_ok() {
        "finished"
    } else {
        "failed"
    };

    exporter
        .complete_run(run_id, final_status, Some(run_ended_at.as_str()))
        .await?;
    exporter.run_summary(run_id).await?;
    execution_result?;

    Ok(())
}

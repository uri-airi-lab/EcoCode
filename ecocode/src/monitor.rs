//! Core measurement loop for the EcoCode CLI.
//!
//! Initialises sensors, spawns the target process, samples RAPL and NVML on
//! every tick, attributes energy to the process, exports records, and writes
//! the final run summary.

use nvml_wrapper::Nvml;
use sqlx::types::chrono::Utc;
use std::collections::HashMap;
use std::io;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, RefreshKind, System};
use tokio::time::{self, Instant, MissedTickBehavior};

/// Reads cumulative CPU ticks (utime + stime) for a single PID from /proc/[pid]/stat.
/// Returns None if the process no longer exists or the file can't be parsed.
fn read_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let after_comm = stat.get(stat.rfind(')')? + 2..)?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    if fields.len() < 13 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    Some(utime + stime)
}

/// Reads cumulative CPU ticks for all PIDs, returning a map of pid → total ticks.
fn read_all_cpu_ticks(pids: &[u32]) -> HashMap<u32, u64> {
    let mut ticks = HashMap::with_capacity(pids.len());
    for &pid in pids {
        if let Some(t) = read_cpu_ticks(pid) {
            ticks.insert(pid, t);
        }
    }
    ticks
}

/// Reads PSS (Proportional Set Size) from /proc/[pid]/smaps_rollup.
/// PSS correctly attributes shared pages proportionally across sharing processes.
fn read_pss_bytes(pid: u32) -> Option<u64> {
    let content = std::fs::read_to_string(format!("/proc/{}/smaps_rollup", pid)).ok()?;
    for line in content.lines() {
        if line.starts_with("Pss:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Returns true if this PID is a thread group leader (i.e. a real process, not a thread).
/// Threads share the same address space as their leader, so PSS must only be read
/// from leaders to avoid counting the same memory N times.
fn is_thread_leader(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{}/status", pid))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Tgid:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u32>().ok())
        })
        .map(|tgid| tgid == pid)
        .unwrap_or(false)
}

/// Shared state updated by the Python SDK via Unix socket (attached mode).
/// The main sampling loop reads these values on every tick and attaches them to the Record.
#[derive(Debug, Default, Clone)]
struct MarkerState {
    current_phase: Option<String>,
    tokens_processed: Option<i64>,
    training_step: Option<i64>,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    sequence_length: Option<i32>,
}

use crate::exporter::csv::CsvExporter;
use crate::exporter::dual::DualExporter;
use crate::exporter::json::JsonExporter;
use crate::exporter::local::SqliteExporter;
use crate::exporter::logfile::LogFileExporter;
use crate::exporter::online::OnlineExporter;
use crate::exporter::prometheus::PrometheusExporter;
use crate::exporter::terminal::TerminalExporter;
use crate::exporter::{Exporter, ExporterType, GpuMetric, Record, SocketMetric};
use ecocode_core::sensor::RAPL_PATH;
use ecocode_core::sensor::get_process_tree_pids_from_sys;
use ecocode_core::sensor::gpu::{
    get_all_gpu_devices, get_all_gpu_energies, get_all_gpu_energies_by_pid,
};
use ecocode_core::sensor::rapl::{delta_cpu_energy_per_pid_j, get_all_energies, scan_rapl_files};

use crate::cli::RunArgs;

/// Core monitoring loop: initialises sensors, spawns the target process, samples
/// energy on each tick, exports records, and writes the final run summary.
pub async fn run_monitoring(args: RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.interval < 0.1 {
        return Err("Sampling interval must be at least 0.1 seconds".into());
    }

    let output = args.output.to_lowercase();
    let project_name = args.project;
    let collaborator = match args.collaborator {
        Some(ref name) => {
            let trimmed = name.trim();
            if trimmed.len() > 0 {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        None => None,
    };

    // Request read access to RAPL energy counters (requires root on most systems).
    // Skipped entirely on machines without Intel RAPL (VMs, ARM, AMD without powercap).
    let needs_chmod = if std::path::Path::new(RAPL_PATH).exists() {
        scan_rapl_files().is_err()
    } else {
        false
    };

    if needs_chmod {
        println!("RAPL energy counters detected but not readable. Requesting read access...");
        println!("  sudo chmod -R +r /sys/class/powercap/intel-rapl/");
        println!("  (You may be prompted for your sudo password.)");
        match Command::new("sudo")
            .arg("chmod")
            .arg("-R")
            .arg("+r")
            .arg(RAPL_PATH)
            .status()
        {
            Ok(s) if s.success() => println!("  RAPL access granted."),
            Ok(_) => eprintln!(
                "Warning: sudo chmod returned a non-zero exit code.\n\
                 CPU energy may not be tracked. Run manually if needed:\n\
                   sudo chmod -R +r /sys/class/powercap/intel-rapl/"
            ),
            Err(e) => eprintln!(
                "Warning: could not run sudo ({e}).\n\
                 CPU energy may not be tracked. Run manually if needed:\n\
                   sudo chmod -R +r /sys/class/powercap/intel-rapl/"
            ),
        }
    }

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

    let mut exporter: Box<dyn Exporter> = if output == "online" {
        Box::new(OnlineExporter::new(args.api_key)?)
    } else {
        let primary: Box<dyn Exporter> = match output.as_str() {
            "terminal" => Box::new(TerminalExporter::new()),
            "csv" => Box::new(CsvExporter::new(require_file("csv"))?),
            "json" => Box::new(JsonExporter::new(require_file("json"))?),
            "local" => Box::new(SqliteExporter::new().await?),
            "prometheus" => Box::new(PrometheusExporter::new()?),
            "logfile" => Box::new(LogFileExporter::new(require_file("logfile"))?),
            _ => Box::new(TerminalExporter::new()),
        };
        let online = OnlineExporter::new(args.api_key)?;
        Box::new(DualExporter::new(primary, online))
    };
    if output == "online" && args.collaborator.is_some() {
        println!(
            "Notice: In 'online' mode, the collaborator name is automatically linked to your EcoCode account. The provided --collaborator value will be ignored."
        );
    }

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
    let run_name = args
        .name
        .unwrap_or_else(|| format!("run_{}", Utc::now().format("%Y-%m-%d_%H-%M-%S")));
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

    if args.intensity.is_some()
        || args.price.is_some()
        || args.address.is_some()
        || args.use_api_cost
    {
        exporter
            .update_project_config(
                project.id,
                args.intensity,
                args.price,
                args.address.clone(),
                if args.use_api_cost { Some(true) } else { None },
            )
            .await?;
    }

    let carbon_config = exporter.get_carbon_config(project.id).await?;

    if project.is_new
        || (carbon_config.manual_cost_per_kwh.is_none() && !carbon_config.use_api_for_cost)
    {
        if carbon_config.manual_cost_per_kwh.is_none()
            && !carbon_config.use_api_for_cost
            && args.price.is_none()
            && !args.use_api_cost
        {
            eprintln!(
                "Error: Financial cost per kWh (--price or --cost) or --use-api-cost is required for new projects or if not already set."
            );
            std::process::exit(1);
        }
        if project.is_new && args.intensity.is_none() {
            println!(
                "Notice: No --intensity provided. Using default global average ({:.1} gCO2/kWh).",
                carbon_config.resolved_intensity_g_per_kwh
            );
        }
    }

    let mut rapl_readers = scan_rapl_files()?;

    // --- Pipe-based marker IPC (attached mode) ---
    // Creates an OS pipe so the Python SDK running inside the child process
    // can send semantic marker events (phase, tokens, step, loss, lr, seq_len)
    // by writing newline-delimited JSON to the inherited write-end FD.
    let marker_state: Arc<Mutex<MarkerState>> = Arc::new(Mutex::new(MarkerState::default()));

    let (pipe_read_fd, pipe_write_fd) = os_pipe::pipe()?;
    let marker_state_for_task = Arc::clone(&marker_state);

    // Clear CLOEXEC on the write-end so the child inherits it across exec.
    // os_pipe sets CLOEXEC by default; we need the child to write markers to this FD.
    {
        use std::os::unix::io::AsRawFd;
        let raw = pipe_write_fd.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(raw, libc::F_GETFD);
            libc::fcntl(raw, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
        }
    }

    // Spawn a blocking reader thread for the pipe read-end.
    // The child process writes markers to pipe_write_fd; we read them here.
    std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(pipe_read_fd);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                        let mut state = marker_state_for_task.lock().unwrap();
                        match event.get("event").and_then(|e| e.as_str()) {
                            Some("phase") => {
                                state.current_phase =
                                    event["value"].as_str().map(|s| s.to_string());
                            }
                            Some("epoch") => {
                                let n = event["value"].as_i64().unwrap_or(0);
                                state.current_phase = Some(format!("epoch:{}", n));
                            }
                            Some("tokens") => {
                                state.tokens_processed = event["count"].as_i64();
                            }
                            Some("step") => {
                                state.training_step = event["value"].as_i64();
                            }
                            Some("loss") => {
                                state.loss = event["value"].as_f64();
                            }
                            Some("learning_rate") => {
                                state.learning_rate = event["value"].as_f64();
                            }
                            Some("sequence_length") => {
                                state.sequence_length =
                                    event["value"].as_i64().map(|n| n as i32);
                            }
                            // accuracy, custom, and other SDK markers are
                            // silently accepted — they don't map to the CLI's
                            // fixed-schema Record fields.
                            _ => {}
                        }
                    }
                }
                Err(_) => break, // pipe closed (child exited)
            }
        }
    });

    // --- Spawn the target process ---
    // Inject ECOCODE_MARKER_FD and ECOCODE_RUN_ID so the Python SDK running inside
    // the child process can attach to this run in attached mode.
    // The child inherits the pipe write-end FD and writes markers to it.
    let write_fd_raw = {
        use std::os::unix::io::AsRawFd;
        pipe_write_fd.as_raw_fd()
    };
    let mut child = Command::new(&args.command[0])
        .args(&args.command[1..])
        .env("ECOCODE_MARKER_FD", write_fd_raw.to_string())
        .env("ECOCODE_RUN_ID", run_id.to_string())
        .spawn()
        .map_err(|e| format!("Failed to execute '{}': {}", args.command[0], e))?;

    // Close the write-end in the parent — only the child writes markers.
    drop(pipe_write_fd);
    

    let child_pid_raw = child.id();
    let pid = Pid::from(child_pid_raw as usize);

    // Install a panic hook that kills the child process to avoid orphans.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = Command::new("kill")
            .args(["-9", &child_pid_raw.to_string()])
            .status();
        prev_hook(info);
    }));

    // --- System setup ---
    let mut sys = System::new_with_specifics(RefreshKind::everything());
    let num_cores = sys.cpus().len();
    let sys_memo = sys.total_memory();
    let mut cpu_usage = 0.0_f32;
    let mut mem_usage = 0.0_f64;

    // --- NVML / GPU setup (multi-GPU) ---
    let nvml = match Nvml::init() {
        Ok(n) => Some(n),
        Err(e) => {
            eprintln!("Warning: NVML initialization failed ({}). GPU metrics disabled.", e);
            None
        }
    };
    let devices = match &nvml {
        Some(n) => match get_all_gpu_devices(n) {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!("Warning: Failed to retrieve GPU devices ({}). GPU metrics disabled.", e);
                None
            }
        },
        None => None,
    };
    let gpu_count = devices.as_ref().map(|d| d.len()).unwrap_or(0);

    let mut gpu_names = Vec::new();
    if let Some(ref devs) = devices {
        for dev in devs {
            if let Ok(name) = dev.name() {
                gpu_names.push(name);
            }
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
    if let Some(cpu) = sys.cpus().first() {
        println!("║  CPU: {:44}║", cpu.brand().trim());
    }
    println!("║  Cores / Threads: {:32}║", format!("{}", num_cores));
    println!(
        "║  Total RAM: {:38}║",
        format!("{:.1} GB", sys_memo as f64 / 1_073_741_824.0)
    );

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

    println!("║  NVIDIA GPUs: {:36}║", gpu_count);
    if let Some(ref devs) = devices {
        for (i, dev) in devs.iter().enumerate() {
            if let Ok(name) = dev.name() {
                println!("║    GPU {}: {:40}║", i, name);
            }
        }
    }

    println!(
        "║  Exporter: {:38}║",
        format!("{:?}", exporter.exporter_type())
    );
    println!("║  Interval: {:38}║", format!("{:.1}s", args.interval));
    println!("╠════════════════════════════════════════════════════╣");
    println!(
        "║  Carbon Intensity: {:31}║",
        format!("{:.1} gCO2/kWh", carbon_config.resolved_intensity_g_per_kwh)
    );
    println!(
        "║  Intensity Source: {:31}║",
        carbon_config.intensity_source
    );
    println!(
        "║  PUE Factor: {:36}║",
        format!("{:.2}", carbon_config.pue_factor)
    );
    println!("╚════════════════════════════════════════════════════╝");
    println!();

    // --- Measurement state ---
    let mut iteration = 0;
    let mut gpu_timestamps: Vec<u64> = vec![0; gpu_count];

    // Placeholder initial readings — overwritten by the warm-up block below.
    #[allow(unused_assignments)]
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    #[allow(unused_assignments)]
    let mut cpu_energy_1 = get_all_energies(&mut rapl_readers)?;
    #[allow(unused_assignments)]
    let mut gpu_energies_1 = devices
        .as_ref()
        .and_then(|devs| get_all_gpu_energies(devs).ok())
        .unwrap_or_default();
    #[allow(unused_assignments)]
    let mut gpu_max_energies: Vec<f64> = gpu_energies_1.clone();

    // --- Warm-up pass ---
    // Fixes three first-sample artefacts:
    //   1. gpu_timestamps=0 makes NVML return ALL historical utilisation data,
    //      not just the last interval → inflated process_util on sample 1.
    //   2. RAPL/GPU counters include the process startup burst (CUDA context
    //      init, library loading) which would corrupt the first energy delta.
    //   3. sysinfo has no prior CPU snapshot, so the first cpu_usage() call
    //      spans from process spawn to tick 1 rather than a clean interval.
    {
        cpu_energy_1 = get_all_energies(&mut rapl_readers)?;
        gpu_energies_1 = devices
            .as_ref()
            .and_then(|devs| get_all_gpu_energies(devs).ok())
            .unwrap_or_default();
        gpu_max_energies = gpu_energies_1.clone();

        if let Some(ref devs) = devices {
            for (i, device) in devs.iter().enumerate() {
                if let Ok(stats) = device.process_utilization_stats(gpu_timestamps[i]) {
                    gpu_timestamps[i] = stats.iter().map(|s| s.timestamp).max().unwrap_or(0);
                }
            }
        }

        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }

    // --- CPU tick baseline (for interval-average CPU usage) ---
    let initial_tree = get_process_tree_pids_from_sys(&sys, pid.as_u32());
    let initial_leaders: Vec<u32> = initial_tree.iter().copied().filter(|&p| is_thread_leader(p)).collect();
    let mut prev_cpu_ticks = read_all_cpu_ticks(&initial_leaders);
    let mut prev_tick_time = std::time::Instant::now();

    let period = Duration::from_secs_f64(args.interval);
    let mut last_sample = Instant::now();
    let mut ticker = time::interval_at(last_sample + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut canceled = false;
    let execution_result: Result<(), Box<dyn std::error::Error>> = async {
        'monitoring: loop {
            tokio::select! {
                biased;
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("\nCtrl+C received — saving run data...");
                    canceled = true;
                    break 'monitoring;
                }
                _ = ticker.tick() => {}
            }

            iteration += 1;
            // Capture wall-clock timestamp before sensor work so that
            // consecutive timestamps are evenly spaced (~ticker interval).
            let tick_wall = Utc::now().to_rfc3339();

            let now = Instant::now();
            let elapsed_secs = now.duration_since(last_sample).as_secs_f64();
            last_sample = now;

            sys.refresh_processes(ProcessesToUpdate::All, true);

            // Check the root process is still alive
            if sys.process(pid).is_none() {
                println!("Process {} finished", pid);
                break;
            }

            // Build the full process tree (root + all descendants) so that child
            // processes (data-loader workers, torch.distributed ranks, etc.) are
            // included in CPU, memory, and GPU attribution.
            let tree_pids = get_process_tree_pids_from_sys(&sys, pid.as_u32());

            // --- CPU: interval-average from /proc/[pid]/stat tick deltas ---
            const CLK_TCK: f64 = 100.0;
            let now_tick = std::time::Instant::now();
            let tick_elapsed = now_tick.duration_since(prev_tick_time).as_secs_f64();
            let elapsed_ticks = tick_elapsed * CLK_TCK;

            // Only read ticks for thread group leaders (real processes).
            // /proc/[tgid]/stat returns aggregate utime+stime for all threads (whole=1).
            // /proc/[tid]/stat for non-leaders ALSO returns the group aggregate,
            // so including threads would count the same CPU time N times.
            let leaders: Vec<u32> = tree_pids.iter().copied().filter(|&p| is_thread_leader(p)).collect();
            let curr_cpu_ticks = read_all_cpu_ticks(&leaders);
            let mut total_delta_ticks = 0_u64;
            for (&p, &curr) in &curr_cpu_ticks {
                if let Some(&prev) = prev_cpu_ticks.get(&p) {
                    total_delta_ticks += curr.saturating_sub(prev);
                }
            }

            cpu_usage = if elapsed_ticks > 0.0 && num_cores > 0 {
                ((total_delta_ticks as f64 / elapsed_ticks) * 100.0 / num_cores as f64).min(100.0) as f32
            } else {
                0.0
            };

            prev_cpu_ticks = curr_cpu_ticks;
            prev_tick_time = now_tick;

            // --- Memory: PSS-sum for accurate shared-memory attribution ---
            // Only read PSS from thread group leaders (real processes), not threads.
            // Threads share the same address space, so /proc/[tid]/smaps_rollup
            // returns the same value as the leader — summing them would Nx the real memory.
            let mut pss_total_bytes = 0_u64;
            for &tpid in &tree_pids {
                if is_thread_leader(tpid) {
                    if let Some(pss) = read_pss_bytes(tpid) {
                        pss_total_bytes += pss;
                    }
                }
            }
            mem_usage = if sys_memo > 0 {
                ((pss_total_bytes as f64 / sys_memo as f64) * 100.0).min(100.0)
            } else {
                0.0
            };

            // --- CPU energy ---
            let cpu_energy_2 = get_all_energies(&mut rapl_readers)?;
            let delta_cpu_energy_j = delta_cpu_energy_per_pid_j(
                &cpu_energy_1,
                &cpu_energy_2,
                cpu_usage as f64,
                mem_usage,
                cpu_usage as f64, // igpu proxy: no per-process iGPU sensor available
            );

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

            let socket_metrics: Vec<SocketMetric> = delta_cpu_energy_j
                .iter()
                .map(|d| SocketMetric {
                    socket: d.socket,
                    domain: d.domain.clone(),
                    energy: d.delta_energy,
                })
                .collect();

            // --- GPU energy ---
            let gpu_energies_2 = devices
                .as_ref()
                .and_then(|devs| get_all_gpu_energies(devs).ok())
                .unwrap_or_default();
            let gpu_results = if let Some(ref devs) = devices {
                get_all_gpu_energies_by_pid(
                    devs,
                    &tree_pids,
                    &gpu_energies_1,
                    &gpu_energies_2,
                    &gpu_timestamps,
                    &gpu_max_energies,
                )
            } else {
                Vec::new()
            };

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

            let avg_gpu_util = if gpu_count > 0 {
                total_gpu_util / gpu_count as f64
            } else {
                0.0
            };

            // Carry forward: end-of-interval reading becomes start of next interval.
            // Only update max when no reset occurred — a reset leaves gpu_energies_2[i]
            // < gpu_energies_1[i], so max() keeps the pre-reset peak for rollover correction.
            gpu_max_energies
                .iter_mut()
                .zip(gpu_energies_2.iter())
                .for_each(|(max, &e2)| *max = max.max(e2));
            cpu_energy_1 = cpu_energy_2;
            gpu_energies_1 = gpu_energies_2;

            // --- Read SDK marker state ---
            let (
                current_phase,
                current_tokens,
                current_step,
                current_loss,
                current_lr,
                current_seqlen,
            ) = {
                let state = marker_state.lock().unwrap();
                (
                    state.current_phase.clone(),
                    state.tokens_processed,
                    state.training_step,
                    state.loss,
                    state.learning_rate,
                    state.sequence_length,
                )
            };

            // --- Export record ---
            let record = Record::new(
                iteration,
                run_id,
                pid.as_u32(),
                tick_wall,
                elapsed_secs,
                cpu_usage as f64,
                cpu_j,
                avg_gpu_util,
                total_gpu_energy_j,
                Some((mem_usage * 100.0).round() / 100.0),
                mem_j,
                None,
                igpu_j,
                gpu_metrics,
                socket_metrics,
                current_phase,
                current_tokens,
                current_step,
                current_loss,
                current_lr,
                current_seqlen,
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

    if canceled {
        let _ = child.kill();
        let _ = child.wait();
    }

    let run_ended_at = Utc::now().to_rfc3339();
    let final_status = if canceled {
        "canceled"
    } else if execution_result.is_ok() {
        "finished"
    } else {
        "failed"
    };

    exporter
        .complete_run(run_id, final_status, Some(run_ended_at.as_str()))
        .await?;
    exporter.run_summary(run_id).await?;

    // Restore the default panic hook now that the child is done.
    let _ = std::panic::take_hook();

    execution_result?;

    Ok(())
}

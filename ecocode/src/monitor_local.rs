//! Local headless energy monitor spawned by the Python SDK as a subprocess.
//!
//! This is the no-account, no-network alternative to `monitor_headless.rs`.
//! It reads semantic markers from stdin (newline-delimited JSON) and writes
//! combined energy + marker records to a local exporter (terminal, CSV, JSON,
//! or logfile) — no API key is required.
//!
//! The Python SDK handles run creation and completion by calling
//! `ecocode monitor-local` with `--run-id`, `--pid`, `--exporter`,
//! and optionally `--file` / `--db-path`.

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nvml_wrapper::Nvml;
use chrono::Utc;
use sysinfo::{Pid, ProcessesToUpdate, RefreshKind, System};
use tokio::time::{self, Instant, MissedTickBehavior};

use ecocode_core::sensor::get_process_tree_pids_from_sys;
use ecocode_core::sensor::gpu::{
    get_all_gpu_devices, get_all_gpu_energies, get_all_gpu_energies_by_pid,
};
use ecocode_core::sensor::rapl::{delta_cpu_energy_per_pid_j, get_all_energies, scan_rapl_files};

use crate::cli::MonitorLocalArgs;
use crate::exporter::csv::CsvExporter;
use crate::exporter::json::JsonExporter;
use crate::exporter::terminal::TerminalExporter;
use crate::exporter::logfile::LogFileExporter;
use crate::exporter::{Exporter, GpuMetric, Record, SocketMetric};

/// Shared marker state updated from stdin events (identical to monitor_headless).
#[derive(Debug, Default, Clone)]
struct MarkerState {
    current_phase: Option<String>,
    tokens_processed: Option<i64>,
    training_step: Option<i64>,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    sequence_length: Option<i32>,
    accuracy: Option<f64>,
    custom_metrics: HashMap<String, f64>,
}

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

fn read_all_cpu_ticks(pids: &[u32]) -> HashMap<u32, u64> {
    let mut ticks = HashMap::with_capacity(pids.len());
    for &pid in pids {
        if let Some(t) = read_cpu_ticks(pid) {
            ticks.insert(pid, t);
        }
    }
    ticks
}

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

/// Entry point for `ecocode monitor-local`. Spawned by the Python SDK.
/// Writes records to a local exporter — no network calls, no API key required.
pub async fn run_local_monitor(args: MonitorLocalArgs) -> Result<(), Box<dyn std::error::Error>> {
    let target_pid = args.pid;

    let require_file = |fmt: &str| -> Result<String, Box<dyn std::error::Error>> {
        args.file.clone().ok_or_else(|| {
            format!(
                "[ecocode monitor-local] --file is required for the '{}' exporter.",
                fmt
            )
            .into()
        })
    };

    let mut exporter: Box<dyn Exporter> = match args.exporter.to_lowercase().as_str() {
        "terminal"         => Box::new(TerminalExporter::new()),
        "csv"              => Box::new(CsvExporter::new(require_file("csv")?)?),
        "json"             => Box::new(JsonExporter::new(require_file("json")?)?),
        "logfile"          => Box::new(LogFileExporter::new(require_file("logfile")?)?),
        other => {
            eprintln!(
                "[ecocode monitor-local] Unknown exporter '{}', falling back to 'terminal'.",
                other
            );
            Box::new(TerminalExporter::new())
        }
    };

    // --- Marker reader thread (reads newline-delimited JSON from stdin) ---
    let marker_state: Arc<Mutex<MarkerState>> = Arc::new(Mutex::new(MarkerState::default()));
    let marker_state_for_stdin = Arc::clone(&marker_state);
    let stdin_closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stdin_closed_writer = Arc::clone(&stdin_closed);

    std::thread::spawn(move || {
        let stdin = io::stdin();
        let reader = stdin.lock();
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                        let mut state = marker_state_for_stdin.lock().unwrap();
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
                            Some("accuracy") => {
                                state.accuracy = event["value"].as_f64();
                            }
                            Some("custom") => {
                                if let (Some(key), Some(value)) = (
                                    event["key"].as_str(),
                                    event["value"].as_f64(),
                                ) {
                                    state.custom_metrics.insert(key.to_string(), value);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => break,
            }
        }
        stdin_closed_writer.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    // --- Sensor initialisation ---
    let mut rapl_readers = scan_rapl_files()?;
    let nvml = match Nvml::init() {
        Ok(n) => Some(n),
        Err(e) => {
            eprintln!(
                "[ecocode monitor-local] NVML init failed ({}). GPU metrics disabled.",
                e
            );
            None
        }
    };
    let devices = match &nvml {
        Some(n) => match get_all_gpu_devices(n) {
            Ok(d) => Some(d),
            Err(e) => {
                eprintln!(
                    "[ecocode monitor-local] Failed to get GPU devices ({}). GPU metrics disabled.",
                    e
                );
                None
            }
        },
        None => None,
    };
    let gpu_count = devices.as_ref().map(|d| d.len()).unwrap_or(0);

    let pid = Pid::from(target_pid as usize);
    let mut sys = System::new_with_specifics(RefreshKind::everything());
    let num_cores = sys.cpus().len();
    let sys_memo = sys.total_memory();

    // --- Warm-up pass ---
    let mut cpu_energy_1 = get_all_energies(&mut rapl_readers)?;
    let mut gpu_energies_1 = devices
        .as_ref()
        .and_then(|devs| get_all_gpu_energies(devs).ok())
        .unwrap_or_default();
    let mut gpu_max_energies: Vec<f64> = gpu_energies_1.clone();
    let mut gpu_timestamps: Vec<u64> = vec![0; gpu_count];

    if let Some(ref devs) = devices {
        for (i, device) in devs.iter().enumerate() {
            if let Ok(stats) = device.process_utilization_stats(gpu_timestamps[i]) {
                gpu_timestamps[i] = stats.iter().map(|s| s.timestamp).max().unwrap_or(0);
            }
        }
    }

    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    let initial_tree = get_process_tree_pids_from_sys(&sys, target_pid);
    let initial_leaders: Vec<u32> = initial_tree
        .iter()
        .copied()
        .filter(|&p| is_thread_leader(p))
        .collect();
    let mut prev_cpu_ticks = read_all_cpu_ticks(&initial_leaders);
    let mut prev_tick_time = std::time::Instant::now();

    let clk_tck = {
        let tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if tck > 0 { tck as f64 } else { 100.0 }
    };

    let run_id = if args.run_id == 0 {
        let project_name = "ecocode";
        let project = exporter.get_or_create_project(project_name, Some("unassigned")).await?;
        let run_name = format!("run_{}", Utc::now().format("%Y-%m-%d_%H-%M-%S"));
        let run_id = exporter.create_run(
            &run_name,
            project.id,
            Some("unassigned"),
            Some(Utc::now().to_rfc3339().as_str()),
            None,
        ).await?;

        let cpu_name = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let mut gpu_names = Vec::new();
        if let Some(ref devs) = devices {
            for dev in devs {
                if let Ok(name) = dev.name() {
                    gpu_names.push(name);
                }
            }
        }
        let sys_info = crate::exporter::SystemInfo {
            cpu_name,
            cpu_cores: num_cores as u32,
            ram_gb: sys_memo as f64 / 1_073_741_824.0,
            gpu_count: gpu_count as u32,
            gpu_names,
        };
        exporter.save_system_info(run_id, sys_info).await?;
        run_id
    } else {
        args.run_id
    };

    let period = Duration::from_secs_f64(args.interval);
    let mut last_sample = Instant::now();
    let mut ticker = time::interval_at(last_sample + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut iteration: u32 = 0;
    let mut cpu_usage: f32;
    let mut mem_usage: f64;

    // --- Main sampling loop ---
    loop {
        ticker.tick().await;

        let tick_wall = Utc::now().to_rfc3339();
        let now = Instant::now();
        let elapsed_secs = now.duration_since(last_sample).as_secs_f64();
        last_sample = now;

        sys.refresh_processes(ProcessesToUpdate::All, true);

        let pid_alive = sys.process(pid).is_some();
        let stdin_done = stdin_closed.load(std::sync::atomic::Ordering::SeqCst);

        if !pid_alive {
            break;
        }

        let tree_pids = get_process_tree_pids_from_sys(&sys, target_pid);

        // --- CPU ---
        let now_tick = std::time::Instant::now();
        let tick_elapsed = now_tick.duration_since(prev_tick_time).as_secs_f64();
        let elapsed_ticks = tick_elapsed * clk_tck;

        let leaders: Vec<u32> = tree_pids
            .iter()
            .copied()
            .filter(|&p| is_thread_leader(p))
            .collect();
        let curr_cpu_ticks = read_all_cpu_ticks(&leaders);
        let mut total_delta_ticks = 0_u64;
        for (&p, &curr) in &curr_cpu_ticks {
            if let Some(&prev) = prev_cpu_ticks.get(&p) {
                total_delta_ticks += curr.saturating_sub(prev);
            }
        }
        cpu_usage = if elapsed_ticks > 0.0 && num_cores > 0 {
            ((total_delta_ticks as f64 / elapsed_ticks) * 100.0 / num_cores as f64).min(100.0)
                as f32
        } else {
            0.0
        };
        prev_cpu_ticks = curr_cpu_ticks;
        prev_tick_time = now_tick;

        // --- Memory (PSS) ---
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
            cpu_usage as f64,
        );
        let (cpu_j, mem_j, igpu_j): (f64, Option<f64>, Option<f64>) = delta_cpu_energy_j
            .iter()
            .fold((0.0_f64, None, None), |mut acc, d| {
                match d.domain.as_str() {
                    "cpu"  => acc.0 += d.delta_energy,
                    "dram" => acc.1 = Some(acc.1.unwrap_or(0.0) + d.delta_energy),
                    "igpu" => acc.2 = Some(acc.2.unwrap_or(0.0) + d.delta_energy),
                    _      => {}
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

        gpu_max_energies
            .iter_mut()
            .zip(gpu_energies_2.iter())
            .for_each(|(max, &e2)| *max = max.max(e2));
        cpu_energy_1 = cpu_energy_2;
        gpu_energies_1 = gpu_energies_2;

        // --- Read marker state ---
        let (phase, tokens, step, loss, lr, seqlen) = {
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

        // --- Build and export record ---
        iteration += 1;
        let record = Record::new(
            iteration,
            run_id,
            target_pid,
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
            phase,
            tokens,
            step,
            loss,
            lr,
            seqlen,
        );

        if let Err(e) = exporter.add_record(record).await {
            eprintln!("[ecocode monitor-local] Failed to write record: {}", e);
        }
        if let Err(e) = exporter.export_line().await {
            eprintln!("[ecocode monitor-local] export_line error: {}", e);
        }

        if stdin_done {
            break;
        }
    }

    // Flush final export
    if let Err(e) = exporter.export().await {
        eprintln!("[ecocode monitor-local] Final export error: {}", e);
    }

    Ok(())
}

//! Headless energy monitor spawned by the Python SDK as a subprocess.
//!
//! Reads semantic markers from stdin (newline-delimited JSON) and posts
//! combined energy + marker records to the backend. The Python SDK handles
//! run creation and completion — this module only samples and reports.

use ecocode_core::http_client::EcoCodeHttpClient;
use ecocode_core::sensor::get_process_tree_pids_from_sys;
use ecocode_core::sensor::gpu::{
    get_all_gpu_devices, get_all_gpu_energies, get_all_gpu_energies_by_pid,
};
use ecocode_core::sensor::rapl::{delta_cpu_energy_per_pid_j, get_all_energies, scan_rapl_files};
use nvml_wrapper::Nvml;
use sqlx::types::chrono::Utc;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Pid, ProcessesToUpdate, RefreshKind, System};
use tokio::time::{self, Instant, MissedTickBehavior};

use crate::cli::MonitorArgs;

/// Shared state updated from stdin marker events.
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

/// Reads cumulative CPU ticks (utime + stime) for a single PID from /proc/[pid]/stat.
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

/// Entry point for `ecocode monitor`. Spawned by the Python SDK as a subprocess.
pub async fn run_headless_monitor(args: MonitorArgs) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = args.run_id;
    let target_pid = args.pid;

    // Resolve API credentials from environment
    let api_key = std::env::var("ECOCODE_API_KEY")
        .map_err(|_| "ECOCODE_API_KEY environment variable is required")?;
    let base_url =
        std::env::var("ECOCODE_API_URL").unwrap_or_else(|_| "https://ecocode.app".to_string());

    let client = EcoCodeHttpClient::new(&base_url, &api_key)?;

    // --- Marker reader task (reads from stdin) ---
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
                Err(_) => break, // stdin closed (EOF or pipe broken)
            }
        }
        stdin_closed_writer.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    // --- Sensor initialisation ---
    let mut rapl_readers = scan_rapl_files()?;
    let nvml = Nvml::init()?;
    let devices = get_all_gpu_devices(&nvml)?;
    let gpu_count = devices.len();

    let pid = Pid::from(target_pid as usize);
    let mut sys = System::new_with_specifics(RefreshKind::everything());
    let num_cores = sys.cpus().len();
    let sys_memo = sys.total_memory();

    // --- Warm-up pass ---
    let mut cpu_energy_1 = get_all_energies(&mut rapl_readers)?;
    let mut gpu_energies_1 = get_all_gpu_energies(&devices)?;
    let mut gpu_max_energies: Vec<f64> = gpu_energies_1.clone();
    let mut gpu_timestamps: Vec<u64> = vec![0; gpu_count];

    for (i, device) in devices.iter().enumerate() {
        if let Ok(stats) = device.process_utilization_stats(gpu_timestamps[i]) {
            gpu_timestamps[i] = stats.iter().map(|s| s.timestamp).max().unwrap_or(0);
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

    let period = Duration::from_secs_f64(args.interval);
    let mut last_sample = Instant::now();
    let mut ticker = time::interval_at(last_sample + period, period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

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

        // Exit if the target process is gone AND stdin is closed
        let pid_alive = sys.process(pid).is_some();
        let stdin_done = stdin_closed.load(std::sync::atomic::Ordering::SeqCst);

        if !pid_alive || stdin_done {
            // Do one final sample before exiting if PID is still alive
            if !pid_alive {
                break;
            }
        }

        // Build process tree
        let tree_pids = get_process_tree_pids_from_sys(&sys, target_pid);

        // --- CPU usage ---
        const CLK_TCK: f64 = 100.0;
        let now_tick = std::time::Instant::now();
        let tick_elapsed = now_tick.duration_since(prev_tick_time).as_secs_f64();
        let elapsed_ticks = tick_elapsed * CLK_TCK;

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

        // --- Memory: PSS ---
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

        let (cpu_j, mem_j, igpu_j): (f64, Option<f64>, Option<f64>) =
            delta_cpu_energy_j
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

        let socket_metrics: Vec<ecocode_core::http_client::SocketMetric> = delta_cpu_energy_j
            .iter()
            .map(|d| ecocode_core::http_client::SocketMetric {
                socket: d.socket,
                domain: d.domain.clone(),
                energy: d.delta_energy,
            })
            .collect();

        // --- GPU energy ---
        let gpu_energies_2 = get_all_gpu_energies(&devices)?;
        let gpu_results = get_all_gpu_energies_by_pid(
            &devices,
            &tree_pids,
            &gpu_energies_1,
            &gpu_energies_2,
            &gpu_timestamps,
            &gpu_max_energies,
        );

        let mut total_gpu_energy_j: f64 = 0.0;
        let mut total_gpu_util: f64 = 0.0;
        let mut gpu_metrics: Vec<ecocode_core::http_client::GpuMetric> =
            Vec::with_capacity(gpu_count);

        for result in &gpu_results {
            total_gpu_energy_j += result.energy_j;
            total_gpu_util += result.util_pct;
            gpu_metrics.push(ecocode_core::http_client::GpuMetric {
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
        let (phase, tokens, step, loss, lr, seqlen, accuracy, custom) = {
            let state = marker_state.lock().unwrap();
            (
                state.current_phase.clone(),
                state.tokens_processed,
                state.training_step,
                state.loss,
                state.learning_rate,
                state.sequence_length,
                state.accuracy,
                if state.custom_metrics.is_empty() {
                    None
                } else {
                    Some(state.custom_metrics.clone())
                },
            )
        };

        // --- Build and send record ---
        let record = ecocode_core::http_client::Record {
            pid: target_pid,
            timestamp: tick_wall,
            elapsed_secs: Some(elapsed_secs),
            cpu_usage: cpu_usage as f64,
            cpu_energy: cpu_j,
            gpu_usage: avg_gpu_util,
            gpu_energy: total_gpu_energy_j,
            mem_usage,
            mem_energy: mem_j,
            igpu_usage: None,
            igpu_energy: igpu_j,
            gpu_metrics,
            socket_metrics,
            phase,
            epoch_num: None,
            tokens_processed: tokens,
            training_step: step,
            loss,
            learning_rate: lr,
            sequence_length: seqlen,
            accuracy,
            custom_metrics: custom,
        };

        if let Err(e) = client.add_record(run_id, &record) {
            eprintln!("[ecocode monitor] Failed to send record: {}", e);
        }

        // Exit after final record if stdin closed
        if stdin_done {
            break;
        }
    }

    Ok(())
}

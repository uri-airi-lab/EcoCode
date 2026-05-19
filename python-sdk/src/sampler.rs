//! Background sampling thread for standalone mode.
//!
//! Replaces the Python `_sampler.py` entirely. Uses:
//! - `ecocode_core::sensor::rapl` — RAPL sysfs reader (shared with CLI)
//! - `ecocode_core::sensor::gpu`  — NVML GPU reader (shared with CLI)
//! - `ecocode_core::http_client`  — blocking HTTP client
//! - `sysinfo`                    — CPU/memory usage (replaces psutil)

use std::collections::HashMap;
use std::fs;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ecocode_core::http_client::{EcoCodeHttpClient, GpuMetric, Record, SocketMetric};
use ecocode_core::sensor::rapl;

#[cfg(feature = "gpu")]
use ecocode_core::sensor::gpu;

use sysinfo::{Pid, System};

/// Reads cumulative CPU ticks (utime + stime) for a single PID from /proc/[pid]/stat.
/// Returns None if the process no longer exists or the file can't be parsed.
fn read_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    // Field 2 (comm) is wrapped in parentheses and may contain spaces/parens.
    // Find the last ')' to safely skip it, then split the remaining fields.
    let after_comm = stat.get(stat.rfind(')')? + 2..)?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // After comm: field index 0 = state, ..., 11 = utime, 12 = stime
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
/// Returns None if the process no longer exists or the file can't be read.
fn read_pss_bytes(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{}/smaps_rollup", pid)).ok()?;
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
    fs::read_to_string(format!("/proc/{}/status", pid))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Tgid:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u32>().ok())
        })
        .map(|tgid| tgid == pid)
        .unwrap_or(false)
}

/// ML marker state — updated from the Python thread, read by the sampler thread.
#[derive(Debug, Default, Clone)]
struct MarkerState {
    phase: Option<String>,
    epoch: Option<i64>,
    tokens_processed: Option<i64>,
    training_step: Option<i64>,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    sequence_length: Option<i32>,
    accuracy: Option<f64>,
    custom_metrics: Option<HashMap<String, f64>>,
}

/// Handle to a running sampler thread.
pub struct SamplerHandle {
    stop_tx: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
    reporter_thread: Option<JoinHandle<()>>,
    markers: Arc<Mutex<MarkerState>>,
}

impl SamplerHandle {
    /// Spawn the sampler and reporter daemon threads.
    pub fn start(client: Arc<EcoCodeHttpClient>, run_id: i64, interval: f64, pid: u32) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (reporter_tx, reporter_rx) = mpsc::channel::<Record>();
        let markers = Arc::new(Mutex::new(MarkerState::default()));
        let markers_clone = Arc::clone(&markers);

        // 1. Start Reporter Thread (handles blocking HTTP calls)
        let client_clone = Arc::clone(&client);
        let reporter_thread = thread::Builder::new()
            .name("ecocode-reporter".into())
            .spawn(move || {
                while let Ok(record) = reporter_rx.recv() {
                    if let Err(e) = client_clone.add_record(run_id, &record) {
                        eprintln!("[ecocode] Failed to send record: {}", e);
                    }
                }
            })
            .expect("ecocode: failed to spawn reporter thread");

        // 2. Start Sampler Thread (handles hardware reads)
        let handle = thread::Builder::new()
            .name("ecocode-sampler".into())
            .spawn(move || {
                sampler_loop(
                    client,
                    run_id,
                    interval,
                    pid,
                    stop_rx,
                    markers_clone,
                    reporter_tx,
                );
            })
            .expect("ecocode: failed to spawn sampler thread");

        Self {
            stop_tx,
            thread: Some(handle),
            reporter_thread: Some(reporter_thread),
            markers,
        }
    }

    /// Stop the sampler and reporter threads.
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        // Reporter thread will exit when the sender (sampler_loop) is dropped.
        if let Some(handle) = self.reporter_thread.take() {
            let _ = handle.join();
        }
    }

    // ── Marker setters (called from the Python/main thread) ──────────

    pub fn set_phase(&self, phase: &str) {
        self.markers.lock().unwrap().phase = Some(phase.to_string());
    }

    pub fn set_epoch(&self, epoch: i64) {
        self.markers.lock().unwrap().epoch = Some(epoch);
    }

    pub fn set_tokens(&self, count: i64) {
        self.markers.lock().unwrap().tokens_processed = Some(count);
    }

    pub fn set_training_step(&self, step: i64) {
        self.markers.lock().unwrap().training_step = Some(step);
    }

    pub fn set_loss(&self, loss: f64) {
        self.markers.lock().unwrap().loss = Some(loss);
    }

    pub fn set_learning_rate(&self, lr: f64) {
        self.markers.lock().unwrap().learning_rate = Some(lr);
    }

    pub fn set_sequence_length(&self, seq_len: i32) {
        self.markers.lock().unwrap().sequence_length = Some(seq_len);
    }
    pub fn set_accuracy(&self, accuracy: f64) {
        self.markers.lock().unwrap().accuracy = Some(accuracy);
    }

    pub fn set_custom_metric(&self, key: String, value: f64) {
        let mut state = self.markers.lock().unwrap();
        state
            .custom_metrics
            .get_or_insert_with(HashMap::new)
            .insert(key, value);
    }
}

/// The main sampling loop — runs in its own thread.
fn sampler_loop(
    client: Arc<EcoCodeHttpClient>,
    run_id: i64,
    interval: f64,
    pid: u32,
    stop_rx: mpsc::Receiver<()>,
    markers: Arc<Mutex<MarkerState>>,
    reporter_tx: mpsc::Sender<Record>,
) {
    let timeout = Duration::from_secs_f64(interval);

    // ── System info via sysinfo ──────────────────────────────────────
    let mut sys = System::new();
    let total_mem = sys.total_memory() as f64;
    // Prime the CPU usage counter for the whole system
    sys.refresh_all();

    let mut tree_pids = ecocode_core::sensor::get_process_tree_pids_from_sys(&sys, pid);
    let mut last_tree_refresh = std::time::Instant::now();

    // ── CPU tick baseline (for interval-average CPU usage) ──────────────────
    let initial_leaders: Vec<u32> = tree_pids.iter().copied().filter(|&p| is_thread_leader(p)).collect();
    let mut prev_cpu_ticks = read_all_cpu_ticks(&initial_leaders);
    let num_cpus = sys.cpus().len().max(1);
    let mut prev_tick_time = std::time::Instant::now();

    // ── Energy reading state ────────────────────────────────────────────────────
    let rapl_path = std::path::Path::new(ecocode_core::sensor::RAPL_PATH);
    let mut rapl_readers = match rapl::scan_rapl_files() {
        Ok(readers) => readers,
        Err(e) => {
            if rapl_path.exists() {
                // Hardware is present but unreadable — almost always a permissions issue.
                let is_permission_denied = e
                    .downcast_ref::<std::io::Error>()
                    .map_or(false, |io_err| {
                        io_err.kind() == std::io::ErrorKind::PermissionDenied
                    });
                if is_permission_denied {
                    eprintln!(
                        "\n[ecocode] RAPL energy files are not readable (permission denied).\n\
                         CPU energy will not be tracked in this run.\n\n\
                         To enable CPU energy monitoring, run the following command once\n\
                         and then restart your script:\n\n\
                             sudo chmod -R +r /sys/class/powercap/intel-rapl/\n"
                    );
                } else {
                    eprintln!("[ecocode] RAPL init failed: {e}. CPU energy will not be tracked.");
                }
            }
            // RAPL path absent → non-Intel CPU or VM; skip silently.
            Vec::new()
        }
    };
    let rapl_available = !rapl_readers.is_empty();

    // Prime RAPL counters so the first delta is 0 instead of garbage.
    let mut rapl_prev = if rapl_available {
        rapl::get_all_energies(&mut rapl_readers).ok()
    } else {
        None
    };

    // ── NVML / GPU init ──────────────────────────────────────────────
    #[cfg(feature = "gpu")]
    let nvml = nvml_wrapper::Nvml::init().ok();

    #[cfg(feature = "gpu")]
    let devices = nvml
        .as_ref()
        .and_then(|n| gpu::get_all_gpu_devices(n).ok())
        .unwrap_or_default();

    #[cfg(feature = "gpu")]
    let gpu_count = devices.len();

    #[cfg(feature = "gpu")]
    let mut gpu_timestamps: Vec<u64> = vec![0; gpu_count];

    #[cfg(feature = "gpu")]
    let mut gpu_energies_prev = devices.iter().map(|_| 0.0f64).collect::<Vec<_>>();

    #[cfg(feature = "gpu")]
    let mut gpu_max_energies = gpu_energies_prev.clone();

    // Prime GPU energy counters.
    #[cfg(feature = "gpu")]
    {
        if let Ok(energies) = gpu::get_all_gpu_energies(&devices) {
            gpu_energies_prev = energies.clone();
            gpu_max_energies = energies;
        }
        // Prime GPU timestamps to avoid first-sample inflation.
        for (i, device) in devices.iter().enumerate() {
            if let Ok(stats) = device.process_utilization_stats(gpu_timestamps[i]) {
                gpu_timestamps[i] = stats.iter().map(|s| s.timestamp).max().unwrap_or(0);
            }
        }
    }

    let mut next_tick = std::time::Instant::now() + timeout;
    let _start_instant = std::time::Instant::now();
    let base_utc = chrono::Utc::now();

    // ── Sampling loop ────────────────────────────────────────────────
    loop {
        let now = std::time::Instant::now();
        let wait_time = next_tick.saturating_duration_since(now);

        match stop_rx.recv_timeout(wait_time) {
            Ok(()) => break,                           // stop signal received
            Err(mpsc::RecvTimeoutError::Timeout) => {} // normal tick
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        next_tick += timeout;
        let now_after_wait = std::time::Instant::now();
        if next_tick <= now_after_wait {
            next_tick = now_after_wait + timeout;
        }

        // Calculate a perfectly accurate timestamp based on the monotonic real-time passage.
        // This ensures both monotonicity (no negative values) and physical accuracy.
        let elapsed = _start_instant.elapsed();
        let ts_duration =
            chrono::Duration::from_std(elapsed).unwrap_or_else(|_| chrono::Duration::zero());
        let timestamp = (base_utc + ts_duration).to_rfc3339();

        // ── Refresh process metrics for the entire tree ──────────────
        let work_start = std::time::Instant::now();

        // Refresh sysinfo for process tree discovery
        if last_tree_refresh.elapsed() > Duration::from_secs(1) {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            tree_pids = ecocode_core::sensor::get_process_tree_pids_from_sys(&sys, pid);
            last_tree_refresh = std::time::Instant::now();
        }

        // ── CPU: interval-average from /proc/[pid]/stat tick deltas ─────
        const CLK_TCK: f64 = 100.0;
        let now_tick = std::time::Instant::now();
        let elapsed_secs = now_tick.duration_since(prev_tick_time).as_secs_f64();
        let elapsed_ticks = elapsed_secs * CLK_TCK;

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

        let cpu_usage = if elapsed_ticks > 0.0 && num_cpus > 0 {
            ((total_delta_ticks as f64 / elapsed_ticks) * 100.0 / num_cpus as f64).min(100.0)
        } else {
            0.0
        };

        prev_cpu_ticks = curr_cpu_ticks;
        prev_tick_time = now_tick;

        // ── Memory: PSS-sum for accurate shared-memory attribution ──────
        // Only read PSS from thread group leaders (real processes), not threads.
        // Threads share the same address space — reading their smaps_rollup
        // returns the same value as the leader, so summing them Nx's the real memory.
        let mut pss_total_bytes = 0_u64;
        for &p_id in &tree_pids {
            if is_thread_leader(p_id) {
                if let Some(pss) = read_pss_bytes(p_id) {
                    pss_total_bytes += pss;
                }
            }
        }
        let mem_usage = if total_mem > 0.0 {
            ((pss_total_bytes as f64 / total_mem) * 100.0).min(100.0)
        } else {
            0.0
        };

        let sensor_work_done = std::time::Instant::now();

        // ── CPU / DRAM energy via RAPL ───────────────────────────────
        let mut cpu_energy_j = 0.0f64;
        let mut mem_energy_j: Option<f64> = None;
        let mut igpu_energy_j: Option<f64> = None;
        let mut socket_metrics: Vec<SocketMetric> = Vec::new();

        if rapl_available {
            if let Ok(rapl_curr) = rapl::get_all_energies(&mut rapl_readers) {
                if let Some(ref prev) = rapl_prev {
                    let deltas = rapl::delta_cpu_energy_per_pid_j(
                        prev, &rapl_curr, cpu_usage, mem_usage,
                        cpu_usage, // igpu proxy
                    );
                    for d in &deltas {
                        match d.domain.as_str() {
                            "cpu" => cpu_energy_j += d.delta_energy,
                            "dram" => {
                                mem_energy_j = Some(mem_energy_j.unwrap_or(0.0) + d.delta_energy)
                            }
                            "igpu" => {
                                igpu_energy_j = Some(igpu_energy_j.unwrap_or(0.0) + d.delta_energy)
                            }
                            _ => {}
                        }
                        socket_metrics.push(SocketMetric {
                            socket: d.socket,
                            domain: d.domain.clone(),
                            energy: d.delta_energy,
                        });
                    }
                }
                rapl_prev = Some(rapl_curr);
            }
        }

        // ── GPU energy via NVML ──────────────────────────────────────
        let mut gpu_energy_j = 0.0f64;
        let mut gpu_usage_total = 0.0f64;
        let mut gpu_metrics: Vec<GpuMetric> = Vec::new();

        #[cfg(feature = "gpu")]
        {
            if let Ok(gpu_energies_curr) = gpu::get_all_gpu_energies(&devices) {
                let results = gpu::get_all_gpu_energies_by_pid(
                    &devices,
                    &tree_pids,
                    &gpu_energies_prev,
                    &gpu_energies_curr,
                    &gpu_timestamps,
                    &gpu_max_energies,
                );

                for result in &results {
                    gpu_energy_j += result.energy_j;
                    gpu_usage_total += result.util_pct;
                    gpu_metrics.push(GpuMetric {
                        device_index: result.device_index,
                        gpu_usage: result.util_pct,
                        gpu_energy: result.energy_j,
                    });
                    gpu_timestamps[result.device_index as usize] = result.next_timestamp;
                }

                // Carry forward — update max and prev
                gpu_max_energies
                    .iter_mut()
                    .zip(gpu_energies_curr.iter())
                    .for_each(|(max, &e2)| *max = max.max(e2));
                gpu_energies_prev = gpu_energies_curr;
            }
        }

        #[cfg(feature = "gpu")]
        let avg_gpu_usage = if gpu_count > 0 {
            gpu_usage_total / gpu_count as f64
        } else {
            0.0
        };

        #[cfg(not(feature = "gpu"))]
        let avg_gpu_usage = 0.0;

        // ── Snapshot ML markers ──────────────────────────────────────
        let state = markers.lock().unwrap().clone();

        // ── Build and send record ────────────────────────────────────
        // We use the timestamp captured right after the sleep to avoid jitter
        // caused by the variable duration of the sensor reads.

        let record = Record {
            pid,
            timestamp,
            elapsed_secs: Some(elapsed_secs),
            cpu_usage,
            cpu_energy: cpu_energy_j,
            gpu_usage: avg_gpu_usage,
            gpu_energy: gpu_energy_j,
            mem_usage,
            mem_energy: mem_energy_j,
            igpu_usage: None,
            igpu_energy: igpu_energy_j,
            gpu_metrics,
            socket_metrics,
            phase: state.phase,
            epoch_num: state.epoch,
            tokens_processed: state.tokens_processed,
            training_step: state.training_step,
            loss: state.loss,
            learning_rate: state.learning_rate,
            sequence_length: state.sequence_length,
            accuracy: state.accuracy,
            custom_metrics: state.custom_metrics,
        };

        // Send record to reporter thread (non-blocking)
        let _ = reporter_tx.send(record);

        let loop_end = std::time::Instant::now();
        let sensor_time = sensor_work_done.duration_since(work_start).as_secs_f64();
        let total_work_time = loop_end.duration_since(work_start).as_secs_f64();

        if total_work_time > interval {
            println!(
                "[ecocode warning] Sampling loop took {:.3}s (Sensors: {:.3}s), which is slower than your interval of {:.3}s. Granularity will be reduced.",
                total_work_time, sensor_time, interval
            );
        }
    }
}

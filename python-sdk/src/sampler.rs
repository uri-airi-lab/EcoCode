//! Background sampling thread for standalone mode.
//!
//! Replaces the Python `_sampler.py` entirely. Uses:
//! - `ecocode_core::sensor::rapl` — RAPL sysfs reader (shared with CLI)
//! - `ecocode_core::sensor::gpu`  — NVML GPU reader (shared with CLI)
//! - `ecocode_core::http_client`  — blocking HTTP client
//! - `sysinfo`                    — CPU/memory usage (replaces psutil)

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ecocode_core::http_client::{EcoCodeHttpClient, GpuMetric, Record, SocketMetric};
use ecocode_core::sensor::rapl;

#[cfg(feature = "gpu")]
use ecocode_core::sensor::gpu;

use sysinfo::{Pid, System};

/// ML marker state — updated from the Python thread, read by the sampler thread.
#[derive(Debug, Default, Clone)]
struct MarkerState {
    phase: Option<String>,
    tokens_processed: Option<i64>,
    training_step: Option<i64>,
    loss: Option<f64>,
    learning_rate: Option<f64>,
    sequence_length: Option<i32>,
    accuracy: Option<f64>,
}

/// Handle to a running sampler thread.
pub struct SamplerHandle {
    stop_tx: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
    markers: Arc<Mutex<MarkerState>>,
}

impl SamplerHandle {
    /// Spawn the sampler daemon thread.
    pub fn start(
        client: Arc<EcoCodeHttpClient>,
        run_id: i64,
        interval: f64,
        pid: u32,
    ) -> Self {
        let (stop_tx, stop_rx) = mpsc::channel();
        let markers = Arc::new(Mutex::new(MarkerState::default()));
        let markers_clone = Arc::clone(&markers);

        let handle = thread::Builder::new()
            .name("ecocode-sampler".into())
            .spawn(move || {
                sampler_loop(client, run_id, interval, pid, stop_rx, markers_clone);
            })
            .expect("failed to spawn sampler thread");

        Self {
            stop_tx,
            thread: Some(handle),
            markers,
        }
    }

    /// Stop the sampler thread and wait for it to finish.
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    // ── Marker setters (called from the Python/main thread) ──────────

    pub fn set_phase(&self, phase: &str) {
        self.markers.lock().unwrap().phase = Some(phase.to_string());
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
}

/// The main sampling loop — runs in its own thread.
fn sampler_loop(
    client: Arc<EcoCodeHttpClient>,
    run_id: i64,
    interval: f64,
    pid: u32,
    stop_rx: mpsc::Receiver<()>,
    markers: Arc<Mutex<MarkerState>>,
) {
    let timeout = Duration::from_secs_f64(interval);

    // ── System info via sysinfo ──────────────────────────────────────
    let mut sys = System::new_all();
    let sysinfo_pid = Pid::from(pid as usize);
    let total_mem = sys.total_memory() as f64;
    // Prime the CPU usage counter for the whole system
    sys.refresh_all();

    // ── RAPL init ────────────────────────────────────────────────────
    let mut rapl_readers = rapl::scan_rapl_files().unwrap_or_else(|e| {
        eprintln!("[ecocode] RAPL init failed: {}. CPU energy will not be tracked.", e);
        Vec::new()
    });
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
    let mut gpu_energies_prev = devices
        .iter()
        .map(|_| 0.0f64)
        .collect::<Vec<_>>();

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

    // ── Sampling loop ────────────────────────────────────────────────
    loop {
        // Wait for the interval or a stop signal.
        match stop_rx.recv_timeout(timeout) {
            Ok(()) => break,           // stop signal received
            Err(mpsc::RecvTimeoutError::Timeout) => {} // normal tick
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // ── Refresh process metrics for the entire tree ──────────────
        sys.refresh_all();
        let tree_pids = ecocode_core::sensor::get_process_tree_pids(pid);
        
        let mut total_cpu_usage = 0.0f64;
        let mut max_mem_usage = 0.0f64;
        let num_cpus = sys.cpus().len().max(1) as f64;

        for p_id in tree_pids {
            if let Some(proc_) = sys.process(Pid::from(p_id as usize)) {
                // CPU: Sum across cores (already normalized by num_cpus)
                total_cpu_usage += proc_.cpu_usage() as f64 / num_cpus;
                
                // Memory: Use Max RSS to avoid double-counting shared memory
                if total_mem > 0.0 {
                    let mut p_mem = proc_.memory() as f64;
                    
                    // HEURISTIC: If memory looks like it's in KB but we expect Bytes, 
                    // or it's just plain impossible (>100% by a factor of 1000), adjust.
                    if p_mem > total_mem * 10.0 {
                        p_mem /= 1024.0; 
                    }

                    let proc_mem_pct = (p_mem / total_mem) * 100.0;
                    if proc_mem_pct > max_mem_usage {
                        max_mem_usage = proc_mem_pct;
                    }
                }
            }
        }
        
        let cpu_usage = total_cpu_usage.min(100.0);
        let mem_usage = max_mem_usage.min(100.0);

        // Debug every 100 samples to avoid spam
        if run_id % 100 == 0 {
             println!("[ecocode debug] PID: {} | RAM: {} bytes | System: {} bytes | Result: {}%", pid, max_mem_usage * total_mem / 100.0, total_mem, mem_usage);
        }

        // ── CPU / DRAM energy via RAPL ───────────────────────────────
        let mut cpu_energy_j = 0.0f64;
        let mut mem_energy_j: Option<f64> = None;
        let mut igpu_energy_j: Option<f64> = None;
        let mut socket_metrics: Vec<SocketMetric> = Vec::new();

        if rapl_available {
            if let Ok(rapl_curr) = rapl::get_all_energies(&mut rapl_readers) {
                if let Some(ref prev) = rapl_prev {
                    let deltas = rapl::delta_cpu_energy_per_pid_j(
                        prev,
                        &rapl_curr,
                        interval,
                        cpu_usage,
                        mem_usage,
                        cpu_usage, // igpu proxy
                    );
                    for d in &deltas {
                        match d.domain.as_str() {
                            "cpu" => cpu_energy_j += d.delta_energy,
                            "dram" => mem_energy_j = Some(mem_energy_j.unwrap_or(0.0) + d.delta_energy),
                            "igpu" => igpu_energy_j = Some(igpu_energy_j.unwrap_or(0.0) + d.delta_energy),
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
                    pid,
                    &gpu_energies_prev,
                    &gpu_energies_curr,
                    &gpu_timestamps,
                    interval,
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
        let timestamp = chrono::Utc::now().to_rfc3339();

        let record = Record {
            pid,
            timestamp,
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
            tokens_processed: state.tokens_processed,
            training_step: state.training_step,
            loss: state.loss,
            learning_rate: state.learning_rate,
            sequence_length: state.sequence_length,
            accuracy: state.accuracy,
        };

        if let Err(e) = client.add_record(run_id, &record) {
            eprintln!("[ecocode] Failed to send record: {}", e);
        }
    }
}

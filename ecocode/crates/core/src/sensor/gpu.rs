//! GPU energy monitoring module using NVIDIA Management Library (NVML).
//!
//! This module provides functionality to monitor GPU energy consumption and utilization
//! using the NVIDIA Management Library (NVML) wrapper. It can retrieve both system-wide
//! and per-process GPU metrics, including multi-GPU setups.
//!
//! Gated behind the `gpu` feature flag — disabled when building without NVIDIA drivers.

use nvml_wrapper::{Device, Nvml};



/// Per-GPU result from energy-by-PID calculation
pub struct GpuPidResult {
    pub device_index: u32,
    pub energy_j: f64,
    pub util_pct: f64,
    pub next_timestamp: u64,
}






/// Collects Device handles for all GPUs.
pub fn get_all_gpu_devices<'a>(
    nvml: &'a Nvml,
) -> Result<Vec<Device<'a>>, Box<dyn std::error::Error>> {
    let count = nvml.device_count()?;
    let mut devices = Vec::with_capacity(count as usize);
    for i in 0..count {
        devices.push(nvml.device_by_index(i)?);
    }
    Ok(devices)
}

/// Returns the total energy consumption counter for a single GPU device in millijoules (mJ).
/// The counter is monotonically increasing and resets only on driver reload or GPU reset.
pub fn get_gpu_energy(device: &Device) -> Result<f64, Box<dyn std::error::Error>> {
    let energy_milij = device.total_energy_consumption()?; // in milliJ
    Ok(energy_milij as f64)
}



/// Reads energy (mJ) from all GPU devices.
pub fn get_all_gpu_energies(devices: &[Device]) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    devices.iter().map(|d| get_gpu_energy(d)).collect() // in milliJ
}

/// Computes the energy (Joules) and utilization (%) consumed by a specific PID
/// on a single GPU between two energy snapshots.
///
/// Returns `(energy_j, util_pct, next_timestamp)`:
/// - `energy_j`       — process-attributed energy in Joules for this interval
/// - `util_pct`       — average SM utilization (0–100) across all NVML samples for this PID
/// - `next_timestamp` — latest NVML sample timestamp, to be used as the baseline for the next call
///
/// Process utilization is obtained by calling `process_utilization_stats(timestamp)` which
/// returns all samples since the given timestamp. The process's fraction of total GPU energy
/// is derived from its share of SM utilization.
pub fn get_gpu_energy_by_pid(
    device: &Device,
    pids: &[u32], // might be multiple PIDs for the same process e.g. torchrun spawned children
    energy_1: f64,
    energy_2: f64,
    timestamp: u64,
    max_energy_seen: f64,
) -> (f64, f64, u64) {
    let stats = match device.process_utilization_stats(timestamp) {
        Ok(s) => s,
        Err(_) => {
            return (0.0, 0.0, timestamp);
        }
    };

    let next_timestamp = stats.iter().map(|s| s.timestamp).max().unwrap_or(timestamp); //the Cursor

    // GPU-wide utilization SUMS across ALL samples in this interval for each
    // subsystem: compute (SM), memory controller, hardware encoder, hardware decoder.
    // Using sums (not averages) ensures that processes with more samples
    // contribute proportionally, and all shares sum to exactly 1.0.
    let global_sm_sum: f64 = stats.iter().map(|s| s.sm_util as f64).sum();
    let global_mem_sum: f64 = stats.iter().map(|s| s.mem_util as f64).sum();
    let global_enc_sum: f64 = stats.iter().map(|s| s.enc_util as f64).sum();
    let global_dec_sum: f64 = stats.iter().map(|s| s.dec_util as f64).sum();
    let total_activity = global_sm_sum + global_mem_sum + global_enc_sum + global_dec_sum;

    // Track the entire process tree (already provided in pids)
    let pid_samples: Vec<_> = stats
        .iter()
        .filter(|s| pids.contains(&(s.pid as u32)))
        .collect();

    // Compute process's fractional share using GPU-wide dynamic weights.
    // Weights are derived from overall GPU behavior (which subsystem is busier)
    // and applied uniformly. Using sums guarantees all shares sum to 1.0.
    let process_share = if total_activity == 0.0 || pid_samples.is_empty() {
        0.0
    } else {
        // Weights: whichever subsystem the GPU uses more gets more blame
        let sm_w = global_sm_sum / total_activity;
        let mem_w = global_mem_sum / total_activity;
        let enc_w = global_enc_sum / total_activity;
        let dec_w = global_dec_sum / total_activity;

        // Sum (not average) for the target process
        let process_sm_sum: f64 = pid_samples.iter().map(|s| s.sm_util as f64).sum();
        let process_mem_sum: f64 = pid_samples.iter().map(|s| s.mem_util as f64).sum();
        let process_enc_sum: f64 = pid_samples.iter().map(|s| s.enc_util as f64).sum();
        let process_dec_sum: f64 = pid_samples.iter().map(|s| s.dec_util as f64).sum();

        let process_score = sm_w * process_sm_sum + mem_w * process_mem_sum
                          + enc_w * process_enc_sum + dec_w * process_dec_sum;
        let total_score = sm_w * global_sm_sum + mem_w * global_mem_sum
                        + enc_w * global_enc_sum + dec_w * global_dec_sum;

        if total_score > 0.0 {
            process_score / total_score
        } else {
            0.0
        }
    };

    // NVML's total_energy_consumption is a monotonically-increasing u64 mJ counter
    // that resets to 0 on driver reload or GPU reset (mathematical u64 overflow
    // would take ~1.5 million years at full load and never happens in practice).
    //
    // When the counter goes backwards we reconstruct the true delta using the
    // highest value observed so far as the rollover point:
    //   delta = energy_2 + (max_energy_seen - energy_1)
    //
    // This recovers any energy accumulated between energy_1 and the reset moment.
    // When max_energy_seen == energy_1 (the common case) it reduces to energy_2.
    let delta_energy_mj = if energy_2 >= energy_1 {
        energy_2 - energy_1
    } else {
        eprintln!(
            "Warning: GPU energy counter decreased — driver reload or GPU reset detected. \
             Applying rollover correction."
        );
        (energy_2 + (max_energy_seen - energy_1)).max(0.0)
    };

    let process_gpu_energy_j = (delta_energy_mj / 1000.0) * process_share;

    // Per-process SM utilization (0-100), averaged across samples.
    // This reflects actual GPU hardware busyness, unlike process_share
    // which is always ~100% when only one process uses the GPU.
    let process_sm_util = if pid_samples.is_empty() {
        0.0
    } else {
        let sm_sum: f64 = pid_samples.iter().map(|s| s.sm_util as f64).sum();
        sm_sum / pid_samples.len() as f64
    };

    (
        process_gpu_energy_j,
        process_sm_util.round_ties_even(),
        next_timestamp,
    )
}



/// Computes per-PID GPU energy/utilization for ALL GPUs in one call.
pub fn get_all_gpu_energies_by_pid(
    devices: &[Device],
    pids: &[u32],
    energies_1: &[f64],
    energies_2: &[f64],
    timestamps: &[u64],
    max_energies: &[f64],
) -> Vec<GpuPidResult> {
    devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let e1 = energies_1.get(i).copied().unwrap_or(0.0);
            let e2 = energies_2.get(i).copied().unwrap_or(0.0);
            let ts = timestamps.get(i).copied().unwrap_or(0);
            let max_e = max_energies.get(i).copied().unwrap_or(0.0);

            let (energy_j, util_pct, next_ts) = get_gpu_energy_by_pid(
                device,
                pids,
                e1,
                e2,
                ts,
                max_e,
            );
            GpuPidResult {
                device_index: i as u32,
                energy_j,
                util_pct,
                next_timestamp: next_ts,
            }
        })
        .collect()
}

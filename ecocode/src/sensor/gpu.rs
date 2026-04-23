//! GPU energy monitoring module using NVIDIA Management Library (NVML).
//!
//! This module provides functionality to monitor GPU energy consumption and utilization
//! using the NVIDIA Management Library (NVML) wrapper. It can retrieve both system-wide
//! and per-process GPU metrics, including multi-GPU setups.

use nvml_wrapper::{Device, Nvml};

#[allow(dead_code)]
pub const DEFAULT_GPU_DEVICE_INDEX: u32 = 0;

/// Per-GPU result from energy-by-PID calculation
pub struct GpuPidResult {
    pub device_index: u32,
    pub energy_j: f64,
    pub util_pct: f64,
    pub next_timestamp: u64,
}

pub fn _get_gpu_info() -> Result<(), Box<dyn std::error::Error>> {
    let nvml = Nvml::init()?;
    let device = nvml.device_by_index(DEFAULT_GPU_DEVICE_INDEX)?;
    let name = device.name()?;
    println!("\n[GPU INFO]");
    println!("  GPU Name: {}", name);
    Ok(())
}

pub fn _get_gpu_power() -> Result<u32, Box<dyn std::error::Error>> {
    let nvml = Nvml::init()?;
    let device = nvml.device_by_index(DEFAULT_GPU_DEVICE_INDEX)?;
    let power = device.power_usage()?; // in mW

    Ok(power)
}

#[allow(dead_code)]
pub fn get_gpu_count(nvml: &Nvml) -> Result<u32, Box<dyn std::error::Error>> {
    Ok(nvml.device_count()?)
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

pub fn get_gpu_energy(device: &Device) -> Result<f64, Box<dyn std::error::Error>> {
    let energy_milij = device.total_energy_consumption()?; // in milliJ
    Ok(energy_milij as f64)
}

/// Reads energy (mJ) from all GPU devices.
pub fn get_all_gpu_energies(devices: &[Device]) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    devices.iter().map(|d| get_gpu_energy(d)).collect() // in milliJ
}

pub fn get_gpu_energy_by_pid(
    device: &Device,
    pid: u32,
    energy_1: f64,
    energy_2: f64,
    timestamp: u64,
    _interval_secs: f64,
    max_energy_seen: f64,
) -> (f64, f64, u64) {
    let stats = match device.process_utilization_stats(timestamp) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Error fetching process utilization for PID {}: {}, maybe it stopped running on the GPU.",
                pid, e
            );
            return (0.0, 0.0, timestamp);
        }
    };

    let next_timestamp = stats.iter().map(|s| s.timestamp).max().unwrap_or(timestamp);

    let pid_samples: Vec<_> = stats.iter().filter(|s| s.pid == pid).collect();

    let process_util = if pid_samples.is_empty() {
        0.0
    } else {
        let sum: u64 = pid_samples.iter().map(|s| s.sm_util as u64).sum();
        (sum as f64 / pid_samples.len() as f64) / 100.0
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

    let total_gpu_energy_mj = delta_energy_mj;
    let process_gpu_energy_j = (total_gpu_energy_mj / 1000.0) * process_util;

    (
        process_gpu_energy_j,
        (process_util * 100.0).round_ties_even(),
        next_timestamp,
    )
}

/// Computes per-PID GPU energy/utilization for ALL GPUs in one call.
pub fn get_all_gpu_energies_by_pid(
    devices: &[Device],
    pid: u32,
    energies_1: &[f64],
    energies_2: &[f64],
    timestamps: &[u64],
    _interval_secs: f64,
    max_energies: &[f64],
) -> Vec<GpuPidResult> {
    devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let (energy_j, util_pct, next_ts) = get_gpu_energy_by_pid(
                device,
                pid,
                energies_1[i],
                energies_2[i],
                timestamps[i],
                _interval_secs,
                max_energies[i],
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

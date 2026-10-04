//! Per-PID GPU utilisation samples.

use nvml_wrapper::Device;
use nvml_wrapper::error::NvmlError;

use crate::domain::GpuUtilSample;

/// Returns all per-process utilisation samples NVML recorded since `timestamp`
/// (µs, as returned in a previous sample). Passing 0 returns the whole history
/// NVML still holds.
pub fn utilization_samples(device: &Device, timestamp: u64) -> Result<Vec<GpuUtilSample>, NvmlError> {
    Ok(device
        .process_utilization_stats(timestamp)?
        .into_iter()
        .map(|s| GpuUtilSample {
            pid: s.pid,
            timestamp: s.timestamp,
            sm_util: s.sm_util,
            mem_util: s.mem_util,
            enc_util: s.enc_util,
            dec_util: s.dec_util,
        })
        .collect())
}

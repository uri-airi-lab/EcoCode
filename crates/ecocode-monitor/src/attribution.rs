//! Attribution of measured energy to the monitored process tree.
//!
//! Sensors measure energy for a whole package, DRAM channel or GPU. Each
//! interval's delta is split by the process tree's share of the matching
//! resource.

use ecocode_sensors::{Domain, GpuUtilSample};

/// The process tree's share of machine resources over one interval (0–100).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Usage {
    /// Share of total CPU capacity (all cores).
    pub cpu_pct: f64,
    /// Share of total physical memory.
    pub mem_pct: f64,
    /// Share of the integrated GPU. There is no per-process iGPU sensor, so
    /// this is proxied from CPU usage.
    pub igpu_pct: f64,
}

/// Energy (J) attributed to the process tree for one non-GPU channel.
///
/// Attribution is proportional: the channel's energy is multiplied by the
/// process tree's fractional share of the relevant resource:
/// - `cpu` and unknown domains → `cpu_pct / 100`
/// - `dram` domain → `mem_pct / 100`
/// - `igpu` domain → `igpu_pct / 100`
///
/// DRAM uses memory size (PSS on Linux) as a proxy, which measures memory
/// *capacity* rather than *bandwidth*. DRAM power is mostly driven by
/// read/write activity, so this can over-attribute to a large-but-idle process
/// and under-attribute to a small-but-active one. A blend of static
/// (size-weighted, for refresh power) and dynamic (CPU-weighted, for access
/// power) would be more accurate, but isn't implemented yet.
pub fn share_energy(domain: &Domain, joules: f64, usage: &Usage) -> f64 {
    let usage_fraction = match domain {
        Domain::Dram => usage.mem_pct,
        Domain::Igpu => usage.igpu_pct,
        _ => usage.cpu_pct,
    } / 100.0;
    (joules * usage_fraction).max(0.0)
}

/// The process tree's fractional share (0–1) of one GPU's energy, and its
/// average SM utilisation (0–100, rounded) over the interval.
///
/// `samples` are all per-process utilisation samples for the interval. The
/// share is the tree's weighted activity over the GPU's total activity, where
/// each subsystem (compute, memory controller, encoder, decoder) is weighted by
/// how busy it was GPU-wide.
pub fn gpu_process_share(samples: &[GpuUtilSample], pids: &[u32]) -> (f64, f64) {
    // GPU-wide utilization SUMS across ALL samples in this interval for each
    // subsystem: compute (SM), memory controller, hardware encoder, hardware decoder.
    // Using sums (not averages) ensures that processes with more samples
    // contribute proportionally, and all shares sum to exactly 1.0.
    let global_sm_sum: f64 = samples.iter().map(|s| s.sm_util as f64).sum();
    let global_mem_sum: f64 = samples.iter().map(|s| s.mem_util as f64).sum();
    let global_enc_sum: f64 = samples.iter().map(|s| s.enc_util as f64).sum();
    let global_dec_sum: f64 = samples.iter().map(|s| s.dec_util as f64).sum();
    let total_activity = global_sm_sum + global_mem_sum + global_enc_sum + global_dec_sum;

    // Keep only the samples belonging to a PID in our process tree
    let pid_samples: Vec<_> = samples.iter().filter(|s| pids.contains(&s.pid)).collect();

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

        let process_score = sm_w * process_sm_sum
            + mem_w * process_mem_sum
            + enc_w * process_enc_sum
            + dec_w * process_dec_sum;
        let total_score = sm_w * global_sm_sum
            + mem_w * global_mem_sum
            + enc_w * global_enc_sum
            + dec_w * global_dec_sum;

        if total_score > 0.0 {
            process_score / total_score
        } else {
            0.0
        }
    };

    // Per-process SM utilization (0-100), averaged across samples.
    // This reflects actual GPU hardware busyness, unlike process_share
    // which is always ~100% when only one process uses the GPU.
    let process_sm_util = if pid_samples.is_empty() {
        0.0
    } else {
        let sm_sum: f64 = pid_samples.iter().map(|s| s.sm_util as f64).sum();
        sm_sum / pid_samples.len() as f64
    };

    (process_share, process_sm_util.round_ties_even())
}

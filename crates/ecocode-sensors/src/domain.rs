//! What a sensor measures: energy domains, channels and readings.

use std::fmt;

/// The part of the machine an energy channel measures.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    /// CPU cores (RAPL core / PP0, Apple CPU clusters).
    Cpu,
    /// Memory (RAPL DRAM, Apple DRAM).
    Dram,
    /// Integrated graphics / uncore (RAPL uncore / PP1).
    Igpu,
    /// A GPU, by device index (NVIDIA via NVML, Apple GPU).
    Gpu(u32),
    /// Apple Neural Engine.
    NeuralEngine,
    /// Any other domain, by the sensor's own name (e.g. RAPL `psys`).
    Other(String),
}

impl Domain {
    /// Label used in records (`socket_metrics[].domain`) and exporters.
    pub fn label(&self) -> String {
        match self {
            Domain::Cpu => "cpu".into(),
            Domain::Dram => "dram".into(),
            Domain::Igpu => "igpu".into(),
            Domain::Gpu(_) => "gpu".into(),
            Domain::NeuralEngine => "ane".into(),
            Domain::Other(name) => name.clone(),
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Gpu(i) => write!(f, "gpu{i}"),
            other => f.write_str(&other.label()),
        }
    }
}

/// How trustworthy a channel's numbers are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    /// Read from a hardware energy counter.
    Measured,
    /// Derived from a model (e.g. power × time), not a counter.
    Estimated,
}

/// One energy counter exposed by a sensor.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub domain: Domain,
    /// CPU socket / package index; 0 for devices that are not per-socket.
    pub socket: i16,
    /// Human-readable name, e.g. `"package-0 core"` or `"NVIDIA A100"`.
    pub name: String,
    pub quality: Quality,
}

/// One NVML-style per-process utilisation sample (percentages, 0–100).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuUtilSample {
    pub pid: u32,
    /// Sample timestamp in µs.
    pub timestamp: u64,
    /// SM (3D / compute) utilisation.
    pub sm_util: u32,
    /// Frame buffer memory utilisation.
    pub mem_util: u32,
    /// Encoder utilisation.
    pub enc_util: u32,
    /// Decoder utilisation.
    pub dec_util: u32,
}

/// Latest sample timestamp, or `prev` when there are no samples. Used as the
/// baseline for the next utilisation query.
pub fn latest_timestamp(samples: &[GpuUtilSample], prev: u64) -> u64 {
    samples.iter().map(|s| s.timestamp).max().unwrap_or(prev)
}

/// The monitored process tree's share of one channel over an interval, for
/// sensors that measure it directly (Apple GPU time per process).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeShare {
    /// Fraction (0–1) of the channel's energy attributed to the tree.
    pub share: f64,
    /// Utilisation of the device by the tree (0–100).
    pub util_pct: f64,
}

/// Energy one channel consumed since the previous read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Whole-domain energy in joules, already corrected for counter wrap/reset.
    pub joules: f64,
    /// Per-process utilisation samples covering the same interval, for devices
    /// that report them (NVML). `None` when unavailable.
    pub util_samples: Option<Vec<GpuUtilSample>>,
    /// The tree's share, when the sensor measures it itself. Takes precedence
    /// over `util_samples` and over the CPU/memory share.
    pub tree_share: Option<TreeShare>,
}

impl Reading {
    pub fn energy(joules: f64) -> Self {
        Reading { joules, ..Default::default() }
    }
}

//! Hardware energy sensors and process inspection for EcoCode.
//!
//! This is the only crate with OS-specific sensor code. Everything else talks
//! to hardware through two traits:
//! - [`EnergySensor`]: whole-domain energy counters (RAPL, NVML, …)
//! - [`ProcessInspector`]: per-process CPU time and memory, used to attribute
//!   a share of that energy to the monitored process tree
//!
//! [`detect`] picks the implementations for the current OS and never fails:
//! a missing or unreadable sensor becomes a [`Diagnostic`] instead of an error.

use std::collections::HashMap;
use std::str::FromStr;

use sysinfo::System;

pub mod common;
pub mod domain;
pub mod error;
pub mod fake;

#[cfg(any(target_os = "linux", test))]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(any(windows, test))]
pub mod windows;

pub use domain::{Channel, Domain, GpuUtilSample, Quality, Reading, TreeShare};
pub use error::SensorError;

/// A source of whole-domain energy readings.
pub trait EnergySensor: Send {
    /// Short identifier, e.g. `"powercap"`, `"nvml"`.
    fn name(&self) -> &'static str;

    /// The channels this sensor reports, in the order [`read`](Self::read) returns them.
    fn channels(&self) -> &[Channel];

    /// Energy consumed by every channel since the previous call (or since the
    /// sensor was created, for the first call). `tree` is the monitored process
    /// tree, for sensors that report per-process data.
    fn read(&mut self, tree: &[u32]) -> Result<Vec<Reading>, SensorError>;
}

/// Per-process resource usage, used to split whole-domain energy.
pub trait ProcessInspector: Send {
    /// Short identifier, e.g. `"procfs"`.
    fn name(&self) -> &'static str;

    /// How many [`cpu_times`](Self::cpu_times) units make one CPU-second.
    fn cpu_time_units_per_sec(&self) -> f64;

    /// Cumulative CPU time per process in `tree`. Threads must be excluded so
    /// the same time is not counted twice.
    fn cpu_times(&mut self, sys: &System, tree: &[u32]) -> HashMap<u32, u64>;

    /// Memory used by the processes in `tree`, in bytes.
    fn memory_bytes(&mut self, sys: &System, tree: &[u32]) -> u64;

    /// What [`memory_bytes`](Self::memory_bytes) measures, e.g. `"PSS"`.
    fn memory_kind(&self) -> &'static str;

    /// Cumulative CPU energy (nanojoules) per process in `tree`, on systems
    /// where the OS measures it per process (Apple Silicon). When available,
    /// the sampler attributes CPU energy from this instead of the CPU-time share.
    fn cpu_energy_nj(&mut self, _tree: &[u32]) -> Option<HashMap<u32, u64>> {
        None
    }

    /// Whether [`cpu_energy_nj`](Self::cpu_energy_nj) is supported here.
    fn has_cpu_energy(&self) -> bool {
        false
    }

    /// Whether this inspector reads CPU time and memory out of the `sysinfo`
    /// process table (so the sampler must refresh those fields every tick).
    /// Inspectors that query the OS directly leave this false, which keeps the
    /// per-tick refresh down to the process list and parent PIDs.
    fn needs_process_stats(&self) -> bool {
        false
    }
}

/// Which sensors [`detect`] should use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorMode {
    /// Real sensors for this OS.
    Auto,
    /// Deterministic fake sensors, for tests on any machine.
    Fake,
    /// No energy sensors (process statistics only).
    None,
}

impl FromStr for SensorMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(SensorMode::Auto),
            "fake" => Ok(SensorMode::Fake),
            "none" => Ok(SensorMode::None),
            other => Err(format!("unknown sensor mode '{other}' (expected auto, fake or none)")),
        }
    }
}

/// Outcome of probing one sensor source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Available,
    /// Not present on this machine.
    Unavailable,
    /// Present but not readable by this user.
    PermissionDenied,
    /// Present but failed to initialise.
    Error,
}

/// What [`detect`] found for one sensor source, for `ecocode doctor` and warnings.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub source: &'static str,
    pub status: Status,
    pub detail: String,
    /// How to fix it, when there is something the user can do.
    pub hint: Option<String>,
}

/// The sensors chosen for this machine.
pub struct Detection {
    pub sensors: Vec<Box<dyn EnergySensor>>,
    pub inspector: Box<dyn ProcessInspector>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Detection {
    /// All channels of all sensors, with the owning sensor's name.
    pub fn channels(&self) -> impl Iterator<Item = (&'static str, &Channel)> {
        self.sensors
            .iter()
            .flat_map(|s| s.channels().iter().map(move |c| (s.name(), c)))
    }

    pub fn has_energy(&self) -> bool {
        self.sensors.iter().any(|s| !s.channels().is_empty())
    }
}

/// Picks the sensors for this machine. Never fails.
pub fn detect(mode: SensorMode) -> Detection {
    let mut detection = Detection {
        sensors: Vec::new(),
        inspector: default_inspector(mode),
        diagnostics: Vec::new(),
    };
    match mode {
        SensorMode::Fake => {
            detection.sensors.push(Box::new(fake::FakeSensor::new()));
            detection.diagnostics.push(Diagnostic {
                source: "fake",
                status: Status::Available,
                detail: "fake sensors (for testing): constant power on cpu, dram and one GPU".into(),
                hint: None,
            });
        }
        SensorMode::None => {}
        SensorMode::Auto => detect_platform(&mut detection),
    }
    detection
}

#[cfg(target_os = "linux")]
fn detect_platform(detection: &mut Detection) {
    linux::detect_into(detection);
    #[cfg(feature = "nvml")]
    common::nvml::detect_into(detection);
}

#[cfg(target_os = "macos")]
fn detect_platform(detection: &mut Detection) {
    macos::detect_into(detection);
}

#[cfg(windows)]
fn detect_platform(detection: &mut Detection) {
    windows::detect_into(detection);
    #[cfg(feature = "nvml")]
    common::nvml::detect_into(detection);
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn detect_platform(detection: &mut Detection) {
    detection.diagnostics.push(Diagnostic {
        source: std::env::consts::OS,
        status: Status::Unavailable,
        detail: "no energy sensors are supported on this OS".into(),
        hint: None,
    });
}

#[cfg(target_os = "linux")]
fn default_inspector(_mode: SensorMode) -> Box<dyn ProcessInspector> {
    Box::new(linux::process::ProcFsInspector::new())
}

#[cfg(target_os = "macos")]
fn default_inspector(mode: SensorMode) -> Box<dyn ProcessInspector> {
    Box::new(macos::process::MacInspector::new(mode == SensorMode::Auto))
}

#[cfg(windows)]
fn default_inspector(_mode: SensorMode) -> Box<dyn ProcessInspector> {
    Box::new(windows::process::WindowsInspector)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn default_inspector(_mode: SensorMode) -> Box<dyn ProcessInspector> {
    Box::new(common::sysinfo_process::SysinfoInspector)
}

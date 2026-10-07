//! NVIDIA GPU energy and per-process utilisation via NVML (Linux and Windows).
//!
//! NVML is loaded at runtime, so building with the `nvml` feature works on
//! machines without NVIDIA drivers; `Nvml::init()` simply fails there.

pub mod device;
pub mod process;

pub use nvml_wrapper::{Device, Nvml};

use nvml_wrapper::error::NvmlError;

use crate::common::counter::millijoules_reset_delta_j;
use crate::domain::{Channel, Domain, Quality, Reading, latest_timestamp};
use crate::{Detection, Diagnostic, EnergySensor, SensorError, Status};

/// Energy of every NVIDIA GPU, with per-process utilisation samples so the
/// monitored process tree's share can be attributed.
pub struct NvmlSensor {
    nvml: Nvml,
    channels: Vec<Channel>,
    /// Previous total-energy counter per device (mJ).
    prev_mj: Vec<f64>,
    /// Highest counter value seen per device, the rollover point after a reset.
    max_seen_mj: Vec<f64>,
    /// Utilisation sample baseline per device (µs).
    timestamps: Vec<u64>,
}

impl NvmlSensor {
    pub fn new(nvml: Nvml) -> Result<Self, NvmlError> {
        let count = nvml.device_count()?;
        let mut channels = Vec::with_capacity(count as usize);
        let mut prev_mj = Vec::with_capacity(count as usize);
        for i in 0..count {
            let device = nvml.device_by_index(i)?;
            channels.push(Channel {
                domain: Domain::Gpu(i),
                socket: 0,
                name: device.name().unwrap_or_else(|_| format!("NVIDIA GPU {i}")),
                quality: Quality::Measured,
            });
            prev_mj.push(device::get_gpu_energy(&device).map_err(|_| NvmlError::NotSupported)?);
        }
        Ok(NvmlSensor {
            nvml,
            channels,
            max_seen_mj: prev_mj.clone(),
            prev_mj,
            timestamps: vec![0; count as usize],
        })
    }
}

impl EnergySensor for NvmlSensor {
    fn name(&self) -> &'static str {
        "nvml"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, _tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let mut readings = Vec::with_capacity(self.channels.len());
        for i in 0..self.channels.len() {
            let device = self
                .nvml
                .device_by_index(i as u32)
                .map_err(|e| SensorError::new("nvml", format!("GPU {i}: {e}")))?;
            let curr = device::get_gpu_energy(&device)
                .map_err(|e| SensorError::new("nvml", format!("GPU {i} energy: {e}")))?;

            // NVML's total_energy_consumption is a monotonically-increasing u64 mJ
            // counter that resets to 0 on driver reload or GPU reset.
            if curr < self.prev_mj[i] {
                eprintln!(
                    "Warning: GPU energy counter decreased, driver reload or GPU reset detected. \
                     Applying rollover correction."
                );
            }
            let joules = millijoules_reset_delta_j(self.prev_mj[i], curr, self.max_seen_mj[i]);
            // Only a value above the previous peak moves the rollover point, so a
            // reset keeps the pre-reset peak for the correction above.
            self.max_seen_mj[i] = self.max_seen_mj[i].max(curr);
            self.prev_mj[i] = curr;

            // Samples since the previous read. A failed query attributes nothing
            // to the process tree for this interval.
            let util_samples = match process::utilization_samples(&device, self.timestamps[i]) {
                Ok(samples) => {
                    self.timestamps[i] = latest_timestamp(&samples, self.timestamps[i]);
                    Some(samples)
                }
                Err(_) => None,
            };
            readings.push(Reading { joules, util_samples, tree_share: None });
        }
        Ok(readings)
    }
}

/// Adds an [`NvmlSensor`] when NVIDIA GPUs are present.
pub fn detect_into(detection: &mut Detection) {
    let diagnostic = |status, detail: String, hint: Option<&str>| Diagnostic {
        source: "nvml",
        status,
        detail,
        hint: hint.map(str::to_string),
    };
    let nvml = match Nvml::init() {
        Ok(n) => n,
        Err(e) => {
            let status = match e {
                NvmlError::LibloadingError(_)
                | NvmlError::LibraryNotFound
                | NvmlError::DriverNotLoaded => Status::Unavailable,
                NvmlError::NoPermission => Status::PermissionDenied,
                _ => Status::Error,
            };
            detection.diagnostics.push(diagnostic(
                status,
                format!("NVIDIA NVML not available ({e})"),
                (status != Status::Unavailable).then_some("check the NVIDIA driver with `nvidia-smi`"),
            ));
            return;
        }
    };
    match NvmlSensor::new(nvml) {
        Ok(sensor) if sensor.channels.is_empty() => {
            detection
                .diagnostics
                .push(diagnostic(Status::Unavailable, "no NVIDIA GPUs found".into(), None));
        }
        Ok(sensor) => {
            let names: Vec<&str> = sensor.channels.iter().map(|c| c.name.as_str()).collect();
            detection.diagnostics.push(diagnostic(
                Status::Available,
                format!("{} NVIDIA GPU(s): {}", names.len(), names.join(", ")),
                None,
            ));
            detection.sensors.push(Box::new(sensor));
        }
        Err(e) => detection.diagnostics.push(diagnostic(
            Status::Error,
            format!("NVML initialised but GPUs could not be read ({e})"),
            Some("check the NVIDIA driver with `nvidia-smi`"),
        )),
    }
}

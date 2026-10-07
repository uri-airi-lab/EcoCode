//! Windows sensors: the Energy Meter Interface ([`emi`]) for CPU, iGPU and
//! DRAM energy, and per-process CPU time and memory ([`process`]). NVIDIA GPUs
//! are measured through NVML like on Linux.
//!
//! The format parsing in these modules has no Windows dependency and is
//! tested on every OS; the Win32 calls are compiled on Windows only.

pub mod emi;
pub mod process;

#[cfg(windows)]
use crate::{Detection, Diagnostic, Status};

/// Adds an EMI sensor when the machine has an energy meter device.
#[cfg(windows)]
pub fn detect_into(detection: &mut Detection) {
    let diagnostic = |status, detail: String, hint: Option<&str>| Diagnostic {
        source: "emi",
        status,
        detail,
        hint: hint.map(str::to_string),
    };
    match emi::EmiSensor::open() {
        Ok(sensor) => {
            use crate::EnergySensor;
            let names: Vec<String> =
                sensor.channels().iter().map(|c| format!("{} → {}", c.name, c.domain)).collect();
            detection
                .diagnostics
                .push(diagnostic(Status::Available, format!("Energy Meter Interface: {}", names.join(", ")), None));
            detection.sensors.push(Box::new(sensor));
        }
        Err(emi::OpenError::NoDevices) => detection.diagnostics.push(diagnostic(
            Status::Unavailable,
            "no Energy Meter Interface device (common on desktops and AMD systems); CPU energy cannot be measured"
                .into(),
            None,
        )),
        Err(emi::OpenError::AccessDenied) => detection.diagnostics.push(diagnostic(
            Status::PermissionDenied,
            "Energy Meter Interface devices exist but could not be opened".into(),
            Some("run ecocode from an Administrator terminal"),
        )),
        Err(emi::OpenError::NoUsableChannels(names)) => detection.diagnostics.push(diagnostic(
            Status::Unavailable,
            format!("Energy Meter Interface has no CPU, GPU or DRAM channels ({})", names.join(", ")),
            None,
        )),
    }
}

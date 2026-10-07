//! macOS sensors.
//!
//! - Apple Silicon: [`ioreport`] for CPU, GPU, Neural Engine and DRAM energy;
//!   per-process GPU time from [`gpu_usage`]; per-process CPU energy from
//!   [`rusage`]
//! - Intel: [`smc`] CPU package power
//! - Both: [`process`] CPU time and memory via `proc_pid_rusage`

pub mod cf;
pub mod gpu_usage;
pub mod iokit;
pub mod ioreport;
pub mod process;
pub mod rusage;
pub mod smc;

use std::ffi::{CStr, c_void};

use crate::{Detection, Diagnostic, Status};

/// True on Apple Silicon, including x86_64 binaries running under Rosetta.
pub fn is_apple_silicon() -> bool {
    sysctl_u32(c"hw.optional.arm64") == Some(1)
}

/// True for an x86_64 binary running under Rosetta on Apple Silicon.
pub fn is_translated() -> bool {
    sysctl_u32(c"sysctl.proc_translated") == Some(1)
}

/// The chip name, e.g. "Apple M1 Pro".
pub fn chip_name() -> String {
    sysctl_string(c"machdep.cpu.brand_string").unwrap_or_else(|| "Apple".to_string())
}

fn sysctl_u32(name: &CStr) -> Option<u32> {
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>();
    let rc = unsafe {
        libc::sysctlbyname(name.as_ptr(), &mut value as *mut u32 as *mut c_void, &mut size, std::ptr::null_mut(), 0)
    };
    (rc == 0).then_some(value)
}

fn sysctl_string(name: &CStr) -> Option<String> {
    let mut buf = [0u8; 256];
    let mut size = buf.len();
    let rc = unsafe {
        libc::sysctlbyname(name.as_ptr(), buf.as_mut_ptr() as *mut c_void, &mut size, std::ptr::null_mut(), 0)
    };
    if rc != 0 {
        return None;
    }
    let s = CStr::from_bytes_until_nul(&buf).ok()?.to_string_lossy().trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Adds IOReport (Apple Silicon) or SMC (Intel) energy sensors.
pub fn detect_into(detection: &mut Detection) {
    let chip = chip_name();
    if let Some(sensor) = ioreport::IoReportSensor::open(&chip) {
        let domains: Vec<String> = sensor.channels_ref().iter().map(|c| c.domain.to_string()).collect();
        detection.diagnostics.push(Diagnostic {
            source: "ioreport",
            status: Status::Available,
            detail: format!("{chip} Energy Model: {}", domains.join(", ")),
            hint: None,
        });
        detection.sensors.push(Box::new(sensor));
        return;
    }
    detection.diagnostics.push(Diagnostic {
        source: "ioreport",
        status: Status::Unavailable,
        detail: "no IOReport Energy Model (Intel Mac or virtual machine)".into(),
        hint: None,
    });

    match smc::SmcSensor::open() {
        Some(sensor) => {
            detection.diagnostics.push(Diagnostic {
                source: "smc",
                status: Status::Available,
                detail: format!("CPU package power from SMC key {} (sampled, estimated energy)", sensor.key()),
                hint: None,
            });
            detection.sensors.push(Box::new(sensor));
        }
        None => detection.diagnostics.push(Diagnostic {
            source: "smc",
            status: Status::Unavailable,
            detail: format!("no SMC CPU power key ({})", smc::CPU_POWER_KEYS.join(", ")),
            hint: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_name_is_known() {
        assert!(!chip_name().is_empty());
    }
}

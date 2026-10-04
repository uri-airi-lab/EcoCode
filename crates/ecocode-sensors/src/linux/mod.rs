//! Linux sensors: RAPL energy via powercap sysfs, per-process stats via `/proc`.

pub mod hwmon;
pub mod powercap;
pub mod process;

use std::path::Path;

use crate::{Detection, Diagnostic, EnergySensor, Status};

/// Adds a [`powercap::RaplSensor`] when RAPL counters are present and
/// readable, otherwise a [`hwmon::HwmonSensor`] when the board exposes CPU or
/// memory energy/power through hwmon (ARM servers). Never both, so nothing is
/// counted twice.
pub fn detect_into(detection: &mut Detection) {
    if detect_powercap(detection, Path::new(powercap::RAPL_PATH)) {
        return;
    }
    detect_hwmon(detection, Path::new(hwmon::HWMON_PATH));
}

fn detect_powercap(detection: &mut Detection, root: &Path) -> bool {
    let diagnostic = |status, detail: String, hint: Option<String>| Diagnostic {
        source: "powercap",
        status,
        detail,
        hint,
    };
    if !root.exists() {
        detection.diagnostics.push(diagnostic(
            Status::Unavailable,
            format!(
                "no RAPL powercap interface at {} (virtual machine, ARM CPU, or intel_rapl module not loaded)",
                root.display()
            ),
            None,
        ));
        return false;
    }
    match powercap::RaplSensor::open(root) {
        Ok(sensor) => {
            let domains: Vec<String> =
                sensor.channels_ref().iter().map(|c| format!("socket {} {}", c.socket, c.domain)).collect();
            detection.diagnostics.push(diagnostic(
                Status::Available,
                format!("RAPL: {}", domains.join(", ")),
                None,
            ));
            detection.sensors.push(Box::new(sensor));
            return true;
        }
        Err(powercap::OpenError::PermissionDenied) => detection.diagnostics.push(diagnostic(
            Status::PermissionDenied,
            "RAPL energy counters exist but are readable by root only".into(),
            Some(format!(
                "permanent: `sudo packaging/linux/install-permissions.sh` (see docs/platforms/linux.md); \
                 until reboot: `sudo chmod -R a+r {}`",
                root.display()
            )),
        )),
        Err(powercap::OpenError::NoDomains) => detection.diagnostics.push(diagnostic(
            Status::Unavailable,
            "RAPL interface has no energy domains".into(),
            None,
        )),
        Err(powercap::OpenError::Io(e)) => detection.diagnostics.push(diagnostic(
            Status::Error,
            format!("could not read RAPL counters ({e})"),
            None,
        )),
    }
    false
}

fn detect_hwmon(detection: &mut Detection, root: &Path) {
    match hwmon::HwmonSensor::open(root) {
        Some(sensor) => {
            let names: Vec<String> =
                sensor.channels().iter().map(|c| format!("{} ({:?})", c.name, c.quality)).collect();
            detection.diagnostics.push(Diagnostic {
                source: "hwmon",
                status: Status::Available,
                detail: names.join(", "),
                hint: None,
            });
            detection.sensors.push(Box::new(sensor));
        }
        None => detection.diagnostics.push(Diagnostic {
            source: "hwmon",
            status: Status::Unavailable,
            detail: "no hwmon CPU or memory energy/power sensors".into(),
            hint: None,
        }),
    }
}

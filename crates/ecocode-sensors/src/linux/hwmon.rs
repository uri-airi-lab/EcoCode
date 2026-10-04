//! CPU and memory energy from hwmon (`/sys/class/hwmon`), for Linux machines
//! without RAPL: ARM servers (NVIDIA Grace, Ampere), and other boards whose
//! drivers expose power telemetry.
//!
//! Each `hwmonN` directory may provide, per channel `N`:
//! - `energyN_input` — cumulative energy in µJ → used as measured energy
//! - `powerN_input` / `powerN_average` — power in µW → integrated between
//!   reads (trapezoid rule), reported as estimated
//! - `energyN_label` / `powerN_label` — what the channel measures
//!
//! Channels are classified by label: `cpu`, `package` or `socket` → CPU;
//! `dram`, `ddr` or `memory` → DRAM. Unlabelled and other channels (GPU, I/O,
//! whole-module rails) are skipped, so nothing is counted twice. A power
//! channel with the same label as an energy channel is skipped too, and when a
//! chip labels some channels explicitly `cpu`, broader CPU channels on it
//! (e.g. Grace's "Grace Power", which includes memory) are dropped.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::common::counter::reset_delta;
use crate::domain::{Channel, Domain, Quality, Reading};
use crate::{EnergySensor, SensorError};

/// Root of the hwmon class.
pub const HWMON_PATH: &str = "/sys/class/hwmon";

/// Which domain a channel label belongs to, if any.
pub fn classify(label: &str) -> Option<Domain> {
    let l = label.to_ascii_lowercase();
    if l.contains("gpu") || l.contains("module") || l.contains("sysio") || l.contains(" io ") || l.starts_with("io ") {
        return None;
    }
    if l.contains("dram") || l.contains("ddr") || l.contains("memory") {
        Some(Domain::Dram)
    } else if l.contains("cpu") || l.contains("package") || l.starts_with("esocket") || l.contains("socket") {
        Some(Domain::Cpu)
    } else {
        None
    }
}

/// Socket number in a label such as `"CPU Power Socket 1"` or `"Esocket0"`, else 0.
pub fn socket_of(label: &str) -> i16 {
    let l = label.to_ascii_lowercase();
    l.find("socket")
        .map(|i| &l[i + "socket".len()..])
        .and_then(|rest| {
            let digits: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        })
        .unwrap_or(0)
}

#[derive(Debug)]
enum Source {
    /// Cumulative µJ counter.
    Energy { path: PathBuf, prev_uj: f64, max_seen_uj: f64 },
    /// Power in µW, integrated between reads.
    Power { path: PathBuf, prev_w: f64 },
}

pub struct HwmonSensor {
    sources: Vec<Source>,
    channels: Vec<Channel>,
    prev_at: Instant,
}

fn read_number(path: &Path) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_label(dir: &Path, stem: &str) -> Option<String> {
    Some(fs::read_to_string(dir.join(format!("{stem}_label"))).ok()?.trim().to_string())
}

/// `energy3_input` → ("energy", "energy3"); other files → None.
fn channel_stem(file: &str) -> Option<(&str, &str)> {
    for (kind, suffix) in [("energy", "_input"), ("power", "_input"), ("power", "_average")] {
        if let Some(stem) = file.strip_suffix(suffix)
            && let Some(n) = stem.strip_prefix(kind)
            && !n.is_empty()
            && n.chars().all(|c| c.is_ascii_digit())
        {
            return Some((kind, stem));
        }
    }
    None
}

impl HwmonSensor {
    /// Opens every classifiable, readable channel under `root`. `None` if there are none.
    pub fn open(root: &Path) -> Option<Self> {
        let mut devices: Vec<PathBuf> = fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
        devices.sort();

        let mut sources = Vec::new();
        let mut channels = Vec::new();
        for dir in devices {
            let chip = fs::read_to_string(dir.join("name")).map(|s| s.trim().to_string()).unwrap_or_default();
            let mut files: Vec<String> = fs::read_dir(&dir)
                .map(|it| it.flatten().filter_map(|e| e.file_name().into_string().ok()).collect())
                .unwrap_or_default();
            // Energy before power, so a power channel duplicating an energy label is skipped.
            files.sort_by_key(|f| (!f.starts_with("energy"), f.clone()));

            let mut seen_labels = HashSet::new();
            let mut found: Vec<(Source, Channel, String)> = Vec::new();
            for file in files {
                let Some((kind, stem)) = channel_stem(&file) else { continue };
                let Some(label) = read_label(&dir, stem) else { continue };
                let Some(domain) = classify(&label) else { continue };
                if !seen_labels.insert(label.clone()) {
                    continue;
                }
                let path = dir.join(&file);
                let Some(value) = read_number(&path) else { continue };
                let (source, quality) = match kind {
                    "energy" => (Source::Energy { path, prev_uj: value, max_seen_uj: value }, Quality::Measured),
                    _ => (Source::Power { path, prev_w: value / 1e6 }, Quality::Estimated),
                };
                let channel = Channel {
                    domain,
                    socket: socket_of(&label),
                    name: if chip.is_empty() { label.clone() } else { format!("{chip}: {label}") },
                    quality,
                };
                found.push((source, channel, label.to_ascii_lowercase()));
            }
            let explicit_cpu = found.iter().any(|(_, c, l)| c.domain == Domain::Cpu && l.contains("cpu"));
            for (source, channel, label) in found {
                if explicit_cpu && channel.domain == Domain::Cpu && !label.contains("cpu") {
                    continue;
                }
                sources.push(source);
                channels.push(channel);
            }
        }
        (!channels.is_empty()).then(|| HwmonSensor { sources, channels, prev_at: Instant::now() })
    }
}

impl EnergySensor for HwmonSensor {
    fn name(&self) -> &'static str {
        "hwmon"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, _tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let now = Instant::now();
        let secs = now.duration_since(self.prev_at).as_secs_f64();
        self.prev_at = now;
        self.sources
            .iter_mut()
            .map(|source| match source {
                Source::Energy { path, prev_uj, max_seen_uj } => {
                    let curr = read_number(path)
                        .ok_or_else(|| SensorError::new("hwmon", format!("could not read {}", path.display())))?;
                    // 64-bit counters do not wrap in practice; a decrease is a reset.
                    let joules = reset_delta(*prev_uj, curr, *max_seen_uj) / 1e6;
                    *max_seen_uj = max_seen_uj.max(curr);
                    *prev_uj = curr;
                    Ok(Reading::energy(joules))
                }
                Source::Power { path, prev_w } => {
                    let watts = read_number(path)
                        .ok_or_else(|| SensorError::new("hwmon", format!("could not read {}", path.display())))?
                        / 1e6;
                    let joules = (*prev_w + watts) / 2.0 * secs;
                    *prev_w = watts;
                    Ok(Reading::energy(joules))
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One hwmon device: (dir, name, [(file, value)]).
    type FakeDevice<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

    fn fake(devices: &[FakeDevice]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for (dir, name, files) in devices {
            let d = root.path().join(dir);
            fs::create_dir(&d).unwrap();
            fs::write(d.join("name"), format!("{name}\n")).unwrap();
            for (file, value) in *files {
                fs::write(d.join(file), format!("{value}\n")).unwrap();
            }
        }
        root
    }

    #[test]
    fn classifies_labels() {
        assert_eq!(classify("CPU Power Socket 0"), Some(Domain::Cpu));
        assert_eq!(classify("Grace Power Socket 1"), Some(Domain::Cpu));
        assert_eq!(classify("Esocket0"), Some(Domain::Cpu));
        assert_eq!(classify("CPU power"), Some(Domain::Cpu));
        assert_eq!(classify("DRAM power"), Some(Domain::Dram));
        for skipped in ["Module Power Socket 0", "SysIO Power Socket 0", "IO power", "VDD_CPU_GPU_CV", "Ecore000", "temp1"] {
            assert_eq!(classify(skipped), None, "{skipped}");
        }
    }

    #[test]
    fn parses_socket_numbers() {
        assert_eq!(socket_of("CPU Power Socket 1"), 1);
        assert_eq!(socket_of("Esocket3"), 3);
        assert_eq!(socket_of("CPU power"), 0);
    }

    #[test]
    fn grace_style_power_channels() {
        // NVIDIA Grace: power telemetry per socket in µW; module and I/O rails skipped.
        let root = fake(&[(
            "hwmon2",
            "grace_power",
            &[
                ("power1_average", "120000000"),
                ("power1_label", "Module Power Socket 0"),
                ("power5_average", "95000000"),
                ("power5_label", "Grace Power Socket 0"), // CPU + memory: dropped, CPU channels exist
                ("power2_average", "80000000"),
                ("power2_label", "CPU Power Socket 0"),
                ("power3_average", "15000000"),
                ("power3_label", "SysIO Power Socket 0"),
                ("power4_average", "70000000"),
                ("power4_label", "CPU Power Socket 1"),
            ],
        )]);
        let opened = Instant::now();
        let mut s = HwmonSensor::open(root.path()).unwrap();
        let ch: Vec<(Domain, i16, Quality)> =
            s.channels().iter().map(|c| (c.domain.clone(), c.socket, c.quality)).collect();
        assert_eq!(ch, [(Domain::Cpu, 0, Quality::Estimated), (Domain::Cpu, 1, Quality::Estimated)]);
        assert_eq!(s.channels()[0].name, "grace_power: CPU Power Socket 0");

        std::thread::sleep(std::time::Duration::from_millis(100));
        fs::write(root.path().join("hwmon2/power2_average"), "100000000\n").unwrap();
        let r = s.read(&[]).unwrap();
        // Trapezoid of 80 W → 100 W: between 80 W and 100 W over the time
        // between open and read (at least the 0.1 s slept).
        let max_secs = opened.elapsed().as_secs_f64();
        assert!(r[0].joules >= 80.0 * 0.1 && r[0].joules <= 100.0 * max_secs, "{} J in {max_secs} s", r[0].joules);
    }

    #[test]
    fn energy_counters_are_preferred_and_measured() {
        let root = fake(&[(
            "hwmon0",
            "amd_energy",
            &[
                ("energy1_input", "1000000"),
                ("energy1_label", "Esocket0"),
                ("energy2_input", "5"),
                ("energy2_label", "Ecore000"),
                ("power1_input", "50000000"),
                ("power1_label", "Esocket0"), // same label as energy1 → skipped
                ("temp1_input", "40000"),
            ],
        )]);
        let mut s = HwmonSensor::open(root.path()).unwrap();
        assert_eq!(s.channels().len(), 1);
        assert_eq!(s.channels()[0].quality, Quality::Measured);
        fs::write(root.path().join("hwmon0/energy1_input"), "3500000\n").unwrap();
        assert_eq!(s.read(&[]).unwrap()[0].joules, 2.5);
        // A reset (driver reload) counts the energy since the reset.
        fs::write(root.path().join("hwmon0/energy1_input"), "200000\n").unwrap();
        assert_eq!(s.read(&[]).unwrap()[0].joules, 0.2);
    }

    #[test]
    fn nothing_classifiable_is_none() {
        let root = fake(&[("hwmon0", "coretemp", &[("temp1_input", "40000"), ("temp1_label", "Package id 0")])]);
        assert!(HwmonSensor::open(root.path()).is_none());
        assert!(HwmonSensor::open(Path::new("/definitely/not/here")).is_none());
    }
}

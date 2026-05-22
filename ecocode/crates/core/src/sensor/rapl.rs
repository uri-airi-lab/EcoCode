//! RAPL (Running Average Power Limit) sensor reader.
//!
//! Reads per-socket, per-domain energy counters exposed by the Linux kernel at
//! `/sys/class/powercap/intel-rapl/`. Supported domains: `cpu` (core), `dram`,
//! and `igpu` (uncore / integrated graphics).
//!
//! Energy values are read in microjoules (uJ) directly from sysfs and converted
//! to Joules before being attributed to the monitored process.

use crate::sensor::RAPL_PATH;
use regex::Regex;
use std::error::Error;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// An open file handle for a single RAPL energy counter, kept open across samples
/// to avoid repeated `open()` syscalls on every tick.
#[derive(Debug)]
pub struct RaplFile {
    /// CPU socket index (0-based). Multi-socket systems have one entry per socket.
    pub socket: i16,
    /// Power domain: `"cpu"`, `"dram"`, or `"igpu"`.
    pub domain: String,
    /// Buffered reader positioned at the `energy_uj` sysfs file.
    pub file: BufReader<File>,
    /// Counter ceiling read from `max_energy_range_uj`, used to detect wrap-around.
    pub max_energy: f64,
    /// Reusable buffer for reading energy values, avoids allocation on every sample.
    buffer: String,
}

/// A snapshot of a RAPL energy counter at one point in time (in microjoules).
#[derive(Debug)]
pub struct RaplEnergy {
    pub socket: i16,
    /// Power domain: `"cpu"`, `"dram"`, or `"igpu"`.
    pub domain: String,
    /// Absolute energy counter value in microjoules at this sample.
    pub energy: f64,
    /// Counter ceiling from `max_energy_range_uj`; used for wrap-around correction.
    pub max_energy: f64,
}

/// Per-domain energy consumed (in Joules) between two consecutive samples,
/// already weighted by the process's resource-usage fraction.
#[derive(Debug)]
pub struct RaplEnergyDelta {
    pub socket: i16,
    /// Power domain: `"cpu"`, `"dram"`, or `"igpu"`.
    pub domain: String,
    /// Energy attributed to the monitored process (Joules).
    pub delta_energy: f64,
}

/// Reads the current energy counter value (microjoules) from an open sysfs file.
/// The file is seeked back to position 0 before every read because the kernel
/// updates it in-place rather than appending new data.
pub fn get_energy(rapl_file: &mut RaplFile) -> Result<f64, Box<dyn Error>> {
    rapl_file.file.seek(SeekFrom::Start(0))?;
    rapl_file.buffer.clear();
    rapl_file.file.read_line(&mut rapl_file.buffer)?;
    Ok(rapl_file.buffer.trim().parse::<f64>()?)
}

/// Reads the hardware counter ceiling from `max_energy_range_uj`.
/// Used to detect and correct wrap-around between consecutive samples.
fn get_max_energy(path: &Path) -> Result<f64, Box<dyn Error>> {
    Ok(std::fs::read_to_string(path)?.trim().parse::<f64>()?)
}

/// Reads the current energy counter from every open RAPL file handle.
/// Returns one [`RaplEnergy`] per (socket, domain) pair, in the same order as
/// the input `rapl_readers` slice.
pub fn get_all_energies(
    rapl_readers: &mut Vec<RaplFile>,
) -> Result<Vec<RaplEnergy>, Box<dyn Error>> {
    let mut rapl_energies = Vec::new();
    for reader in rapl_readers {
        let energy = get_energy(reader)?;
        let rapl_energy = RaplEnergy {
            socket: reader.socket,
            domain: reader.domain.clone(),
            energy,
            max_energy: reader.max_energy,
        };
        rapl_energies.push(rapl_energy);
    }
    Ok(rapl_energies)
}

/// Computes the energy (in Joules) attributed to a specific process for each
/// RAPL domain between two consecutive snapshots.
///
/// Attribution is proportional: each domain's raw delta energy is multiplied by
/// the process's fractional share of the relevant resource:
/// - `cpu` domain  → weighted by `cpu_usage / 100`
/// - `dram` domain → weighted by `mem_usage / 100`
/// - `igpu` domain → weighted by `igpu_usage / 100` (proxied from CPU usage;
///   no per-process iGPU sensor exists in Linux)
///
/// Counter wrap-around is handled: if `energy_2 < energy_1`, the hardware
/// counter has rolled over and we add `max_energy` to reconstruct the true delta.
pub fn delta_cpu_energy_per_pid_j(
    energies_1: &[RaplEnergy],
    energies_2: &[RaplEnergy],
    cpu_usage: f64,
    mem_usage: f64,
    igpu_usage: f64,
) -> Vec<RaplEnergyDelta> {
    energies_1
        .iter()
        .zip(energies_2.iter())
        .map(|(e1, e2)| {
            // Handle counter reset logic
            let energy_2_corrected = if e2.energy < e1.energy {
                e2.energy + e1.max_energy
            } else {
                e2.energy
            };

            // Map domains to their respective usage fractions
            //
            // NOTE: DRAM attribution uses RSS (resident set size) as a proxy,
            // which measures memory *capacity* (how much RAM is allocated), not
            // memory *bandwidth* (how often RAM is accessed). DRAM energy is
            // primarily driven by read/write activity, so RSS can over-attribute
            // to large-but-idle processes and under-attribute to small-but-active ones.
            //
            // A more accurate approach would blend static and dynamic components:
            //   let static_fraction = 0.3; // ~30% of DRAM power is refresh/leakage
            //   "dram" => static_fraction * mem_usage + (1.0 - static_fraction) * cpu_usage
            // This attributes refresh energy by RSS (fair — all pages need refreshing)
            // and dynamic access energy by CPU usage (better bandwidth proxy).
            let usage_fraction = match e1.domain.as_str() {
                "cpu"  => cpu_usage,
                "dram" => mem_usage,
                "igpu" => igpu_usage,
                _      => cpu_usage,
            } / 100.0;

            // Calculate delta and ensure it's non-negative
            let delta_j = (((energy_2_corrected - e1.energy) / 1_000_000.0) * usage_fraction).max(0.0);

            RaplEnergyDelta {
                socket: e1.socket,
                domain: e1.domain.clone(),
                delta_energy: delta_j,
            }
        })
        .collect::<Vec<RaplEnergyDelta>>() //returned because it is the last variable in the function (no need for let variable = then reutrn)
                         // more idiomatic Rust way
}

/// Scans the RAPL sysfs hierarchy and opens a buffered file handle for every
/// discovered energy counter.
///
/// Directory layout expected:
/// ```text
/// /sys/class/powercap/intel-rapl/
///   intel-rapl:0/          ← socket 0
///     intel-rapl:0:0/      ← sub-domain (core / dram / uncore)
///     intel-rapl:0:1/
///   intel-rapl:1/          ← socket 1 (multi-socket systems)
///     ...
/// ```
/// Domain names are normalised: `"core"` → `"cpu"`, `"uncore"` → `"igpu"`.
/// The returned vector is sorted alphabetically by domain so that paired calls
/// to [`get_all_energies`] always produce entries in a consistent order.
pub fn scan_rapl_files() -> Result<Vec<RaplFile>, Box<dyn Error>> {
    let mut rapl_files = Vec::new();

    let re_socket = Regex::new(r"^intel-rapl:(\d+)$")?;
    let re_domain = Regex::new(r"^intel-rapl:\d+:\d+$")?;

    let entries = read_dir(RAPL_PATH)?;

    for entry in entries {
        // socket level lookout
        let entry = match entry {
            Ok(e) => e.path(),
            Err(_) => continue,
        };

        let dir_name = match entry.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let socket: i16 = match re_socket.captures(dir_name).and_then(|c| c.get(1)) {
            Some(m) => match m.as_str().parse() {
                Ok(s) => s,
                Err(_) => continue,
            },
            None => continue, // skip non-socket dirs
        };

        // domain level lookout
        let sub_entries = match entry.read_dir() {
            Ok(se) => se,
            Err(_) => continue,
        };

        for sub_entry in sub_entries {
            let ent_path = match sub_entry {
                Ok(se) => se.path(),
                Err(_) => continue,
            };
            let mut ent = ent_path.clone();
            let name_path = ent.join("name");
            let raw_name =
                std::fs::read_to_string(name_path).unwrap_or_else(|_| "unknown".to_string());
            
            let entf = match ent_path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if ent.is_dir() && re_domain.is_match(entf) {
                let name = match raw_name.trim() {
                    "core" => "cpu".to_string(),
                    "dram" => "dram".to_string(),
                    "uncore" | "graphics" => "igpu".to_string(),
                    other => other.to_string(),
                };
                let max_energy_path = ent.join("max_energy_range_uj");
                ent.push("energy_uj");
                let file = match File::open(&ent) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let file = BufReader::new(file);

                let max_energy = match get_max_energy(&max_energy_path) {
                    Ok(me) => me,
                    Err(_) => continue,
                };

                let file = RaplFile {
                    socket,
                    domain: name,
                    file,
                    max_energy,
                    buffer: String::new(),
                };
                rapl_files.push(file);
            }
        }
    }
    rapl_files.sort_by(|a, b| a.domain.cmp(&b.domain));

    Ok(rapl_files)
}

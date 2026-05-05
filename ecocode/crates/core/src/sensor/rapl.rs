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
pub fn get_energy(rapl_file: &mut BufReader<File>) -> Result<f64, Box<dyn Error>> {
    rapl_file.seek(SeekFrom::Start(0))?;
    let mut buffer = String::new();
    rapl_file.read_line(&mut buffer)?;
    let energy_consumed = buffer.trim().parse::<f64>()?;
    Ok(energy_consumed)
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
        let energy = get_energy(&mut reader.file)?;
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
    _elapsed_secs: f64,
    cpu_usage: f64,
    mem_usage: f64,
    igpu_usage: f64,
) -> Vec<RaplEnergyDelta> {
    if energies_1.len() != energies_2.len() {
        panic!("energy vectors must have the same length");
    };
    let mut delta_energy = Vec::new();
    for i in 0..energies_1.len() {
        let energy_1 = energies_1[i].energy;
        let energy_2 = energies_2[i].energy;
        let max_energy = energies_1[i].max_energy; // in microjoules

        // Handle counter reset
        let energy_2 = if energy_2 < energy_1 {
            energy_2 + max_energy
        } else {
            energy_2
        };

        // Each domain is attributed using the process's share of the relevant resource.
        // Domains absent on this system simply won't appear in the energies vectors,
        // so missing DRAM or iGPU domains are handled naturally — no entries, no delta.
        let usage_fraction = match energies_1[i].domain.as_str() {
            "cpu" => cpu_usage / 100.0,
            "dram" => mem_usage / 100.0,
            // No dedicated per-process iGPU sensor exists; cpu_usage is the best proxy
            // for processes that drive integrated graphics alongside CPU work.
            "igpu" => igpu_usage / 100.0,
            _ => cpu_usage / 100.0,
        };

        let delta_energy_per_pid =
            (((energy_2 - energy_1) / 1_000_000.0) * usage_fraction).max(0.0);

        delta_energy.push(RaplEnergyDelta {
            socket: energies_1[i].socket,
            domain: energies_1[i].domain.clone(),
            delta_energy: delta_energy_per_pid,
        });
    }

    delta_energy
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
        //socket level lookout
        let entry = entry.unwrap().path();

        let dir_name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Failed to convert directory name to string")?;

        let socket: i16 = match re_socket.captures(dir_name).and_then(|c| c.get(1)) {
            Some(m) => m.as_str().parse()?,
            None => continue, // skip non-socket dirs
        };

        //domain level lookout
        for sub_entry in entry.read_dir()? {
            let mut ent = sub_entry.unwrap().path();
            let name_path = ent.join("name");
            let raw_name =
                std::fs::read_to_string(name_path).unwrap_or_else(|_| "unknown".to_string());
            let entf = ent.file_name().unwrap().to_str().unwrap();
            if ent.is_dir() && re_domain.is_match(&entf) {
                let name = match raw_name.trim() {
                    "core" => "cpu".to_string(),
                    "dram" => "dram".to_string(),
                    "uncore" => "igpu".to_string(),
                    other => other.to_string(),
                };
                let max_energy_path = ent.join("max_energy_range_uj");
                ent.push("energy_uj");
                let file = File::open(&ent)?;
                let file = BufReader::new(file);

                let file = RaplFile {
                    socket: socket,
                    domain: name,
                    file: file,
                    max_energy: get_max_energy(&max_energy_path)?,
                };
                rapl_files.push(file);
            }
        }
    }
    rapl_files.sort_by(|a, b| a.domain.cmp(&b.domain));

    Ok(rapl_files)
}

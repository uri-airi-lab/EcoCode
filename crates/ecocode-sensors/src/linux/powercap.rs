//! RAPL (Running Average Power Limit) sensor reader.
//!
//! Reads per-socket, per-domain energy counters exposed by the Linux kernel at
//! `/sys/class/powercap/intel-rapl/` (Intel and AMD). Supported domains: `cpu`
//! (core), `dram`, and `igpu` (uncore / integrated graphics).
//!
//! Energy values are read in microjoules (uJ) directly from sysfs and converted
//! to Joules before being attributed to the monitored process.

use regex::Regex;
use std::error::Error;
use std::fs::{File, read_dir};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use crate::common::counter::microjoules_delta_j;
use crate::domain::{Channel, Domain, Quality, Reading};
use crate::{EnergySensor, SensorError};

/// Root of the RAPL powercap hierarchy.
pub const RAPL_PATH: &str = "/sys/class/powercap/intel-rapl/";

/// An open file handle for a single RAPL energy counter, kept open across samples
/// to avoid repeated `open()` syscalls on every tick.
#[derive(Debug)]
pub struct RaplFile {
    /// CPU socket index, 0-based.
    pub socket: i16,
    /// `"cpu"`, `"dram"`, `"igpu"`, or the raw sysfs name for anything else.
    pub domain: String,
    /// Reader positioned at the `energy_uj` sysfs file.
    pub file: BufReader<File>,
    /// Counter ceiling from `max_energy_range_uj`, used to detect wrap-around.
    pub max_energy: f64,
    /// Reused across samples to avoid allocating on every tick.
    buffer: String,
}

/// A snapshot of a RAPL energy counter at one point in time, in microjoules.
#[derive(Debug)]
pub struct RaplEnergy {
    pub socket: i16,
    pub domain: String,
    pub energy: f64,
    pub max_energy: f64,
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

/// Scans the RAPL sysfs hierarchy at [`RAPL_PATH`] and opens a buffered file
/// handle for every discovered energy counter.
pub fn scan_rapl_files() -> Result<Vec<RaplFile>, Box<dyn Error>> {
    scan_rapl_files_in(Path::new(RAPL_PATH))
}

/// Scans a RAPL sysfs hierarchy rooted at `root`.
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
pub fn scan_rapl_files_in(root: &Path) -> Result<Vec<RaplFile>, Box<dyn Error>> {
    let mut rapl_files = Vec::new();

    let re_socket = Regex::new(r"^intel-rapl:(\d+)$")?;
    let re_domain = Regex::new(r"^intel-rapl:\d+:\d+$")?;

    let entries = read_dir(root)?;

    for entry in entries {
        // Each entry here is a socket directory, e.g. intel-rapl:0
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

        // Each sub-entry here is a domain directory, e.g. intel-rapl:0:0
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

/// Why [`RaplSensor::open`] found nothing to read.
#[derive(Debug)]
pub enum OpenError {
    /// Energy counters exist but this user cannot read them.
    PermissionDenied,
    /// The hierarchy has no energy domains.
    NoDomains,
    Io(Box<dyn Error>),
}

/// Energy of every RAPL domain on every socket.
pub struct RaplSensor {
    files: Vec<RaplFile>,
    channels: Vec<Channel>,
    /// Previous counter value per domain (µJ).
    prev_uj: Vec<f64>,
}

impl RaplSensor {
    /// Opens every readable energy counter under `root`.
    pub fn open(root: &Path) -> Result<Self, OpenError> {
        let mut files = scan_rapl_files_in(root).map_err(OpenError::Io)?;
        if files.is_empty() {
            return Err(if has_unreadable_counter(root) {
                OpenError::PermissionDenied
            } else {
                OpenError::NoDomains
            });
        }
        let prev_uj = get_all_energies(&mut files)
            .map_err(OpenError::Io)?
            .into_iter()
            .map(|e| e.energy)
            .collect();
        let channels = files
            .iter()
            .map(|f| Channel {
                domain: match f.domain.as_str() {
                    "cpu" => Domain::Cpu,
                    "dram" => Domain::Dram,
                    "igpu" => Domain::Igpu,
                    other => Domain::Other(other.to_string()),
                },
                socket: f.socket,
                name: format!("package-{} {}", f.socket, f.domain),
                quality: Quality::Measured,
            })
            .collect();
        Ok(RaplSensor { files, channels, prev_uj })
    }

    pub fn channels_ref(&self) -> &[Channel] {
        &self.channels
    }
}

impl EnergySensor for RaplSensor {
    fn name(&self) -> &'static str {
        "powercap"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, _tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let energies =
            get_all_energies(&mut self.files).map_err(|e| SensorError::new("powercap", e.to_string()))?;
        Ok(energies
            .iter()
            .zip(self.prev_uj.iter_mut())
            .map(|(e, prev)| {
                let joules = microjoules_delta_j(*prev, e.energy, e.max_energy);
                *prev = e.energy;
                Reading::energy(joules)
            })
            .collect())
    }
}

/// True if some domain's `energy_uj` exists but cannot be opened (since Linux
/// 5.10 the counters are root-only by default).
fn has_unreadable_counter(root: &Path) -> bool {
    let Ok(sockets) = read_dir(root) else { return false };
    sockets.flatten().any(|socket| {
        let Ok(domains) = read_dir(socket.path()) else { return false };
        domains.flatten().any(|d| {
            let f = d.path().join("energy_uj");
            f.exists() && File::open(&f).is_err()
        })
    })
}

#[cfg(all(test, not(windows)))] // the fake sysfs tree uses "intel-rapl:0" names, which Windows rejects
mod tests {
    use super::*;
    use std::fs;

    /// (subdir, name, energy_uj, max_energy_range_uj)
    type FakeDomain<'a> = (&'a str, &'a str, &'a str, &'a str);

    /// Builds a fake powercap tree: one socket dir per entry, each holding domains.
    fn fake_tree(sockets: &[(&str, &[FakeDomain])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (socket_dir, domains) in sockets {
            let sp = dir.path().join(socket_dir);
            fs::create_dir(&sp).unwrap();
            fs::write(sp.join("name"), "package-0\n").unwrap();
            for (sub, name, energy, max) in *domains {
                let dp = sp.join(sub);
                fs::create_dir(&dp).unwrap();
                fs::write(dp.join("name"), format!("{name}\n")).unwrap();
                fs::write(dp.join("energy_uj"), format!("{energy}\n")).unwrap();
                fs::write(dp.join("max_energy_range_uj"), format!("{max}\n")).unwrap();
            }
        }
        dir
    }

    #[test]
    fn scan_normalises_names_and_sorts_by_domain() {
        let tree = fake_tree(&[
            ("intel-rapl:0", &[
                ("intel-rapl:0:0", "core", "1000", "262143328850"),
                ("intel-rapl:0:1", "uncore", "20", "262143328850"),
                ("intel-rapl:0:2", "dram", "300", "65712999613"),
            ]),
            ("intel-rapl:1", &[("intel-rapl:1:0", "core", "4000", "262143328850")]),
        ]);
        let mut files = scan_rapl_files_in(tree.path()).unwrap();
        let mut seen: Vec<(i16, String)> =
            files.iter().map(|f| (f.socket, f.domain.clone())).collect();
        // Sort is by domain only and stable over directory order, which is
        // filesystem-dependent, so compare domain order then the set.
        let domains: Vec<&str> = seen.iter().map(|(_, d)| d.as_str()).collect();
        assert_eq!(domains, ["cpu", "cpu", "dram", "igpu"]);
        seen.sort();
        assert_eq!(
            seen,
            [(0, "cpu".into()), (0, "dram".into()), (0, "igpu".into()), (1, "cpu".into())]
        );

        let energies = get_all_energies(&mut files).unwrap();
        let dram = energies.iter().find(|e| e.domain == "dram").unwrap();
        assert_eq!(dram.energy, 300.0);
        assert_eq!(dram.max_energy, 65_712_999_613.0);
    }

    #[test]
    fn scan_skips_non_socket_dirs_and_package_level_files() {
        let tree = fake_tree(&[("intel-rapl:0", &[("intel-rapl:0:0", "core", "1", "10")])]);
        fs::create_dir(tree.path().join("intel-rapl-mmio:0")).unwrap();
        // Package-level energy (intel-rapl:0/energy_uj) is not a sub-domain and
        // is not read; CPU energy comes from the core sub-domain.
        fs::write(tree.path().join("intel-rapl:0").join("energy_uj"), "999\n").unwrap();
        let files = scan_rapl_files_in(tree.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].domain, "cpu");
    }

    #[test]
    fn scan_skips_domain_without_max_energy() {
        let tree = fake_tree(&[("intel-rapl:0", &[("intel-rapl:0:0", "core", "1", "10")])]);
        fs::remove_file(tree.path().join("intel-rapl:0/intel-rapl:0:0/max_energy_range_uj"))
            .unwrap();
        assert!(scan_rapl_files_in(tree.path()).unwrap().is_empty());
    }

    #[test]
    fn get_energy_rereads_updated_value() {
        let tree = fake_tree(&[("intel-rapl:0", &[("intel-rapl:0:0", "core", "100", "10")])]);
        let mut files = scan_rapl_files_in(tree.path()).unwrap();
        assert_eq!(get_energy(&mut files[0]).unwrap(), 100.0);
        fs::write(tree.path().join("intel-rapl:0/intel-rapl:0:0/energy_uj"), "250\n").unwrap();
        assert_eq!(get_energy(&mut files[0]).unwrap(), 250.0);
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(scan_rapl_files_in(Path::new("/definitely/not/here")).is_err());
    }

    #[test]
    fn sensor_reports_joules_since_previous_read() {
        let tree = fake_tree(&[("intel-rapl:0", &[
            ("intel-rapl:0:0", "core", "1000000", "262143328850"),
            ("intel-rapl:0:1", "psys", "5", "100"),
        ])]);
        let mut sensor = RaplSensor::open(tree.path()).unwrap();
        let domains: Vec<_> = sensor.channels().iter().map(|c| c.domain.clone()).collect();
        assert_eq!(domains, [Domain::Cpu, Domain::Other("psys".into())]);
        fs::write(tree.path().join("intel-rapl:0/intel-rapl:0:0/energy_uj"), "3500000\n").unwrap();
        fs::write(tree.path().join("intel-rapl:0/intel-rapl:0:1/energy_uj"), "3\n").unwrap(); // wrapped at 100
        let r = sensor.read(&[]).unwrap();
        assert_eq!(r[0].joules, 2.5);
        assert_eq!(r[1].joules, 98.0 / 1_000_000.0);
        // Next read is relative to the previous one.
        assert_eq!(sensor.read(&[]).unwrap()[0].joules, 0.0);
    }

    #[test]
    fn empty_hierarchy_is_no_domains() {
        let tree = fake_tree(&[]);
        assert!(matches!(RaplSensor::open(tree.path()), Err(OpenError::NoDomains)));
    }

    #[cfg(unix)]
    #[test]
    fn root_only_counters_are_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        let tree = fake_tree(&[("intel-rapl:0", &[("intel-rapl:0:0", "core", "1", "10")])]);
        let f = tree.path().join("intel-rapl:0/intel-rapl:0:0/energy_uj");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o000)).unwrap();
        if File::open(&f).is_ok() {
            return; // running as root: permissions are not enforced
        }
        assert!(matches!(RaplSensor::open(tree.path()), Err(OpenError::PermissionDenied)));
    }
}

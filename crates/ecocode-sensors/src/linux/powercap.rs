//! RAPL (Running Average Power Limit) sensor reader.
//!
//! Reads per-socket energy counters exposed by the Linux kernel at
//! `/sys/class/powercap/intel-rapl/` (Intel and AMD). Each socket yields `cpu`,
//! and `dram` and `igpu` (integrated graphics) where the hardware has them.
//!
//! `cpu` is the socket's package counter: the cores together with the caches,
//! memory controller and interconnect they rely on. The integrated GPU sits inside
//! the package too, so its counter is subtracted from `cpu` and reported on its
//! own. DRAM is outside the package and is reported as it is.
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
    /// `"cpu"`, `"dram"` or `"igpu"`.
    pub domain: String,
    /// The sysfs zone behind `domain`, for diagnostics: `"package-0"`, `"dram"`,
    /// `"uncore"` and so on.
    pub counter: String,
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

/// Which reported domain a RAPL zone feeds, if any.
///
/// A socket's top-level zone is `package-N`; its sub-zones split part of it out.
/// The package gives `cpu`. `dram` is outside the package and is read as is.
/// `uncore`/`graphics` is the integrated GPU, read so it can be reported apart
/// from `cpu`. Every other sub-zone (`core` and the like) is already inside the
/// package and is not read. Top-level zones other than packages, such as `psys`
/// (the whole platform), are skipped so nothing is counted twice.
fn domain_for_zone(zone_name: &str, top_level: bool) -> Option<&'static str> {
    if top_level {
        return zone_name.starts_with("package").then_some("cpu");
    }
    match zone_name {
        "dram" => Some("dram"),
        "uncore" | "graphics" => Some("igpu"),
        _ => None,
    }
}

/// Opens one zone's `energy_uj`, or `None` if it or its counter ceiling is unreadable.
fn open_zone(dir: &Path, socket: i16, domain: &str, counter: &str) -> Option<RaplFile> {
    let max_energy = get_max_energy(&dir.join("max_energy_range_uj")).ok()?;
    let file = BufReader::new(File::open(dir.join("energy_uj")).ok()?);
    Some(RaplFile {
        socket,
        domain: domain.to_string(),
        counter: counter.to_string(),
        file,
        max_energy,
        buffer: String::new(),
    })
}

fn zone_name(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("name")).unwrap_or_default().trim().to_string()
}

/// Scans a RAPL sysfs hierarchy rooted at `root`.
///
/// Directory layout expected:
/// ```text
/// /sys/class/powercap/intel-rapl/
///   intel-rapl:0/          ← socket 0 package (name "package-0")
///     intel-rapl:0:0/      ← sub-zone (core / dram / uncore)
///     intel-rapl:0:1/
///   intel-rapl:1/          ← socket 1 on multi-socket systems, or "psys"
///     ...
/// ```
/// See [`domain_for_zone`] for which zones are read. The returned vector is
/// sorted by domain so that paired calls to [`get_all_energies`] always produce
/// entries in a consistent order.
pub fn scan_rapl_files_in(root: &Path) -> Result<Vec<RaplFile>, Box<dyn Error>> {
    let mut rapl_files = Vec::new();

    let re_socket = Regex::new(r"^intel-rapl:(\d+)$")?;
    let re_sub_zone = Regex::new(r"^intel-rapl:\d+:\d+$")?;

    for entry in read_dir(root)?.flatten() {
        let socket_dir = entry.path();
        let Some(dir_name) = socket_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(socket) = re_socket
            .captures(dir_name)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<i16>().ok())
        else {
            continue; // not a top-level zone
        };

        let package_name = zone_name(&socket_dir);
        let Some(domain) = domain_for_zone(&package_name, true) else {
            continue; // psys and other non-package zones
        };
        let Some(package) = open_zone(&socket_dir, socket, domain, &package_name) else {
            continue;
        };
        rapl_files.push(package);

        let Ok(sub_entries) = socket_dir.read_dir() else { continue };
        for sub_entry in sub_entries.flatten() {
            let sub_dir = sub_entry.path();
            let is_sub_zone = sub_dir
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| re_sub_zone.is_match(name));
            if !is_sub_zone || !sub_dir.is_dir() {
                continue;
            }
            let name = zone_name(&sub_dir);
            if let Some(domain) = domain_for_zone(&name, false)
                && let Some(file) = open_zone(&sub_dir, socket, domain, &name)
            {
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
    /// (igpu index, cpu index) on the same socket: the iGPU's energy is part of
    /// the package counter behind `cpu` and is taken out of it.
    igpu_in_cpu: Vec<(usize, usize)>,
}

/// Pairs every `igpu` file with the `cpu` file of its socket.
fn igpu_cpu_pairs(files: &[RaplFile]) -> Vec<(usize, usize)> {
    files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.domain == "igpu")
        .filter_map(|(i, igpu)| {
            files
                .iter()
                .position(|f| f.domain == "cpu" && f.socket == igpu.socket)
                .map(|cpu| (i, cpu))
        })
        .collect()
}

/// Takes each socket's iGPU energy out of its package energy. The two counters
/// are read a moment apart, so the difference is clamped at zero.
fn separate_igpu(joules: &mut [f64], igpu_in_cpu: &[(usize, usize)]) {
    for &(igpu, cpu) in igpu_in_cpu {
        joules[cpu] = (joules[cpu] - joules[igpu]).max(0.0);
    }
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
        let igpu_in_cpu = igpu_cpu_pairs(&files);
        Ok(RaplSensor { files, channels, prev_uj, igpu_in_cpu })
    }

    pub fn channels_ref(&self) -> &[Channel] {
        &self.channels
    }

    /// One line per channel for `ecocode doctor`, naming the counter it reads,
    /// e.g. `socket 0 cpu (package-0 minus uncore)`.
    pub fn describe(&self) -> Vec<String> {
        self.files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut source = f.counter.clone();
                for &(igpu, cpu) in &self.igpu_in_cpu {
                    if cpu == i {
                        source = format!("{source} minus {}", self.files[igpu].counter);
                    }
                }
                format!("socket {} {} ({source})", f.socket, f.domain)
            })
            .collect()
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
        let mut joules: Vec<f64> = energies
            .iter()
            .zip(self.prev_uj.iter_mut())
            .map(|(e, prev)| {
                let j = microjoules_delta_j(*prev, e.energy, e.max_energy);
                *prev = e.energy;
                j
            })
            .collect();
        separate_igpu(&mut joules, &self.igpu_in_cpu);
        Ok(joules.into_iter().map(Reading::energy).collect())
    }
}

/// True if a zone's `energy_uj` exists but cannot be opened (since Linux 5.10
/// the counters are root-only by default).
fn has_unreadable_counter(root: &Path) -> bool {
    let unreadable = |dir: &Path| {
        let f = dir.join("energy_uj");
        f.exists() && File::open(&f).is_err()
    };
    let Ok(sockets) = read_dir(root) else { return false };
    sockets.flatten().any(|socket| {
        if unreadable(&socket.path()) {
            return true;
        }
        let Ok(domains) = read_dir(socket.path()) else { return false };
        domains.flatten().any(|d| unreadable(&d.path()))
    })
}

#[cfg(all(test, not(windows)))] // the fake sysfs tree uses "intel-rapl:0" names, which Windows rejects
mod tests {
    use super::*;
    use std::fs;

    /// (dir, name, energy_uj, max_energy_range_uj)
    type FakeZone<'a> = (&'a str, &'a str, &'a str, &'a str);
    const MAX: &str = "262143328850";

    fn write_zone(dir: &Path, name: &str, energy: &str, max: &str) {
        fs::create_dir(dir).unwrap();
        fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        fs::write(dir.join("energy_uj"), format!("{energy}\n")).unwrap();
        fs::write(dir.join("max_energy_range_uj"), format!("{max}\n")).unwrap();
    }

    /// Builds a fake powercap tree: one top-level zone per entry, each with sub-zones.
    fn fake_tree(zones: &[(FakeZone, &[FakeZone])]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for ((dir, name, energy, max), subs) in zones {
            let zone = root.path().join(dir);
            write_zone(&zone, name, energy, max);
            for (sub, sub_name, sub_energy, sub_max) in *subs {
                write_zone(&zone.join(sub), sub_name, sub_energy, sub_max);
            }
        }
        root
    }

    fn set_energy(root: &Path, zone: &str, uj: &str) {
        fs::write(root.join(zone).join("energy_uj"), format!("{uj}\n")).unwrap();
    }

    #[test]
    fn zone_roles() {
        assert_eq!(domain_for_zone("package-0", true), Some("cpu"));
        assert_eq!(domain_for_zone("package-1", true), Some("cpu"));
        assert_eq!(domain_for_zone("psys", true), None);
        assert_eq!(domain_for_zone("dram", false), Some("dram"));
        assert_eq!(domain_for_zone("uncore", false), Some("igpu"));
        assert_eq!(domain_for_zone("graphics", false), Some("igpu"));
        assert_eq!(domain_for_zone("core", false), None);
        assert_eq!(domain_for_zone("something-new", false), None);
    }

    #[test]
    fn scan_reads_package_dram_and_igpu_per_socket() {
        let tree = fake_tree(&[
            (("intel-rapl:0", "package-0", "5000", MAX), &[
                ("intel-rapl:0:0", "core", "1000", MAX),
                ("intel-rapl:0:1", "uncore", "20", MAX),
                ("intel-rapl:0:2", "dram", "300", "65712999613"),
            ]),
            (("intel-rapl:1", "package-1", "7000", MAX), &[("intel-rapl:1:0", "core", "4000", MAX)]),
        ]);
        let mut files = scan_rapl_files_in(tree.path()).unwrap();
        // Sorted by domain; order within a domain follows the directory listing.
        let domains: Vec<&str> = files.iter().map(|f| f.domain.as_str()).collect();
        assert_eq!(domains, ["cpu", "cpu", "dram", "igpu"]);
        let mut seen: Vec<(i16, &str, &str)> =
            files.iter().map(|f| (f.socket, f.domain.as_str(), f.counter.as_str())).collect();
        seen.sort();
        assert_eq!(
            seen,
            [(0, "cpu", "package-0"), (0, "dram", "dram"), (0, "igpu", "uncore"), (1, "cpu", "package-1")]
        );

        let energies = get_all_energies(&mut files).unwrap();
        let cpu0 = energies.iter().find(|e| e.domain == "cpu" && e.socket == 0).unwrap();
        assert_eq!(cpu0.energy, 5000.0);
        let dram = energies.iter().find(|e| e.domain == "dram").unwrap();
        assert_eq!(dram.energy, 300.0);
        assert_eq!(dram.max_energy, 65_712_999_613.0);
    }

    #[test]
    fn scan_skips_psys_and_non_rapl_dirs() {
        let tree = fake_tree(&[
            (("intel-rapl:0", "package-0", "1", "10"), &[]),
            // Platform energy (whole machine) would count the package twice.
            (("intel-rapl:1", "psys", "99", "1000"), &[]),
        ]);
        fs::create_dir(tree.path().join("intel-rapl-mmio:0")).unwrap();
        let files = scan_rapl_files_in(tree.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].domain.as_str(), files[0].counter.as_str()), ("cpu", "package-0"));
    }

    #[test]
    fn scan_skips_zone_without_max_energy() {
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "1", "10"), &[])]);
        fs::remove_file(tree.path().join("intel-rapl:0/max_energy_range_uj")).unwrap();
        assert!(scan_rapl_files_in(tree.path()).unwrap().is_empty());
    }

    #[test]
    fn get_energy_rereads_updated_value() {
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "100", "10"), &[])]);
        let mut files = scan_rapl_files_in(tree.path()).unwrap();
        assert_eq!(get_energy(&mut files[0]).unwrap(), 100.0);
        set_energy(tree.path(), "intel-rapl:0", "250");
        assert_eq!(get_energy(&mut files[0]).unwrap(), 250.0);
    }

    #[test]
    fn missing_root_is_an_error() {
        assert!(scan_rapl_files_in(Path::new("/definitely/not/here")).is_err());
    }

    #[test]
    fn sensor_reports_joules_with_igpu_taken_out_of_cpu() {
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "1000000", MAX), &[
            ("intel-rapl:0:0", "core", "0", MAX),
            ("intel-rapl:0:1", "uncore", "0", MAX),
            ("intel-rapl:0:2", "dram", "5", "100"),
        ])]);
        let mut sensor = RaplSensor::open(tree.path()).unwrap();
        let domains: Vec<_> = sensor.channels().iter().map(|c| c.domain.clone()).collect();
        assert_eq!(domains, [Domain::Cpu, Domain::Dram, Domain::Igpu]);

        set_energy(tree.path(), "intel-rapl:0", "4500000"); // package +3.5 J
        set_energy(tree.path(), "intel-rapl:0/intel-rapl:0:1", "1000000"); // iGPU +1 J
        set_energy(tree.path(), "intel-rapl:0/intel-rapl:0:2", "3"); // DRAM wrapped at 100
        let r = sensor.read(&[]).unwrap();
        assert_eq!(r[0].joules, 2.5); // package minus iGPU
        assert_eq!(r[1].joules, 98.0 / 1_000_000.0);
        assert_eq!(r[2].joules, 1.0);
        // Next read is relative to the previous one.
        assert!(sensor.read(&[]).unwrap().iter().all(|r| r.joules == 0.0));
    }

    #[test]
    fn cpu_never_goes_negative_when_igpu_reads_ahead() {
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "0", MAX), &[(
            "intel-rapl:0:0",
            "graphics",
            "0",
            MAX,
        )])]);
        let mut sensor = RaplSensor::open(tree.path()).unwrap();
        set_energy(tree.path(), "intel-rapl:0", "500000"); // +0.5 J
        set_energy(tree.path(), "intel-rapl:0/intel-rapl:0:0", "1000000"); // +1 J
        let r = sensor.read(&[]).unwrap();
        assert_eq!((r[0].joules, r[1].joules), (0.0, 1.0));
    }

    #[test]
    fn igpu_is_only_taken_from_its_own_socket() {
        let tree = fake_tree(&[
            (("intel-rapl:0", "package-0", "0", MAX), &[("intel-rapl:0:0", "uncore", "0", MAX)]),
            (("intel-rapl:1", "package-1", "0", MAX), &[]),
        ]);
        let mut sensor = RaplSensor::open(tree.path()).unwrap();
        set_energy(tree.path(), "intel-rapl:0", "3000000");
        set_energy(tree.path(), "intel-rapl:0/intel-rapl:0:0", "1000000");
        set_energy(tree.path(), "intel-rapl:1", "2000000");
        let r = sensor.read(&[]).unwrap();
        let by_socket = |domain: Domain, socket: i16| {
            let i = sensor.channels().iter().position(|c| c.domain == domain && c.socket == socket).unwrap();
            r[i].joules
        };
        assert_eq!(by_socket(Domain::Cpu, 0), 2.0);
        assert_eq!(by_socket(Domain::Cpu, 1), 2.0);
        assert_eq!(by_socket(Domain::Igpu, 0), 1.0);
    }

    #[test]
    fn describe_names_the_counters() {
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "0", MAX), &[
            ("intel-rapl:0:0", "uncore", "0", MAX),
            ("intel-rapl:0:1", "dram", "0", MAX),
        ])]);
        let sensor = RaplSensor::open(tree.path()).unwrap();
        assert_eq!(
            sensor.describe(),
            ["socket 0 cpu (package-0 minus uncore)", "socket 0 dram (dram)", "socket 0 igpu (uncore)"]
        );
    }

    #[test]
    fn empty_hierarchy_is_no_domains() {
        let tree = fake_tree(&[]);
        assert!(matches!(RaplSensor::open(tree.path()), Err(OpenError::NoDomains)));
        let psys_only = fake_tree(&[(("intel-rapl:0", "psys", "1", "10"), &[])]);
        assert!(matches!(RaplSensor::open(psys_only.path()), Err(OpenError::NoDomains)));
    }

    #[cfg(unix)]
    #[test]
    fn root_only_counters_are_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        let tree = fake_tree(&[(("intel-rapl:0", "package-0", "1", "10"), &[])]);
        let f = tree.path().join("intel-rapl:0/energy_uj");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o000)).unwrap();
        if File::open(&f).is_ok() {
            return; // running as root: permissions are not enforced
        }
        assert!(matches!(RaplSensor::open(tree.path()), Err(OpenError::PermissionDenied)));
    }
}

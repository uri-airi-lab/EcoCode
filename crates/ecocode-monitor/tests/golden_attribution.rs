//! Golden tests: expected values were captured from the original
//! implementation on these exact inputs and must not drift. Each test recombines the
//! counter correction the sensor applies with the attribution the sampler
//! applies; any change here changes the numbers users see on Linux.

use ecocode_monitor::attribution::{Usage, gpu_process_share, share_energy};
use ecocode_sensors::common::counter::{microjoules_delta_j, millijoules_reset_delta_j};
use ecocode_sensors::domain::latest_timestamp;
use ecocode_sensors::{Domain, GpuUtilSample};

const CPU_MAX: f64 = 262_143_328_850.0;
const DRAM_MAX: f64 = 65_712_999_613.0;

/// (socket, domain, previous µJ, current µJ, counter max µJ)
fn rapl_fixture() -> Vec<(i16, &'static str, f64, f64, f64)> {
    vec![
        (0, "cpu", 1_000_000.0, 4_123_456.0, CPU_MAX),
        (0, "dram", 5_000_000.0, 5_987_654.0, DRAM_MAX),
        (0, "igpu", 42.0, 142.0, CPU_MAX),
        (1, "cpu", 262_143_000_000.0, 1_234_567.0, CPU_MAX), // wraps
        (0, "psys", 9_000_000.0, 12_500_000.0, CPU_MAX),     // unknown domain → cpu share
        (1, "dram", 7_000_000.0, 7_000_000.0, DRAM_MAX),     // no change
    ]
}

fn domain(name: &str) -> Domain {
    match name {
        "cpu" => Domain::Cpu,
        "dram" => Domain::Dram,
        "igpu" => Domain::Igpu,
        other => Domain::Other(other.into()),
    }
}

fn rapl(cpu: f64, mem: f64, igpu: f64) -> Vec<(i16, String, f64)> {
    let usage = Usage { cpu_pct: cpu, mem_pct: mem, igpu_pct: igpu };
    rapl_fixture()
        .into_iter()
        .map(|(socket, name, prev, curr, max)| {
            let joules = microjoules_delta_j(prev, curr, max);
            (socket, name.to_string(), share_energy(&domain(name), joules, &usage))
        })
        .collect()
}

fn expect(v: &[(i16, &str, f64)]) -> Vec<(i16, String, f64)> {
    v.iter().map(|(s, d, x)| (*s, d.to_string(), *x)).collect()
}

#[test]
fn rapl_partial_share() {
    assert_eq!(
        rapl(37.5, 12.25, 37.5),
        expect(&[
            (0, "cpu", 1.171296),
            (0, "dram", 0.120987615),
            (0, "igpu", 3.7500000000000003e-5),
            (1, "cpu", 0.586281375),
            (0, "psys", 1.3125),
            (1, "dram", 0.0),
        ])
    );
}

#[test]
fn rapl_zero_share() {
    assert_eq!(
        rapl(0.0, 0.0, 0.0),
        expect(&[
            (0, "cpu", 0.0),
            (0, "dram", 0.0),
            (0, "igpu", 0.0),
            (1, "cpu", 0.0),
            (0, "psys", 0.0),
            (1, "dram", 0.0),
        ])
    );
}

#[test]
fn rapl_full_share() {
    assert_eq!(
        rapl(100.0, 100.0, 100.0),
        expect(&[
            (0, "cpu", 3.123456),
            (0, "dram", 0.987654),
            (0, "igpu", 0.0001),
            (1, "cpu", 1.563417),
            (0, "psys", 3.5),
            (1, "dram", 0.0),
        ])
    );
}

#[test]
fn rapl_real_world_share() {
    assert_eq!(
        rapl(8.609794616699219, 7.99, 8.609794616699219),
        expect(&[
            (0, "cpu", 0.2689231465429688),
            (0, "dram", 0.0789135546),
            (0, "igpu", 8.609794616699219e-6),
            (1, "cpu", 0.13460699270256044),
            (0, "psys", 0.3013428115844727),
            (1, "dram", 0.0),
        ])
    );
}

fn s(pid: u32, timestamp: u64, sm: u32, mem: u32, enc: u32, dec: u32) -> GpuUtilSample {
    GpuUtilSample { pid, timestamp, sm_util: sm, mem_util: mem, enc_util: enc, dec_util: dec }
}

fn gpu_stats() -> Vec<GpuUtilSample> {
    vec![
        s(100, 10, 80, 30, 0, 0),
        s(101, 12, 10, 5, 0, 0),
        s(200, 15, 40, 20, 7, 3),
        s(100, 20, 90, 35, 0, 0),
    ]
}

/// The original `get_gpu_energy_by_pid` result: (energy J, util %, next timestamp).
fn gpu(samples: &[GpuUtilSample], pids: &[u32], e1: f64, e2: f64, ts: u64, max: f64) -> (f64, f64, u64) {
    let (share, util) = gpu_process_share(samples, pids);
    (millijoules_reset_delta_j(e1, e2, max) * share, util, latest_timestamp(samples, ts))
}

#[test]
fn gpu_normal_interval() {
    assert_eq!(
        gpu(&gpu_stats(), &[100, 101], 1_000_000.0, 1_250_000.0, 5, 1_000_000.0),
        (202.88906962763892, 60.0, 20)
    );
}

#[test]
fn gpu_counter_reset() {
    assert_eq!(
        gpu(&gpu_stats(), &[100, 101], 9_000_000.0, 40_000.0, 5, 9_100_000.0),
        (113.6178789914778, 60.0, 20)
    );
}

#[test]
fn gpu_only_other_processes() {
    assert_eq!(gpu(&gpu_stats(), &[999], 0.0, 500.0, 5, 0.0), (0.0, 0.0, 20));
}

#[test]
fn gpu_no_samples_keeps_timestamp() {
    assert_eq!(gpu(&[], &[100], 0.0, 500.0, 7, 0.0), (0.0, 0.0, 7));
}

#[test]
fn gpu_zero_activity() {
    assert_eq!(gpu(&[s(100, 30, 0, 0, 0, 0)], &[100], 0.0, 500.0, 7, 0.0), (0.0, 0.0, 30));
}

#[test]
fn gpu_util_rounds_ties_to_even() {
    assert_eq!(
        gpu(&[s(100, 30, 25, 0, 0, 0), s(100, 31, 26, 0, 0, 0)], &[100], 0.0, 3333.0, 7, 0.0),
        (3.333, 26.0, 31)
    );
}

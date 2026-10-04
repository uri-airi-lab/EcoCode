//! Per-process CPU time and memory from `/proc/<pid>/{stat,smaps_rollup,status}`.

use std::collections::HashMap;

use sysinfo::System;

use crate::ProcessInspector;

/// CPU time in clock ticks (`/proc/<pid>/stat` utime + stime) and memory as
/// PSS (proportional set size, so shared pages are split between processes).
/// Only thread group leaders are read: `/proc/<tid>/stat` of a thread reports
/// the whole group's time and `smaps_rollup` its whole address space.
pub struct ProcFsInspector {
    clk_tck: f64,
}

impl ProcFsInspector {
    pub fn new() -> Self {
        ProcFsInspector { clk_tck: clock_ticks_per_second() }
    }
}

impl Default for ProcFsInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessInspector for ProcFsInspector {
    fn name(&self) -> &'static str {
        "procfs"
    }

    fn cpu_time_units_per_sec(&self) -> f64 {
        self.clk_tck
    }

    fn cpu_times(&mut self, _sys: &System, tree: &[u32]) -> HashMap<u32, u64> {
        let leaders: Vec<u32> = tree.iter().copied().filter(|&p| is_thread_leader(p)).collect();
        read_all_cpu_ticks(&leaders)
    }

    fn memory_bytes(&mut self, _sys: &System, tree: &[u32]) -> u64 {
        let mut pss_total_bytes = 0_u64;
        for &tpid in tree {
            if is_thread_leader(tpid)
                && let Some(pss) = read_pss_bytes(tpid)
            {
                pss_total_bytes += pss;
            }
        }
        pss_total_bytes
    }

    fn memory_kind(&self) -> &'static str {
        "PSS"
    }
}

/// Kernel clock ticks per second (`_SC_CLK_TCK`), 100 if unknown.
fn clock_ticks_per_second() -> f64 {
    #[cfg(unix)]
    {
        let tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if tck > 0 {
            return tck as f64;
        }
    }
    100.0
}

/// Reads cumulative CPU ticks (utime + stime) for a single PID from /proc/[pid]/stat.
/// Returns None if the process no longer exists or the file can't be parsed.
pub fn read_cpu_ticks(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    parse_cpu_ticks(&stat)
}

/// Parses utime + stime (fields 14 and 15) from the contents of /proc/[pid]/stat.
/// Fields are counted after the last `)` because the command name may contain
/// spaces and parentheses.
pub fn parse_cpu_ticks(stat: &str) -> Option<u64> {
    let after_comm = stat.get(stat.rfind(')')? + 2..)?;
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    if fields.len() < 13 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    Some(utime + stime)
}

/// Reads cumulative CPU ticks for all PIDs, returning a map of pid → total ticks.
pub fn read_all_cpu_ticks(pids: &[u32]) -> HashMap<u32, u64> {
    let mut ticks = HashMap::with_capacity(pids.len());
    for &pid in pids {
        if let Some(t) = read_cpu_ticks(pid) {
            ticks.insert(pid, t);
        }
    }
    ticks
}

/// Reads PSS (Proportional Set Size) from /proc/[pid]/smaps_rollup.
/// PSS correctly attributes shared pages proportionally across sharing processes.
pub fn read_pss_bytes(pid: u32) -> Option<u64> {
    let content = std::fs::read_to_string(format!("/proc/{}/smaps_rollup", pid)).ok()?;
    parse_pss_bytes(&content)
}

/// Parses the `Pss:` line (in kB) of /proc/[pid]/smaps_rollup into bytes.
pub fn parse_pss_bytes(smaps_rollup: &str) -> Option<u64> {
    for line in smaps_rollup.lines() {
        if line.starts_with("Pss:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Returns true if this PID is a thread group leader (i.e. a real process, not a thread).
/// Threads share the same address space as their leader, so PSS must only be read
/// from leaders to avoid counting the same memory N times.
pub fn is_thread_leader(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{}/status", pid))
        .ok()
        .and_then(|s| parse_tgid(&s))
        .map(|tgid| tgid == pid)
        .unwrap_or(false)
}

/// Parses the `Tgid:` line of /proc/[pid]/status.
pub fn parse_tgid(status: &str) -> Option<u32> {
    status
        .lines()
        .find(|l| l.starts_with("Tgid:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real /proc/<pid>/stat line; utime=1523, stime=310.
    const STAT: &str = "155504 (python3) R 155490 155504 155490 34816 155504 4194304 \
        512345 0 12 0 1523 310 0 0 20 0 9 0 8812345 4123451392 245123 \
        18446744073709551615 1 1 0 0 0 0 0 16781312 17642 0 0 0 17 3 0 0 0 0 0";

    #[test]
    fn parses_utime_plus_stime() {
        assert_eq!(parse_cpu_ticks(STAT), Some(1833));
    }

    #[test]
    fn command_name_with_spaces_and_parens_is_skipped() {
        let stat = STAT.replace("(python3)", "(my (weird) proc)");
        assert_eq!(parse_cpu_ticks(&stat), Some(1833));
    }

    #[test]
    fn truncated_stat_is_rejected() {
        assert_eq!(parse_cpu_ticks("1 (x) R 0 0"), None);
    }

    #[test]
    fn parses_pss_in_bytes() {
        let smaps = "55a4c8000000-7ffd1f9fe000 ---p 00000000 00:00 0  [rollup]\n\
                     Rss:              245120 kB\n\
                     Pss:              201234 kB\n\
                     Pss_Anon:         180000 kB\n";
        assert_eq!(parse_pss_bytes(smaps), Some(201_234 * 1024));
    }

    #[test]
    fn missing_pss_is_none() {
        assert_eq!(parse_pss_bytes("Rss: 1 kB\n"), None);
    }

    #[test]
    fn parses_tgid() {
        let status = "Name:\tpython3\nUmask:\t0022\nState:\tR (running)\nTgid:\t155504\nNgid:\t0\nPid:\t155507\n";
        assert_eq!(parse_tgid(status), Some(155_504));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_own_process_on_linux() {
        let me = std::process::id();
        assert!(read_cpu_ticks(me).is_some());
        assert!(is_thread_leader(me));
    }
}

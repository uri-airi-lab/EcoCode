//! Portable per-process statistics via `sysinfo` (any OS).
//!
//! Used where no OS-specific inspector exists yet. CPU time comes from the OS
//! process accounting; memory is resident set size (RSS), which counts shared
//! pages in full for every process that maps them.

use std::collections::HashMap;

use sysinfo::{Pid, System};

use crate::ProcessInspector;

pub struct SysinfoInspector;

impl ProcessInspector for SysinfoInspector {
    fn name(&self) -> &'static str {
        "sysinfo"
    }

    fn cpu_time_units_per_sec(&self) -> f64 {
        1000.0 // accumulated_cpu_time() is in CPU-milliseconds
    }

    fn cpu_times(&mut self, sys: &System, tree: &[u32]) -> HashMap<u32, u64> {
        tree.iter()
            .filter_map(|&pid| {
                let p = sys.process(Pid::from_u32(pid))?;
                // Threads listed as processes share their leader's CPU time.
                p.thread_kind().is_none().then(|| (pid, p.accumulated_cpu_time()))
            })
            .collect()
    }

    fn memory_bytes(&mut self, sys: &System, tree: &[u32]) -> u64 {
        tree.iter()
            .filter_map(|&pid| sys.process(Pid::from_u32(pid)))
            .filter(|p| p.thread_kind().is_none())
            .map(|p| p.memory())
            .sum()
    }

    fn memory_kind(&self) -> &'static str {
        "RSS"
    }

    fn needs_process_stats(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::ProcessesToUpdate;

    #[test]
    fn reports_own_cpu_time_and_memory() {
        // Burn a little CPU so accumulated time is non-zero.
        let start = std::time::Instant::now();
        let mut x = 0u64;
        while start.elapsed().as_millis() < 50 {
            x = x.wrapping_add(std::hint::black_box(1));
        }
        assert!(x > 0);
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let me = std::process::id();
        let mut inspector = SysinfoInspector;
        assert!(inspector.cpu_times(&sys, &[me]).contains_key(&me));
        assert!(inspector.memory_bytes(&sys, &[me]) > 0);
    }

    #[test]
    fn unknown_pid_is_skipped() {
        let sys = System::new();
        let mut inspector = SysinfoInspector;
        assert!(inspector.cpu_times(&sys, &[u32::MAX - 1]).is_empty());
        assert_eq!(inspector.memory_bytes(&sys, &[u32::MAX - 1]), 0);
    }
}

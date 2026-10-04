//! Per-process CPU time, memory and CPU energy on macOS.

use std::collections::HashMap;

use sysinfo::System;

use super::rusage::{Timebase, rusage};
use crate::ProcessInspector;

/// CPU time in nanoseconds, memory as physical footprint (what Activity
/// Monitor shows as "Memory"), and on Apple Silicon per-process CPU energy.
pub struct MacInspector {
    timebase: Timebase,
    energy: bool,
}

impl MacInspector {
    /// `with_energy`: report per-process CPU energy where the hardware supports
    /// it (off for fake/no-sensor runs, which must not measure real energy).
    pub fn new(with_energy: bool) -> Self {
        MacInspector {
            timebase: Timebase::current(),
            energy: with_energy && super::is_apple_silicon(),
        }
    }
}

impl ProcessInspector for MacInspector {
    fn name(&self) -> &'static str {
        "rusage"
    }

    fn cpu_time_units_per_sec(&self) -> f64 {
        1e9
    }

    fn cpu_times(&mut self, _sys: &System, tree: &[u32]) -> HashMap<u32, u64> {
        tree.iter()
            .filter_map(|&pid| {
                let r = rusage(pid)?;
                Some((pid, self.timebase.to_ns(r.ri_user_time + r.ri_system_time)))
            })
            .collect()
    }

    fn memory_bytes(&mut self, _sys: &System, tree: &[u32]) -> u64 {
        tree.iter().filter_map(|&pid| rusage(pid)).map(|r| r.ri_phys_footprint).sum()
    }

    fn memory_kind(&self) -> &'static str {
        "physical footprint"
    }

    fn cpu_energy_nj(&mut self, tree: &[u32]) -> Option<HashMap<u32, u64>> {
        if !self.energy {
            return None;
        }
        Some(tree.iter().filter_map(|&pid| Some((pid, rusage(pid)?.ri_energy_nj))).collect())
    }

    fn has_cpu_energy(&self) -> bool {
        self.energy
    }
}

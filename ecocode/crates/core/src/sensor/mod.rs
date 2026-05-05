//! Sensor module for monitoring system energy consumption.
//!
//! This module provides functions to read CPU and GPU energy data from the system.
//! It includes:
//! - `rapl`: CPU energy monitoring using Intel RAPL interface
//! - `gpu`: GPU energy monitoring using NVIDIA Management Library (NVML)

pub mod rapl;

#[cfg(feature = "gpu")]
pub mod gpu;

pub const RAPL_PATH: &str = "/sys/class/powercap/intel-rapl/";

use sysinfo::System;

/// Finds all recursive children PIDs of a given root PID.
pub fn get_process_tree_pids(root_pid: u32) -> Vec<u32> {
    let mut sys = System::new_all();
    sys.refresh_all();
    get_process_tree_pids_from_sys(&sys, root_pid)
}

/// Finds all recursive children PIDs of a given root PID using an existing System object.
pub fn get_process_tree_pids_from_sys(sys: &sysinfo::System, root_pid: u32) -> Vec<u32> {
    let mut pids = vec![root_pid];
    let mut to_visit = vec![root_pid];

    while let Some(parent_pid) = to_visit.pop() {
        for (pid, process) in sys.processes() {
            if let Some(ppid) = process.parent() {
                if ppid.as_u32() == parent_pid {
                    let child_pid = pid.as_u32();
                    if !pids.contains(&child_pid) {
                        pids.push(child_pid);
                        to_visit.push(child_pid);
                    }
                }
            }
        }
    }
    pids
}

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

use std::collections::{HashMap, HashSet};
use sysinfo::System;


/// Finds all recursive children PIDs of a given root PID using an existing System object.
///
/// Builds a parent→children HashMap once (O(n)), then walks the tree via BFS (O(k)
/// where k is the subtree size). This avoids the previous O(n·k) scan that iterated
/// all processes for every node in the queue.
pub fn get_process_tree_pids_from_sys(sys: &sysinfo::System, root_pid: u32) -> Vec<u32> {
    // Build parent → children index in a single pass over all processes.
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, process) in sys.processes() {
        if let Some(ppid) = process.parent() {
            children_map
                .entry(ppid.as_u32())
                .or_default()
                .push(pid.as_u32());
        }
    }

    // BFS from root_pid using the pre-built index.
    let mut pids = vec![root_pid];
    let mut visited = HashSet::new();
    visited.insert(root_pid);
    let mut to_visit = vec![root_pid];

    while let Some(parent_pid) = to_visit.pop() {
        if let Some(kids) = children_map.get(&parent_pid) {
            for &child_pid in kids {
                if visited.insert(child_pid) {
                    pids.push(child_pid);
                    to_visit.push(child_pid);
                }
            }
        }
    }
    pids
}


/// Finds all recursive children PIDs of a given root PID.
pub fn get_process_tree_pids(root_pid: u32) -> Vec<u32> {
    let mut sys = System::new_all();
    sys.refresh_all();
    get_process_tree_pids_from_sys(&sys, root_pid)
}
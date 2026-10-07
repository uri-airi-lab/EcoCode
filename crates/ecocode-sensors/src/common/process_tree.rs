//! Process-tree discovery: a root PID plus all of its descendants.
//!
//! Uses `sysinfo`, which works on Linux, macOS and Windows. The tree is used to
//! decide which PIDs energy is attributed to (data-loader workers,
//! torch.distributed ranks, etc. are children of the monitored process).

use std::collections::{HashMap, HashSet};
use sysinfo::System;

/// Finds all recursive children PIDs of a given root PID using an existing System object.
///
/// Builds a parent→children HashMap once (O(n)), then walks the tree via BFS (O(k)
/// where k is the subtree size). This avoids the previous O(n·k) scan that iterated
/// all processes for every node in the queue.
pub fn get_process_tree_pids_from_sys(sys: &System, root_pid: u32) -> Vec<u32> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::{ProcessesToUpdate, RefreshKind};

    #[test]
    fn tree_of_current_process_starts_with_root() {
        let mut sys = System::new_with_specifics(RefreshKind::nothing());
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let me = std::process::id();
        let tree = get_process_tree_pids_from_sys(&sys, me);
        assert_eq!(tree[0], me);
    }

    #[test]
    fn tree_includes_spawned_child() {
        let mut child = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", "ping -n 3 127.0.0.1 >NUL"])
                .spawn()
                .unwrap()
        } else {
            std::process::Command::new("sleep").arg("2").spawn().unwrap()
        };
        let mut sys = System::new_with_specifics(RefreshKind::nothing());
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let tree = get_process_tree_pids_from_sys(&sys, std::process::id());
        let child_pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        assert!(tree.contains(&child_pid), "child {child_pid} not in {tree:?}");
    }
}

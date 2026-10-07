//! Per-process GPU activity on macOS, from the IORegistry.
//!
//! Every GPU client (`AGXDeviceUserClient` on Apple Silicon) is a child of the
//! `IOAccelerator` service and records its creator (`"pid 400, WindowServer"`)
//! and, per command queue, `accumulatedGPUTime` and `lastSubmittedTime`. This
//! is what Activity Monitor's GPU column uses, and it is readable without root.
//!
//! These counters show *when* a process uses the GPU, not how much energy it
//! causes: GPU time is shared out in turns, and high-priority UI clients
//! (WindowServer, apps drawing on screen) accrue time while waiting behind a
//! heavy job. EcoCode therefore uses them only to tell whether the monitored
//! process is using the GPU at all (see `ioreport`).

use std::collections::HashMap;

use core_foundation_sys::base::kCFAllocatorDefault;
use core_foundation_sys::string::CFStringRef;

use super::cf::{self, Owned};
use super::iokit::{self, Object};

/// GPU counters of one process, summed over all of its GPU clients.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClientActivity {
    /// Cumulative GPU time (ns); advances when submitted work completes.
    pub gpu_time_ns: u64,
    /// Time of the most recent submission (mach ticks); advances when new work is queued.
    pub last_submitted: u64,
}

/// GPU activity counters per PID.
pub fn activity_by_pid() -> HashMap<u32, ClientActivity> {
    let mut by_pid: HashMap<u32, ClientActivity> = HashMap::new();
    for_each_accelerator(|accelerator| unsafe {
        let mut children = 0;
        if iokit::IORegistryEntryGetChildIterator(accelerator.0, c"IOService".as_ptr(), &mut children)
            != iokit::KERN_SUCCESS
        {
            return;
        }
        let children = Object(children);
        for client in iokit::iterate(&children) {
            let Some(pid) = property(&client, "IOUserClientCreator")
                .and_then(|p| cf::string(p.as_ptr()))
                .and_then(|s| parse_creator_pid(&s))
            else {
                continue;
            };
            let Some(usage) = property(&client, "AppUsage") else { continue };
            let entry = by_pid.entry(pid).or_default();
            for queue in cf::array(usage.as_ptr()) {
                let time = cf::number(cf::dict_get(queue, "accumulatedGPUTime")).unwrap_or(0).max(0) as u64;
                let submitted = cf::number(cf::dict_get(queue, "lastSubmittedTime")).unwrap_or(0).max(0) as u64;
                entry.gpu_time_ns += time;
                entry.last_submitted = entry.last_submitted.max(submitted);
            }
        }
    });
    by_pid
}

/// How busy the whole GPU is right now (0–100), from the accelerator's
/// `PerformanceStatistics`. An instantaneous reading, not an interval average.
pub fn device_utilization() -> Option<f64> {
    let mut best: Option<f64> = None;
    for_each_accelerator(|accelerator| unsafe {
        let Some(stats) = property(accelerator, "PerformanceStatistics") else { return };
        if let Some(pct) = cf::number(cf::dict_get(stats.as_ptr(), "Device Utilization %")) {
            best = Some(best.unwrap_or(0.0).max(pct as f64));
        }
    });
    best.map(|p| p.clamp(0.0, 100.0))
}

/// True if any process in `tree` used the GPU between the two snapshots:
/// its GPU time advanced (work completed) or it submitted new work. A process
/// missing from `prev` started using the GPU since the previous snapshot.
pub fn tree_active(prev: &HashMap<u32, ClientActivity>, curr: &HashMap<u32, ClientActivity>, tree: &[u32]) -> bool {
    tree.iter().any(|pid| match (prev.get(pid), curr.get(pid)) {
        (Some(p), Some(c)) => c.gpu_time_ns > p.gpu_time_ns || c.last_submitted > p.last_submitted,
        (None, Some(c)) => c.gpu_time_ns > 0 || c.last_submitted > 0,
        _ => false,
    })
}

fn for_each_accelerator(mut f: impl FnMut(&Object)) {
    unsafe {
        let matching = iokit::IOServiceMatching(c"IOAccelerator".as_ptr());
        let mut services = 0;
        if iokit::IOServiceGetMatchingServices(iokit::MAIN_PORT_DEFAULT, matching, &mut services) != iokit::KERN_SUCCESS {
            return;
        }
        let services = Object(services);
        for accelerator in iokit::iterate(&services) {
            f(&accelerator);
        }
    }
}

fn property(entry: &Object, key: &str) -> Option<Owned> {
    let key = cf::cfstr(key);
    unsafe {
        Owned::new(iokit::IORegistryEntryCreateCFProperty(
            entry.0,
            key.as_ptr() as CFStringRef,
            kCFAllocatorDefault,
            0,
        ))
    }
}

/// `"pid 400, WindowServer"` → 400.
pub fn parse_creator_pid(creator: &str) -> Option<u32> {
    creator.strip_prefix("pid ")?.split(',').next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(gpu_time_ns: u64, last_submitted: u64) -> ClientActivity {
        ClientActivity { gpu_time_ns, last_submitted }
    }

    #[test]
    fn parses_creator() {
        assert_eq!(parse_creator_pid("pid 400, WindowServer"), Some(400));
        assert_eq!(parse_creator_pid("pid 7"), Some(7));
        assert_eq!(parse_creator_pid("kernel"), None);
    }

    #[test]
    fn activity_is_completed_or_submitted_work_of_the_tree() {
        let prev = HashMap::from([(1, a(100, 5)), (2, a(50, 9))]);
        // Completed work.
        assert!(tree_active(&prev, &HashMap::from([(1, a(160, 5))]), &[1]));
        // New submission, nothing completed yet (a long GPU job).
        assert!(tree_active(&prev, &HashMap::from([(1, a(100, 6))]), &[1]));
        // A client that appeared since the previous snapshot.
        assert!(tree_active(&prev, &HashMap::from([(3, a(0, 11))]), &[3]));
        // Unchanged counters, or activity outside the tree, is not ours.
        assert!(!tree_active(&prev, &HashMap::from([(1, a(100, 5)), (2, a(900, 99))]), &[1]));
        assert!(!tree_active(&prev, &HashMap::new(), &[1]));
    }

    #[test]
    #[ignore = "needs a Mac with a GPU (run with --ignored)"]
    fn window_server_uses_the_gpu() {
        let usage = activity_by_pid();
        assert!(!usage.is_empty(), "no GPU clients found");
        assert!(usage.values().any(|u| u.gpu_time_ns > 0));
        assert!(device_utilization().is_some());
    }
}

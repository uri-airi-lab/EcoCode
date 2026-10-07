//! Apple Silicon energy via IOReport's "Energy Model" group (no root needed).
//!
//! The group exposes cumulative energy per hardware block. We read the totals
//! only, never the per-core / per-cluster channels they are made of:
//! - `CPU Energy` (or `DIE_<n>_CPU Energy` on multi-die chips) → CPU
//! - `GPU Energy` → GPU
//! - `ANE*` → Neural Engine
//! - `DRAM*` → memory
//!
//! Each channel carries its own unit (mJ, µJ or nJ).
//!
//! # GPU attribution
//! macOS has no per-process GPU energy a normal user can read, and per-process
//! GPU time is shared out in turns that favour UI clients (see
//! [`super::gpu_usage`]), so splitting energy by it badly under-credits compute
//! jobs. Instead, the monitored process tree is credited with the **whole GPU's
//! energy while it is using the GPU** (it completed or submitted GPU work in
//! the last [`GPU_ACTIVE_WINDOW`]), and with none otherwise. The error is what
//! other apps draw on the GPU at the same time (an idle desktop: ~0.1 W).

use std::collections::HashMap;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use core_foundation_sys::base::CFTypeRef;
use core_foundation_sys::dictionary::{CFDictionaryCreateMutableCopy, CFDictionaryGetCount, CFDictionaryRef, CFMutableDictionaryRef};
use core_foundation_sys::string::CFStringRef;

use super::cf::{self, Owned};
use super::gpu_usage;
use crate::domain::{Channel, Domain, Quality, Reading, TreeShare};
use crate::{EnergySensor, SensorError};

#[link(name = "IOReport", kind = "dylib")]
unsafe extern "C" {
    fn IOReportCopyChannelsInGroup(group: CFStringRef, subgroup: CFStringRef, a: u64, b: u64, c: u64) -> CFDictionaryRef;
    fn IOReportCreateSubscription(
        a: *const c_void,
        desired: CFMutableDictionaryRef,
        subscribed: *mut CFMutableDictionaryRef,
        channel_id: u64,
        b: CFTypeRef,
    ) -> *const c_void;
    fn IOReportCreateSamples(subscription: *const c_void, channels: CFMutableDictionaryRef, a: CFTypeRef) -> CFDictionaryRef;
    fn IOReportCreateSamplesDelta(prev: CFDictionaryRef, curr: CFDictionaryRef, a: CFTypeRef) -> CFDictionaryRef;
    fn IOReportChannelGetChannelName(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportChannelGetUnitLabel(item: CFDictionaryRef) -> CFStringRef;
    fn IOReportSimpleGetIntegerValue(item: CFDictionaryRef, a: i32) -> i64;
}

/// Which energy domain an Energy Model channel is a total for, if any.
pub fn classify(channel: &str) -> Option<Domain> {
    // Multi-die chips prefix per-die channels: "DIE_0_CPU Energy".
    let base = match channel.strip_prefix("DIE_") {
        Some(rest) => rest.split_once('_').map(|(_, b)| b).unwrap_or(rest),
        None => channel,
    };
    if base == "CPU Energy" {
        Some(Domain::Cpu)
    } else if base == "GPU Energy" {
        Some(Domain::Gpu(0))
    } else if base.starts_with("ANE") {
        Some(Domain::NeuralEngine)
    } else if base.starts_with("DRAM") {
        Some(Domain::Dram)
    } else {
        None
    }
}

/// Converts an IOReport energy value to joules using its unit label.
pub fn to_joules(value: i64, unit: &str) -> Option<f64> {
    // Dividing by an exactly representable power of ten rounds correctly;
    // multiplying by 1e-6 (which is not exact) can be off by one ulp.
    let divisor = match unit.trim() {
        "J" => 1.0,
        "mJ" => 1e3,
        "uJ" | "µJ" => 1e6,
        "nJ" => 1e9,
        _ => return None,
    };
    Some(value as f64 / divisor)
}

/// How long after its last GPU activity the process tree is still credited
/// with GPU energy (long GPU jobs only report activity when they are queued
/// and when they complete).
pub const GPU_ACTIVE_WINDOW: Duration = Duration::from_secs(5);

/// The order channels are reported in.
const DOMAIN_ORDER: [Domain; 4] = [Domain::Cpu, Domain::Gpu(0), Domain::NeuralEngine, Domain::Dram];

pub struct IoReportSensor {
    subscription: Owned,
    desired: Owned,
    _subscribed: Option<Owned>,
    prev: Owned,
    channels: Vec<Channel>,
    gpu_prev: HashMap<u32, gpu_usage::ClientActivity>,
    gpu_last_active: Option<Instant>,
}

impl IoReportSensor {
    /// Subscribes to the Energy Model group. `None` when it has none of the
    /// channels we use (Intel Macs, virtual machines).
    pub fn open(chip: &str) -> Option<Self> {
        unsafe {
            let group = cf::cfstr("Energy Model");
            let found = Owned::new(IOReportCopyChannelsInGroup(group.as_ptr() as CFStringRef, std::ptr::null(), 0, 0, 0) as CFTypeRef)?;
            let desired = Owned::new(CFDictionaryCreateMutableCopy(
                std::ptr::null(),
                CFDictionaryGetCount(found.as_ptr() as CFDictionaryRef),
                found.as_ptr() as CFDictionaryRef,
            ) as CFTypeRef)?;
            let mut subscribed: CFMutableDictionaryRef = std::ptr::null_mut();
            let subscription = Owned::new(IOReportCreateSubscription(
                std::ptr::null(),
                desired.as_ptr() as CFMutableDictionaryRef,
                &mut subscribed,
                0,
                std::ptr::null(),
            ))?;
            let subscribed = Owned::new(subscribed as CFTypeRef);
            let first = Owned::new(IOReportCreateSamples(
                subscription.as_ptr(),
                desired.as_ptr() as CFMutableDictionaryRef,
                std::ptr::null(),
            ) as CFTypeRef)?;

            let present: Vec<Domain> = channel_items(&first).filter_map(|(name, _, _)| classify(&name)).collect();
            let channels: Vec<Channel> = DOMAIN_ORDER
                .iter()
                .filter(|d| present.contains(d))
                .map(|d| Channel {
                    domain: d.clone(),
                    socket: 0,
                    name: match d {
                        Domain::Cpu => format!("{chip} CPU"),
                        Domain::Gpu(_) => format!("{chip} GPU"),
                        Domain::NeuralEngine => format!("{chip} Neural Engine"),
                        _ => "DRAM".to_string(),
                    },
                    quality: Quality::Measured,
                })
                .collect();
            if channels.is_empty() {
                return None;
            }
            Some(IoReportSensor {
                subscription,
                desired,
                _subscribed: subscribed,
                prev: first,
                channels,
                gpu_prev: gpu_usage::activity_by_pid(),
                gpu_last_active: None,
            })
        }
    }

    pub fn channels_ref(&self) -> &[Channel] {
        &self.channels
    }

    /// Whole GPU while the tree is using it, nothing otherwise (see module docs).
    fn gpu_tree_share(&mut self, tree: &[u32]) -> TreeShare {
        let now = Instant::now();
        let curr = gpu_usage::activity_by_pid();
        if gpu_usage::tree_active(&self.gpu_prev, &curr, tree) {
            self.gpu_last_active = Some(now);
        }
        self.gpu_prev = curr;
        let active = self.gpu_last_active.is_some_and(|t| now.duration_since(t) <= GPU_ACTIVE_WINDOW);
        if active {
            TreeShare { share: 1.0, util_pct: gpu_usage::device_utilization().unwrap_or(0.0) }
        } else {
            TreeShare { share: 0.0, util_pct: 0.0 }
        }
    }
}

/// (name, unit, value) of every channel in a samples dictionary.
fn channel_items(samples: &Owned) -> impl Iterator<Item = (String, String, i64)> {
    let items = unsafe { cf::array(cf::dict_get(samples.as_ptr(), "IOReportChannels")) };
    items.into_iter().filter_map(|item| unsafe {
        let item = item as CFDictionaryRef;
        let name = cf::string(IOReportChannelGetChannelName(item) as CFTypeRef)?;
        let unit = cf::string(IOReportChannelGetUnitLabel(item) as CFTypeRef).unwrap_or_default();
        Some((name, unit, IOReportSimpleGetIntegerValue(item, 0)))
    })
}

// The subscription and samples are only used from the sampler's thread.
unsafe impl Send for IoReportSensor {}

impl EnergySensor for IoReportSensor {
    fn name(&self) -> &'static str {
        "ioreport"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let (curr, delta) = unsafe {
            let curr = Owned::new(IOReportCreateSamples(
                self.subscription.as_ptr(),
                self.desired.as_ptr() as CFMutableDictionaryRef,
                std::ptr::null(),
            ) as CFTypeRef)
            .ok_or_else(|| SensorError::new("ioreport", "IOReportCreateSamples failed"))?;
            let delta = Owned::new(IOReportCreateSamplesDelta(
                self.prev.as_ptr() as CFDictionaryRef,
                curr.as_ptr() as CFDictionaryRef,
                std::ptr::null(),
            ) as CFTypeRef)
            .ok_or_else(|| SensorError::new("ioreport", "IOReportCreateSamplesDelta failed"))?;
            (curr, delta)
        };
        self.prev = curr;

        let mut joules: HashMap<Domain, f64> = HashMap::new();
        for (name, unit, value) in channel_items(&delta) {
            if let (Some(domain), Some(j)) = (classify(&name), to_joules(value, &unit)) {
                *joules.entry(domain).or_insert(0.0) += j;
            }
        }

        let gpu_share = self
            .channels
            .iter()
            .any(|c| matches!(c.domain, Domain::Gpu(_)))
            .then(|| self.gpu_tree_share(tree));

        Ok(self
            .channels
            .iter()
            .map(|c| Reading {
                joules: joules.get(&c.domain).copied().unwrap_or(0.0),
                util_samples: None,
                tree_share: matches!(c.domain, Domain::Gpu(_)).then_some(gpu_share).flatten(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_totals_and_skips_parts() {
        assert_eq!(classify("CPU Energy"), Some(Domain::Cpu));
        assert_eq!(classify("DIE_1_CPU Energy"), Some(Domain::Cpu));
        assert_eq!(classify("GPU Energy"), Some(Domain::Gpu(0)));
        assert_eq!(classify("ANE0"), Some(Domain::NeuralEngine));
        assert_eq!(classify("DRAM1"), Some(Domain::Dram));
        // Parts of the totals, and blocks we don't report.
        for part in ["PACC0_CPU", "EACC_CPU0", "PCPUDTL0e", "GPU0", "GPU SRAM0", "DCS0", "AMCC0", "ISP0"] {
            assert_eq!(classify(part), None, "{part}");
        }
    }

    #[test]
    fn converts_units() {
        assert_eq!(to_joules(4095, "mJ"), Some(4.095));
        assert_eq!(to_joules(60_848_590, "nJ"), Some(0.06084859));
        assert_eq!(to_joules(5, "uJ "), Some(5e-6));
        assert_eq!(to_joules(5, "W"), None);
    }

    #[test]
    #[ignore = "needs Apple Silicon (run with --ignored)"]
    fn measures_cpu_energy_of_a_busy_loop() {
        let mut sensor = IoReportSensor::open("Apple").expect("Energy Model not available");
        let domains: Vec<_> = sensor.channels().iter().map(|c| c.domain.clone()).collect();
        assert!(domains.contains(&Domain::Cpu) && domains.contains(&Domain::Gpu(0)), "{domains:?}");
        let start = Instant::now();
        let mut x = 0u64;
        while start.elapsed().as_millis() < 500 {
            x = x.wrapping_add(std::hint::black_box(1));
        }
        std::hint::black_box(x);
        let r = sensor.read(&[std::process::id()]).unwrap();
        let cpu_watts = r[0].joules / start.elapsed().as_secs_f64();
        assert!((0.3..200.0).contains(&cpu_watts), "CPU power {cpu_watts} W");
        let gpu = r.iter().find(|r| r.tree_share.is_some()).expect("GPU reading has a tree share");
        assert!(gpu.joules >= 0.0);
    }
}

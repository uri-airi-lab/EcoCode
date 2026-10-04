//! Per-process accounting from `proc_pid_rusage` (`RUSAGE_INFO_V6`).
//!
//! V6 adds `ri_energy_nj`: the kernel's per-process CPU energy estimate,
//! which on Apple Silicon is derived from per-core energy counters.

use std::ffi::c_int;

/// `struct rusage_info_v6` from `<sys/resource.h>` (464 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RusageInfoV6 {
    pub ri_uuid: [u8; 16],
    pub ri_user_time: u64,
    pub ri_system_time: u64,
    pub ri_pkg_idle_wkups: u64,
    pub ri_interrupt_wkups: u64,
    pub ri_pageins: u64,
    pub ri_wired_size: u64,
    pub ri_resident_size: u64,
    pub ri_phys_footprint: u64,
    pub ri_proc_start_abstime: u64,
    pub ri_proc_exit_abstime: u64,
    pub ri_child_user_time: u64,
    pub ri_child_system_time: u64,
    pub ri_child_pkg_idle_wkups: u64,
    pub ri_child_interrupt_wkups: u64,
    pub ri_child_pageins: u64,
    pub ri_child_elapsed_abstime: u64,
    pub ri_diskio_bytesread: u64,
    pub ri_diskio_byteswritten: u64,
    pub ri_cpu_time_qos_default: u64,
    pub ri_cpu_time_qos_maintenance: u64,
    pub ri_cpu_time_qos_background: u64,
    pub ri_cpu_time_qos_utility: u64,
    pub ri_cpu_time_qos_legacy: u64,
    pub ri_cpu_time_qos_user_initiated: u64,
    pub ri_cpu_time_qos_user_interactive: u64,
    pub ri_billed_system_time: u64,
    pub ri_serviced_system_time: u64,
    pub ri_logical_writes: u64,
    pub ri_lifetime_max_phys_footprint: u64,
    pub ri_instructions: u64,
    pub ri_cycles: u64,
    pub ri_billed_energy: u64,
    pub ri_serviced_energy: u64,
    pub ri_interval_max_phys_footprint: u64,
    pub ri_runnable_time: u64,
    pub ri_flags: u64,
    pub ri_user_ptime: u64,
    pub ri_system_ptime: u64,
    pub ri_pinstructions: u64,
    pub ri_pcycles: u64,
    pub ri_energy_nj: u64,
    pub ri_penergy_nj: u64,
    pub ri_secure_time_in_system: u64,
    pub ri_secure_ptime_in_system: u64,
    pub ri_neural_footprint: u64,
    pub ri_lifetime_max_neural_footprint: u64,
    pub ri_interval_max_neural_footprint: u64,
    pub ri_reserved: [u64; 9],
}

const _: () = assert!(std::mem::size_of::<RusageInfoV6>() == 464);

const RUSAGE_INFO_V6: c_int = 6;

/// `struct mach_timebase_info` from `<mach/mach_time.h>`.
#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut RusageInfoV6) -> c_int;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> c_int;
}

/// Resource usage of `pid`, or `None` if it exited or belongs to another user.
pub fn rusage(pid: u32) -> Option<RusageInfoV6> {
    let mut info = std::mem::MaybeUninit::<RusageInfoV6>::zeroed();
    let rc = unsafe { proc_pid_rusage(pid as c_int, RUSAGE_INFO_V6, info.as_mut_ptr()) };
    (rc == 0).then(|| unsafe { info.assume_init() })
}

/// Converts mach absolute time units (used by `ri_*_time`) to nanoseconds.
#[derive(Clone, Copy, Debug)]
pub struct Timebase {
    numer: u32,
    denom: u32,
}

impl Timebase {
    /// The timebase `ri_*_time` values are in.
    ///
    /// Under Rosetta, `mach_timebase_info` reports the translated 1/1 timebase
    /// but `proc_pid_rusage` still returns native ticks, so use the Apple
    /// Silicon timer instead (24 MHz on every M-series chip: 125/3 ns per tick).
    pub fn current() -> Self {
        if super::is_translated() {
            return Timebase { numer: 125, denom: 3 };
        }
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        if unsafe { mach_timebase_info(&mut info) } != 0 || info.denom == 0 {
            return Timebase { numer: 1, denom: 1 };
        }
        Timebase { numer: info.numer, denom: info.denom }
    }

    pub fn to_ns(self, ticks: u64) -> u64 {
        (ticks as u128 * self.numer as u128 / self.denom as u128) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn timebase_converts_to_nanoseconds() {
        // Apple Silicon: 125/3 (24 MHz ticks); Intel: 1/1.
        let tb = Timebase { numer: 125, denom: 3 };
        assert_eq!(tb.to_ns(24_000_000), 1_000_000_000);
        assert_eq!(Timebase { numer: 1, denom: 1 }.to_ns(42), 42);
    }

    #[test]
    fn own_cpu_time_matches_wall_time_of_a_busy_loop() {
        let tb = Timebase::current();
        let me = std::process::id();
        let before = rusage(me).unwrap();
        let start = Instant::now();
        let mut x = 0u64;
        while start.elapsed() < Duration::from_millis(300) {
            x = x.wrapping_add(std::hint::black_box(1));
        }
        std::hint::black_box(x);
        let after = rusage(me).unwrap();
        let cpu_s = tb.to_ns(after.ri_user_time + after.ri_system_time - before.ri_user_time - before.ri_system_time)
            as f64
            / 1e9;
        assert!((0.2..0.6).contains(&cpu_s), "busy 0.3 s but CPU time {cpu_s} s");
        assert!(after.ri_phys_footprint > 0);
    }

    #[test]
    fn missing_process_is_none() {
        assert!(rusage(u32::MAX - 3).is_none());
    }
}

//! Per-process CPU time and memory on Windows.

/// A `FILETIME` (two 32-bit halves of a count of 100 ns units) as one number.
pub fn filetime_to_u64(low: u32, high: u32) -> u64 {
    ((high as u64) << 32) | low as u64
}

#[cfg(windows)]
pub use imp::WindowsInspector;

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;

    use sysinfo::System;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX2,
    };
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    use super::filetime_to_u64;
    use crate::ProcessInspector;

    /// CPU time from `GetProcessTimes` (100 ns units) and memory as the private
    /// working set (Task Manager's "Memory" column), or the working set on
    /// Windows versions before 10 1809.
    pub struct WindowsInspector;

    struct Process(HANDLE);

    impl Process {
        fn open(pid: u32) -> Option<Self> {
            let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            (!h.is_null()).then_some(Process(h))
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn cpu_time(pid: u32) -> Option<u64> {
        let p = Process::open(pid)?;
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let ok = unsafe { GetProcessTimes(p.0, &mut created, &mut exited, &mut kernel, &mut user) };
        (ok != 0).then(|| {
            filetime_to_u64(kernel.dwLowDateTime, kernel.dwHighDateTime)
                + filetime_to_u64(user.dwLowDateTime, user.dwHighDateTime)
        })
    }

    fn memory(pid: u32) -> Option<u64> {
        let p = Process::open(pid)?;
        unsafe {
            let mut ex2: PROCESS_MEMORY_COUNTERS_EX2 = std::mem::zeroed();
            let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX2>() as u32;
            ex2.cb = size;
            if K32GetProcessMemoryInfo(p.0, &mut ex2 as *mut _ as *mut PROCESS_MEMORY_COUNTERS, size) != 0 {
                // Some implementations (e.g. Wine) leave the private working set at 0.
                let private = ex2.PrivateWorkingSetSize as u64;
                return Some(if private > 0 { private } else { ex2.WorkingSetSize as u64 });
            }
            let mut basic: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            basic.cb = size;
            (K32GetProcessMemoryInfo(p.0, &mut basic, size) != 0).then_some(basic.WorkingSetSize as u64)
        }
    }

    impl ProcessInspector for WindowsInspector {
        fn name(&self) -> &'static str {
            "win32"
        }

        fn cpu_time_units_per_sec(&self) -> f64 {
            1e7
        }

        fn cpu_times(&mut self, _sys: &System, tree: &[u32]) -> HashMap<u32, u64> {
            tree.iter().filter_map(|&pid| Some((pid, cpu_time(pid)?))).collect()
        }

        fn memory_bytes(&mut self, _sys: &System, tree: &[u32]) -> u64 {
            tree.iter().filter_map(|&pid| memory(pid)).sum()
        }

        fn memory_kind(&self) -> &'static str {
            "private working set"
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::{Duration, Instant};

        #[test]
        fn own_cpu_time_matches_a_busy_loop() {
            let me = std::process::id();
            let before = cpu_time(me).unwrap();
            let start = Instant::now();
            let mut x = 0u64;
            while start.elapsed() < Duration::from_millis(300) {
                x = x.wrapping_add(std::hint::black_box(1));
            }
            std::hint::black_box(x);
            let secs = (cpu_time(me).unwrap() - before) as f64 / 1e7;
            assert!((0.2..0.6).contains(&secs), "busy 0.3 s but CPU time {secs} s");
            assert!(memory(me).unwrap() > 0);
        }

        #[test]
        fn missing_process_is_none() {
            assert!(cpu_time(u32::MAX - 3).is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_filetime_halves() {
        assert_eq!(filetime_to_u64(5, 0), 5);
        assert_eq!(filetime_to_u64(0, 1), 1 << 32);
        assert_eq!(filetime_to_u64(u32::MAX, 2), (2u64 << 32) + u32::MAX as u64);
    }
}

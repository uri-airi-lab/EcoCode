//! Windows: the marker pipe is an inherited handle, and the command's process
//! tree lives in a Job Object.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

use super::STOP_GRACE;

/// Environment variable holding the marker pipe's handle value. The Python
/// SDK opens it with `msvcrt.open_osfhandle`.
pub const MARKER_ENV: &str = "ECOCODE_MARKER_HANDLE";

/// Makes `writer` inheritable (Rust spawns children with handle inheritance
/// on) and tells the child its value.
pub fn pass_marker_pipe(cmd: &mut Command, writer: &os_pipe::PipeWriter) -> io::Result<()> {
    let handle = writer.as_raw_handle();
    if unsafe { SetHandleInformation(handle as HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
        return Err(io::Error::last_os_error());
    }
    cmd.env(MARKER_ENV, (handle as usize).to_string());
    Ok(())
}

/// Terminates `pid` immediately.
pub fn kill(pid: u32) {
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !h.is_null() {
            TerminateProcess(h, 1);
            CloseHandle(h);
        }
    }
}

/// A Job Object holding the command and everything it starts. It is created
/// with "kill on close", so the whole tree ends even if ecocode itself
/// crashes. (A process the command starts in the first instant after spawn,
/// before it is assigned to the job, can escape it.)
pub struct TreeGuard {
    job: HANDLE,
}

// The job handle is used from one thread at a time.
unsafe impl Send for TreeGuard {}

impl TreeGuard {
    pub fn attach(child: &Child) -> io::Result<Self> {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let guard = TreeGuard { job };
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
                || AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(guard)
        }
    }

    /// Gives the command [`STOP_GRACE`] to exit (Ctrl+C reaches it through the
    /// shared console), then terminates every process in the job.
    pub fn stop(&self, child: &mut Child, _tree: &[u32]) {
        let deadline = Instant::now() + STOP_GRACE;
        while !matches!(child.try_wait(), Ok(Some(_))) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        unsafe { TerminateJobObject(self.job, 1) };
        let _ = child.wait();
    }
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        // Kill-on-close: anything still in the job ends here.
        unsafe { CloseHandle(self.job) };
    }
}

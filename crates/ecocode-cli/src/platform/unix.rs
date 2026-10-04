//! Linux and macOS: the marker pipe is an inherited file descriptor, and a
//! cancelled command's process tree is stopped with SIGTERM, then SIGKILL.

use std::io;
use std::os::unix::io::AsRawFd;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use super::STOP_GRACE;

/// Environment variable holding the marker pipe's file descriptor number.
pub const MARKER_ENV: &str = "ECOCODE_MARKER_FD";

/// Lets the child inherit `writer` across exec and tells it the descriptor number.
pub fn pass_marker_pipe(cmd: &mut Command, writer: &os_pipe::PipeWriter) -> io::Result<()> {
    let fd = writer.as_raw_fd();
    // os_pipe sets CLOEXEC by default; clear it so the child keeps the
    // write end after exec and can send markers to it.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    cmd.env(MARKER_ENV, fd.to_string());
    Ok(())
}

/// Sends SIGKILL to `pid`.
pub fn kill(pid: u32) {
    signal(pid, libc::SIGKILL);
}

fn signal(pid: u32, sig: libc::c_int) {
    unsafe {
        libc::kill(pid as libc::pid_t, sig);
    }
}

/// Whether `pid` exists. A zombie (exited, not yet reaped) still counts, so
/// `Child::try_wait` is used for our own child.
fn is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Unix needs no setup: the tree is taken from the process table on cancel.
pub struct TreeGuard;

impl TreeGuard {
    pub fn attach(_child: &Child) -> io::Result<Self> {
        Ok(TreeGuard)
    }

    /// Stops the command and everything it started: SIGTERM to the whole
    /// `tree`, then SIGKILL to whatever is still running after [`STOP_GRACE`].
    pub fn stop(&self, child: &mut Child, tree: &[u32]) {
        let child_pid = child.id();
        let others: Vec<u32> = tree.iter().copied().filter(|&p| p != child_pid).collect();

        signal(child_pid, libc::SIGTERM);
        for &pid in &others {
            signal(pid, libc::SIGTERM);
        }
        let deadline = Instant::now() + STOP_GRACE;
        let all_exited =
            |child: &mut Child| matches!(child.try_wait(), Ok(Some(_))) && !others.iter().any(|&p| is_alive(p));
        while !all_exited(child) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        for &pid in &others {
            if is_alive(pid) {
                kill(pid);
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

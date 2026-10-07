//! Everything OS-specific in the CLI. Both modules expose the same API:
//! - `MARKER_ENV` and `pass_marker_pipe(cmd, writer)`: let the child inherit
//!   the marker pipe's write end and tell the Python SDK where it is
//! - `TreeGuard::attach(child)`: track the child's whole process tree, and
//!   `TreeGuard::stop(child, tree)` to end it when a run is cancelled
//! - `kill(pid)`: forcefully terminate one process (used by the panic hook)

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// How long a cancelled command gets to exit on its own before it is killed.
pub const STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

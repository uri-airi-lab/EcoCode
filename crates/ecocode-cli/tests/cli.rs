//! End-to-end tests of the `ecocode` binary with fake sensors, so they run on
//! any machine. Tests that need a POSIX shell or signals are Unix-only.

use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn ecocode() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ecocode"));
    cmd.env_remove("ECOCODE_SENSORS").env_remove("ECOCODE_API_KEY");
    cmd
}

/// A command that runs for about `secs` seconds on every OS.
fn sleeper(secs: u32) -> Vec<String> {
    if cfg!(windows) {
        vec!["ping".into(), "-n".into(), (secs + 1).to_string(), "127.0.0.1".into()]
    } else {
        vec!["sleep".into(), secs.to_string()]
    }
}

fn wait_with_timeout(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("ecocode did not exit within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

fn run_ok(cmd: &mut Command) -> Output {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn read_json(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("invalid JSON ({e}):\n{text}"))
}

#[test]
fn doctor_lists_sensors() {
    let out = run_ok(ecocode().args(["doctor", "--sensors", "fake"]));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[ok] fake"), "{stdout}");
    assert!(stdout.contains("Result: measuring cpu"), "{stdout}");
}

#[test]
fn doctor_json_lists_channels() {
    let out = run_ok(ecocode().args(["doctor", "--json", "--sensors", "fake"]));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let domains: Vec<&str> = report["channels"].as_array().unwrap().iter().map(|c| c["domain"].as_str().unwrap()).collect();
    assert_eq!(domains, ["cpu", "dram", "gpu"]);
    assert_eq!(report["diagnostics"][0]["status"], "available");
    assert!(report["logical_cores"].as_u64().unwrap() > 0);
}

#[test]
fn doctor_never_fails_on_real_hardware() {
    run_ok(ecocode().arg("doctor"));
}

#[test]
fn run_writes_json_records() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    run_ok(
        ecocode()
            .args(["run", "--sensors", "fake", "-o", "json", "-i", "0.2", "-f"])
            .arg(&file)
            .arg("--")
            .args(sleeper(1)),
    );
    let records = read_json(&file);
    assert!(records.len() >= 3, "{} records", records.len());
    for (i, r) in records.iter().enumerate() {
        assert_eq!(r["id"], i as u64 + 1);
        assert_eq!(r["gpu_metrics"].as_array().unwrap().len(), 1);
        let domains: Vec<&str> =
            r["socket_metrics"].as_array().unwrap().iter().map(|s| s["domain"].as_str().unwrap()).collect();
        assert_eq!(domains, ["cpu", "dram"]);
        assert!(r["gpu_energy"].as_f64().unwrap() > 0.0, "{r}");
        assert!((0.1..0.6).contains(&r["elapsed_secs"].as_f64().unwrap()), "{r}");
    }
}

#[test]
fn run_writes_csv_with_gpu_columns() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.csv");
    run_ok(
        ecocode()
            .args(["run", "--sensors", "fake", "-o", "csv", "-i", "0.2", "-f"])
            .arg(&file)
            .arg("--")
            .args(sleeper(2)),
    );
    let text = std::fs::read_to_string(&file).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "ID,Run ID,PID,Timestamp,CPU%,CPU(W),GPU%,GPU(W),MEM%,MEM(W),IGPU%,IGPU(W),GPU0%,GPU0(W),Phase,TokensProcessed,TrainingStep,Loss,LearningRate,SequenceLength"
    );
    assert!(lines.count() >= 3);
}

#[test]
fn real_sensors_never_abort_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    run_ok(
        ecocode()
            .args(["run", "-o", "json", "-i", "0.2", "-f"])
            .arg(&file)
            .arg("--")
            .args(sleeper(1)),
    );
    assert!(!read_json(&file).is_empty());
}

/// Real sensors on this machine measure a CPU burn. Opt-in: run with
/// `cargo test -- --ignored` on a machine where `ecocode doctor` reports a
/// CPU energy sensor.
#[test]
#[ignore = "needs a CPU energy sensor (run with --ignored)"]
fn hw_run_measures_cpu_energy() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    let python = if cfg!(windows) { "python" } else { "python3" };
    run_ok(
        ecocode()
            .args(["run", "-o", "json", "-i", "0.5", "-f"])
            .arg(&file)
            .args(["--", python, "-c", "import time\nt=time.time()\nwhile time.time()-t<2: pass"]),
    );
    let records = read_json(&file);
    let cpu_j: f64 = records.iter().map(|r| r["cpu_energy"].as_f64().unwrap()).sum();
    let secs: f64 = records.iter().map(|r| r["elapsed_secs"].as_f64().unwrap()).sum();
    assert!(cpu_j / secs > 0.2, "busy loop measured at only {} W", cpu_j / secs);
}

#[test]
fn run_stops_promptly_and_passes_on_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    let started = Instant::now();
    let mut cmd = ecocode();
    cmd.args(["run", "--sensors", "fake", "-o", "json", "-i", "0.2", "-f"]).arg(&file).arg("--");
    if cfg!(windows) {
        cmd.args(["cmd", "/C", "exit 3"]);
    } else {
        cmd.args(["sh", "-c", "exit 3"]);
    }
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(started.elapsed() < Duration::from_secs(5));
    read_json(&file); // the array is closed even with zero or one records
}

#[test]
fn last_sample_covers_the_end_of_the_command() {
    // At a 0.7 s interval a 1 s command ends between ticks; the records must
    // still cover all of it, not stop at the last tick before it exited.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    let started = Instant::now();
    run_ok(
        ecocode()
            .args(["run", "--sensors", "fake", "-o", "json", "-i", "0.7", "-f"])
            .arg(&file)
            .arg("--")
            .args(sleeper(1)),
    );
    let covered: f64 = read_json(&file).iter().map(|r| r["elapsed_secs"].as_f64().unwrap()).sum();
    assert!(covered >= 0.95, "records cover {covered} s of a 1 s command");
    assert!(covered <= started.elapsed().as_secs_f64(), "records cover {covered} s, longer than the run");
}

#[test]
fn invalid_arguments_fail_with_a_message() {
    let out = ecocode().args(["run", "-i", "0.01", "--", "true"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("at least 0.1 seconds"));

    let out = ecocode().args(["run", "-o", "csv", "--", "true"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--file is required"));

    let out = ecocode()
        .args(["monitor", "--run-id", "1", "--pid", "1", "--sensors", "fake"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ECOCODE_API_KEY"));
}

#[test]
fn monitor_local_reads_stdin_markers_and_stops_on_eof() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.json");
    let mut child = ecocode()
        .args(["monitor-local", "--sensors", "fake", "--run-id", "0", "--interval", "0.2", "--exporter", "json"])
        .arg("--pid")
        .arg(std::process::id().to_string())
        .arg("--file")
        .arg(&file)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, r#"{{"event":"phase","value":"forward"}}"#).unwrap();
    writeln!(stdin, r#"{{"event":"custom","key":"grad_norm","value":0.5}}"#).unwrap();
    std::thread::sleep(Duration::from_millis(700));
    drop(stdin); // the SDK closing the pipe ends the run
    let out = wait_with_timeout(child, Duration::from_secs(5));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let records = read_json(&file);
    let last = records.last().unwrap();
    assert_eq!(last["phase"], "forward");
    assert_eq!(last["custom_metrics"]["grad_norm"], 0.5);
    assert_eq!(last["run_id"], 1, "monitor-local creates its own run");
}

#[test]
fn monitor_local_stops_when_target_exits() {
    let mut target = Command::new(&sleeper(1)[0]).args(&sleeper(1)[1..]).stdout(Stdio::null()).spawn().unwrap();
    let child = ecocode()
        .args(["monitor-local", "--sensors", "fake", "--run-id", "5", "--interval", "0.2"])
        .arg("--pid")
        .arg(target.id().to_string())
        .stdin(Stdio::piped()) // kept open: only the target's exit can end the run
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    target.wait().unwrap();
    let out = wait_with_timeout(child, Duration::from_secs(5));
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("[TERMINAL EXPORT]"));
}

#[cfg(unix)]
mod unix {
    use super::*;

    #[test]
    fn markers_reach_the_run_through_the_inherited_pipe() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("out.json");
        let script = r#"printf '{"event":"phase","value":"forward","sent_at_ns":1}\n{"event":"step","value":3}\n' >&"$ECOCODE_MARKER_FD"; echo "$ECOCODE_RUN_ID" > run_id; sleep 1"#;
        let out = run_ok(
            ecocode()
                .current_dir(dir.path())
                .args(["run", "--sensors", "fake", "-o", "json", "-i", "0.2", "-f"])
                .arg(&file)
                // bash, not sh: dash only accepts single-digit descriptors in
                // redirections, and the pipe can be fd 10 or higher.
                .args(["--", "bash", "-c", script]),
        );
        let records = read_json(&file);
        let last = records.last().unwrap();
        assert_eq!(last["phase"], "forward");
        assert_eq!(last["training_step"], 3);
        assert_eq!(std::fs::read_to_string(dir.path().join("run_id")).unwrap().trim(), "1");
        assert!(String::from_utf8_lossy(&out.stdout).contains("Marker pipe latency: 1 samples"));
    }

    #[test]
    fn ctrl_c_saves_the_run_and_stops_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("out.json");
        let child = ecocode()
            .current_dir(dir.path())
            .args(["run", "--sensors", "fake", "-o", "json", "-i", "0.2", "-f"])
            .arg(&file)
            .args(["--", "sh", "-c", "echo $$ > child_pid; exec sleep 30"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
        let out = wait_with_timeout(child, Duration::from_secs(5));
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stderr).contains("Ctrl+C received"));
        assert!(!read_json(&file).is_empty());

        let pid: i32 = std::fs::read_to_string(dir.path().join("child_pid")).unwrap().trim().parse().unwrap();
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        assert!(!alive, "child {pid} still running");
    }

    /// SIGTERM to ecocode alone (as a process manager would send) stops the
    /// command's whole process tree, including background grandchildren.
    #[test]
    fn termination_stops_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let child = ecocode()
            .current_dir(dir.path())
            .args(["run", "--sensors", "fake", "-o", "terminal", "-i", "0.2", "--"])
            .args(["sh", "-c", "sleep 30 & echo $! > grandchild_pid; wait"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
        let out = wait_with_timeout(child, Duration::from_secs(8));
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

        let pid: i32 = std::fs::read_to_string(dir.path().join("grandchild_pid")).unwrap().trim().parse().unwrap();
        std::thread::sleep(Duration::from_millis(200)); // let init reap it
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        assert!(!alive, "grandchild {pid} still running");
    }
}

#[cfg(windows)]
mod windows {
    //! The child processes are this test binary itself, re-run in a helper
    //! mode (env `ECOCODE_TEST_HELPER`), so the tests need no PowerShell.
    use super::*;

    /// Runs `helper` (one of the `helper_*` tests below) as a child command.
    fn helper_command(helper: &str) -> Vec<String> {
        vec![
            std::env::current_exe().unwrap().to_string_lossy().into_owned(),
            "--exact".into(),
            format!("windows::{helper}"),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ]
    }

    fn helper_mode(name: &str) -> bool {
        std::env::var("ECOCODE_TEST_HELPER").as_deref() == Ok(name)
    }

    /// Child: writes two markers to the inherited handle, records the run ID, waits.
    #[test]
    fn helper_write_markers() {
        if !helper_mode("markers") {
            return;
        }
        use std::os::windows::io::{FromRawHandle, RawHandle};
        let handle: usize = std::env::var("ECOCODE_MARKER_HANDLE").unwrap().parse().unwrap();
        let mut pipe = unsafe { std::fs::File::from_raw_handle(handle as RawHandle) };
        pipe.write_all(b"{\"event\":\"phase\",\"value\":\"forward\"}\n{\"event\":\"step\",\"value\":3}\n").unwrap();
        drop(pipe);
        std::fs::write("run_id", std::env::var("ECOCODE_RUN_ID").unwrap()).unwrap();
        std::thread::sleep(Duration::from_secs(2));
    }

    /// Child: starts a grandchild (sleeping), records its PID, then sleeps.
    /// The grandchild is deliberately never waited for: the test checks that
    /// the Job Object ends it.
    #[test]
    #[allow(clippy::zombie_processes)]
    fn helper_spawn_grandchild() {
        if !helper_mode("spawn") {
            return;
        }
        let grandchild = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "windows::helper_sleep", "--test-threads=1"])
            .env("ECOCODE_TEST_HELPER", "sleep")
            .spawn()
            .unwrap();
        std::fs::write("grandchild_pid", grandchild.id().to_string()).unwrap();
        std::thread::sleep(Duration::from_secs(60));
    }

    /// Grandchild: sleeps.
    #[test]
    fn helper_sleep() {
        if helper_mode("sleep") {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    #[test]
    fn markers_reach_the_run_through_the_inherited_handle() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("out.json");
        run_ok(
            ecocode()
                .current_dir(dir.path())
                .env("ECOCODE_TEST_HELPER", "markers")
                .args(["run", "--sensors", "fake", "-o", "json", "-i", "0.2", "-f"])
                .arg(&file)
                .arg("--")
                .args(helper_command("helper_write_markers")),
        );
        let records = read_json(&file);
        let last = records.last().unwrap();
        assert_eq!(last["phase"], "forward");
        assert_eq!(last["training_step"], 3);
        assert_eq!(std::fs::read_to_string(dir.path().join("run_id")).unwrap().trim(), "1");
    }

    /// The Job Object ends the command's whole tree even when ecocode itself
    /// is killed without any chance to clean up.
    #[test]
    fn killing_ecocode_ends_the_command_tree() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = ecocode()
            .current_dir(dir.path())
            .env("ECOCODE_TEST_HELPER", "spawn")
            .args(["run", "--sensors", "fake", "-o", "terminal", "-i", "0.5", "--"])
            .args(helper_command("helper_spawn_grandchild"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let pid_file = dir.path().join("grandchild_pid");
        let deadline = Instant::now() + Duration::from_secs(30);
        let pid: u32 = loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file).ok().and_then(|s| s.trim().parse().ok()) {
                break pid;
            }
            assert!(Instant::now() < deadline, "grandchild never started");
            std::thread::sleep(Duration::from_millis(200));
        };

        child.kill().unwrap(); // TerminateProcess: no cleanup code runs in ecocode
        child.wait().unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut sys = sysinfo::System::new();
        loop {
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            if sys.process(sysinfo::Pid::from_u32(pid)).is_none() {
                break;
            }
            assert!(Instant::now() < deadline, "grandchild {pid} still running");
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

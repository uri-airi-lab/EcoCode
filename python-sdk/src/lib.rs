//! PyO3 extension module for the EcoCode Python SDK.
//!
//! Exposes `EcoCodeRun` as a Python class with `__enter__`/`__exit__` context
//! manager support and all the `mark`/`log_*` methods that the integration
//! callbacks rely on.
//!
//! Usage from Python:
//! ```python
//! from ecocode._core import EcoCodeRun
//!
//! run = EcoCodeRun("my-run", api_key="eco_...", project_id=42)
//! with run:
//!     run.mark("phase", "forward")
//!     run.log_loss(1.45)
//! ```

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

use ecocode_core::http_client::{EcoCodeHttpClient, SystemInfo};
use sysinfo::System;

mod sampler;
mod socket_client;

use sampler::SamplerHandle;
use socket_client::SocketClient;

const DEFAULT_BASE_URL: &str = "http://localhost:3001";

/// Operation mode — auto-detected from environment variables.
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    /// CLI is running — markers go via Unix socket, no HTTP from Python.
    Attached { run_id: i64 },
    /// No CLI — SDK creates the run and samples energy itself.
    Standalone,
}

/// The main context manager exposed to Python.
///
/// This is the Rust replacement for `_run.py::EcoCodeRun`.
#[pyclass]
struct EcoCodeRun {
    name: String,
    mode: Mode,
    run_id: Option<i64>,
    interval: f64,
    project_id: Option<i64>,
    client: Option<Arc<EcoCodeHttpClient>>,
    sampler: Option<SamplerHandle>,
    socket: Option<SocketClient>,
}

#[pymethods]
impl EcoCodeRun {
    /// Create a new EcoCodeRun.
    ///
    /// In attached mode (ECOCODE_SOCKET + ECOCODE_RUN_ID set), api_key/project_id
    /// are ignored — the CLI already created the run.
    ///
    /// In standalone mode, api_key is required (or ECOCODE_API_KEY env var).
    #[new]
    #[pyo3(signature = (name="run", *, api_key=None, project_id=None, base_url=None, interval=1.0))]
    fn new(
        name: &str,
        api_key: Option<&str>,
        project_id: Option<i64>,
        base_url: Option<&str>,
        interval: f64,
    ) -> PyResult<Self> {
        let socket_path = std::env::var("ECOCODE_SOCKET").ok();
        let env_run_id = std::env::var("ECOCODE_RUN_ID").ok();

        // Detect mode
        if let (Some(path), Some(rid_str)) = (&socket_path, &env_run_id) {
            // Attached mode
            match rid_str.parse::<i64>() {
                Ok(run_id) => {
                    let (socket, warning) = SocketClient::new(path);
                    if let Some(msg) = warning {
                        // Use eprintln — PyO3 warnings.warn is complex and this matches
                        // the Python SDK's behavior of printing to stderr.
                        eprintln!("{}", msg);
                    }
                    return Ok(Self {
                        name: name.to_string(),
                        mode: Mode::Attached { run_id },
                        run_id: Some(run_id),
                        interval,
                        project_id: None,
                        client: None,
                        sampler: None,
                        socket: Some(socket),
                    });
                }
                Err(_) => {
                    eprintln!(
                        "[ecocode] ECOCODE_RUN_ID is not a valid integer: {:?}. \
                         Falling back to standalone mode.",
                        rid_str
                    );
                    // Fall through to standalone
                }
            }
        }

        // Standalone mode
        let resolved_key = api_key
            .map(|s| s.to_string())
            .or_else(|| std::env::var("ECOCODE_API_KEY").ok())
            .ok_or_else(|| {
                PyValueError::new_err(
                    "[ecocode] Standalone mode requires an api_key. \
                     Pass api_key=... or set the ECOCODE_API_KEY environment variable.",
                )
            })?;

        let resolved_url = base_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("ECOCODE_API_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let client = EcoCodeHttpClient::new(&resolved_url, &resolved_key).map_err(|e| {
            PyValueError::new_err(format!("[ecocode] HTTP client init failed: {}", e))
        })?;

        Ok(Self {
            name: name.to_string(),
            mode: Mode::Standalone,
            run_id: None,
            interval,
            project_id,
            client: Some(Arc::new(client)),
            sampler: None,
            socket: None,
        })
    }

    /// Context manager entry — creates the run and starts the sampler.
    fn __enter__(mut slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        if slf.mode == Mode::Standalone {
            // Clone the Arc so we don't hold an immutable borrow on `slf` while mutating it.
            let client = Arc::clone(
                slf.client
                    .as_ref()
                    .expect("client must be set in standalone mode"),
            );
            let project_id = slf.project_id.unwrap_or(0);
            let name = slf.name.clone();
            let interval = slf.interval;

            // Create the run
            let started_at = chrono::Utc::now().to_rfc3339();
            let args: Vec<String> = std::env::args().collect();
            let command = args.join(" ");

            match client.create_run(project_id, &name, None, Some(&started_at), Some(&command)) {
                Ok(run_id) => {
                    slf.run_id = Some(run_id);

                    // Save system info (best effort)
                    if let Err(e) = save_system_info(&client, run_id) {
                        eprintln!("[ecocode] Could not save system info: {}", e);
                    }

                    // Start sampler thread
                    let pid = std::process::id();
                    let sampler = SamplerHandle::start(client, run_id, interval, pid);
                    slf.sampler = Some(sampler);
                }
                Err(e) => {
                    eprintln!("[ecocode] Could not create run: {}", e);
                }
            }
        }
        slf
    }

    /// Context manager exit — stops the sampler and completes the run.
    #[pyo3(signature = (exc_type=None, _exc_val=None, _exc_tb=None))]
    fn __exit__(
        &mut self,
        exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        if self.mode == Mode::Standalone {
            // Stop the sampler
            if let Some(ref mut sampler) = self.sampler {
                sampler.stop();
            }
            self.sampler = None;

            // Complete the run
            let status = if exc_type.is_some() {
                "failed"
            } else {
                "finished"
            };
            let ended_at = chrono::Utc::now().to_rfc3339();

            if let (Some(client), Some(run_id)) = (&self.client, self.run_id) {
                if let Err(e) = client.complete_run(run_id, status, Some(&ended_at)) {
                    eprintln!("[ecocode] Could not complete run: {}", e);
                }
            }
        } else if let Some(ref socket) = self.socket {
            socket.close();
        }

        false // never suppress exceptions from the training loop
    }

    // ── Public marker API ────────────────────────────────────────────

    /// Send a semantic phase marker.
    fn mark(&self, key: &str, value: &Bound<'_, PyAny>) {
        let value_str = value.str().map(|s| s.to_string()).unwrap_or_else(|_| {
            eprintln!("[ecocode warning] run.mark(\"{}\", ...) could not convert value to string, using empty string", key);
            String::new()
        });

        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({
                        "event": key,
                        "value": value_str,
                    });
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    match key {
                        "phase" => sampler.set_phase(&value_str),
                        "epoch" => sampler.set_phase(&format!("epoch:{}", value_str)),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Mark the start of an epoch. Stored as phase = 'epoch:<n>'.
    fn log_epoch(&self, epoch: i64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({
                        "event": "epoch",
                        "value": epoch,
                    });
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_phase(&format!("epoch:{}", epoch));
                }
            }
        }
    }

    /// Record the current optimizer step (gradient update count).
    fn log_step(&self, step: i64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "step", "value": step});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_training_step(step);
                }
            }
        }
    }

    /// Record the cumulative number of tokens seen by the model.
    fn log_tokens(&self, count: i64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "tokens", "count": count});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_tokens(count);
                }
            }
        }
    }

    /// Record the current training loss.
    fn log_loss(&self, loss: f64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "loss", "value": loss});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_loss(loss);
                }
            }
        }
    }

    /// Record the current learning rate.
    fn log_lr(&self, lr: f64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "learning_rate", "value": lr});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_learning_rate(lr);
                }
            }
        }
    }

    /// Record the current sequence length.
    fn log_sequence_length(&self, seq_len: i32) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "sequence_length", "value": seq_len});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_sequence_length(seq_len);
                }
            }
        }
    }

    /// Record the current model accuracy (0-100).
    fn log_accuracy(&self, accuracy: f64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({"event": "accuracy", "value": accuracy});
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_accuracy(accuracy);
                }
            }
        }
    }

    /// Record an arbitrary custom metric (key-value pair).
    /// Values are accumulated in a JSON map and sent with each record.
    fn log_custom(&self, key: &str, value: f64) {
        match &self.mode {
            Mode::Attached { .. } => {
                if let Some(ref socket) = self.socket {
                    let event = serde_json::json!({
                        "event": "custom",
                        "key": key,
                        "value": value,
                    });
                    socket.send_event(&event);
                }
            }
            Mode::Standalone => {
                if let Some(ref sampler) = self.sampler {
                    sampler.set_custom_metric(key.to_string(), value);
                }
            }
        }
    }
}

// ── Internal helpers ────────────────────────────────────────────────────────

/// Collect and send system hardware info (CPU name, cores, RAM, GPU names).
fn save_system_info(
    client: &EcoCodeHttpClient,
    run_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let sys = System::new_all();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c: &sysinfo::Cpu| c.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let cpu_cores = sys.cpus().len() as u32;
    let ram_gb = sys.total_memory() as f64 / 1_073_741_824.0;

    let mut gpu_count: u32 = 0;
    let mut gpu_names: Vec<String> = Vec::new();

    #[cfg(feature = "gpu")]
    {
        if let Ok(nvml) = nvml_wrapper::Nvml::init() {
            if let Ok(count) = nvml.device_count() {
                gpu_count = count;
                for i in 0..count {
                    if let Ok(device) = nvml.device_by_index(i) {
                        if let Ok(name) = device.name() {
                            gpu_names.push(name);
                        }
                    }
                }
            }
        }
    }

    let info = SystemInfo {
        cpu_name,
        cpu_cores,
        ram_gb,
        gpu_count,
        gpu_names,
    };

    client.save_system_info(run_id, &info)
}

// ── Python module ───────────────────────────────────────────────────────────

/// The `ecocode._core` Python module.
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EcoCodeRun>()?;
    Ok(())
}

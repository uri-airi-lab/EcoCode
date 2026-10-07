//! Prometheus exporter: exposes measurement metrics via an HTTP `/metrics` endpoint.
//!
//! Starts a background HTTP server on port 9091 (one thread, `tiny_http`),
//! serving metrics in the Prometheus text exposition format.

use crate::exporters::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};
use prometheus::{Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder};
use std::error::Error;

/// Address the `/metrics` endpoint listens on.
pub const LISTEN_ADDR: &str = "0.0.0.0:9091";

/// Exports records as Prometheus gauge metrics available at `http://localhost:9091/metrics`.
///
/// Aggregate metrics (across all GPUs):
/// - `ecocode_cpu_usage`, `ecocode_cpu_energy_watts`
/// - `ecocode_gpu_usage`, `ecocode_gpu_energy_watts`
/// - `ecocode_mem_usage`, `ecocode_mem_energy_watts`
/// - `ecocode_igpu_usage`, `ecocode_igpu_energy_watts`
///
/// Per-device metrics (labeled by `device_index`, absent when no GPU present):
/// - `ecocode_gpu_usage_per_device{device_index="N"}`
/// - `ecocode_gpu_energy_watts_per_device{device_index="N"}`
pub struct PrometheusExporter {
    registry: Registry,
    cpu_usage: Gauge,
    cpu_energy: Gauge,
    gpu_usage: Gauge,
    gpu_energy: Gauge,
    mem_usage: Gauge,
    mem_energy: Gauge,
    igpu_usage: Gauge,
    igpu_energy: Gauge,
    gpu_usage_per_device: GaugeVec,
    gpu_energy_per_device: GaugeVec,
    socket_energy_per_domain: GaugeVec,
    tracker: crate::exporters::LocalSummaryTracker,
    port: u16,
}

impl PrometheusExporter {
    /// Creates a new Prometheus exporter and spawns the HTTP server in the background.
    pub fn new() -> Result<PrometheusExporter, Box<dyn Error>> {
        Self::listen(LISTEN_ADDR)
    }

    /// Like [`new`](Self::new), on another address (tests use port 0).
    pub fn listen(addr: &str) -> Result<PrometheusExporter, Box<dyn Error>> {
        let registry = Registry::new();

        let cpu_usage = Gauge::new("ecocode_cpu_usage", "CPU usage percentage")?;
        let cpu_energy = Gauge::new(
            "ecocode_cpu_energy_watts",
            "CPU energy consumption in Watts",
        )?;
        let gpu_usage = Gauge::new("ecocode_gpu_usage", "GPU usage percentage")?;
        let gpu_energy = Gauge::new(
            "ecocode_gpu_energy_watts",
            "GPU energy consumption in Watts",
        )?;
        let mem_usage = Gauge::new("ecocode_mem_usage", "Memory usage percentage")?;
        let mem_energy = Gauge::new(
            "ecocode_mem_energy_watts",
            "Memory energy consumption in Watts",
        )?;
        let igpu_usage = Gauge::new("ecocode_igpu_usage", "IGPU usage percentage")?;
        let igpu_energy = Gauge::new(
            "ecocode_igpu_energy_watts",
            "IGPU energy consumption in Watts",
        )?;
        let gpu_usage_per_device = GaugeVec::new(
            Opts::new(
                "ecocode_gpu_usage_per_device",
                "GPU usage percentage per device",
            ),
            &["device_index"],
        )?;
        let gpu_energy_per_device = GaugeVec::new(
            Opts::new(
                "ecocode_gpu_energy_watts_per_device",
                "GPU energy consumption in Watts per device",
            ),
            &["device_index"],
        )?;
        let socket_energy_per_domain = GaugeVec::new(
            Opts::new(
                "ecocode_socket_energy_watts_per_domain",
                "Socket energy consumption in Watts per domain",
            ),
            &["socket", "domain"],
        )?;

        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(cpu_energy.clone()))?;
        registry.register(Box::new(gpu_usage.clone()))?;
        registry.register(Box::new(gpu_energy.clone()))?;
        registry.register(Box::new(mem_usage.clone()))?;
        registry.register(Box::new(mem_energy.clone()))?;
        registry.register(Box::new(igpu_usage.clone()))?;
        registry.register(Box::new(igpu_energy.clone()))?;
        registry.register(Box::new(gpu_usage_per_device.clone()))?;
        registry.register(Box::new(gpu_energy_per_device.clone()))?;
        registry.register(Box::new(socket_energy_per_domain.clone()))?;

        let mut exporter = PrometheusExporter {
            registry,
            cpu_usage,
            cpu_energy,
            gpu_usage,
            gpu_energy,
            mem_usage,
            mem_energy,
            igpu_usage,
            igpu_energy,
            gpu_usage_per_device,
            gpu_energy_per_device,
            socket_energy_per_domain,
            tracker: crate::exporters::LocalSummaryTracker::new(),
            port: 0,
        };

        // Serve /metrics from a background thread.
        let server = tiny_http::Server::http(addr)
            .map_err(|e| format!("Prometheus exporter could not listen on {addr}: {e}"))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(9091);
        let registry_clone = exporter.registry.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let response = if request.url() == "/metrics" {
                    tiny_http::Response::from_string(encode_metrics(&registry_clone))
                } else {
                    tiny_http::Response::from_string("Not Found").with_status_code(404)
                };
                let _ = request.respond(response);
            }
        });
        println!("Prometheus metrics available at http://localhost:{port}/metrics");
        exporter.port = port;

        Ok(exporter)
    }
}

impl Exporter for PrometheusExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Prometheus
    }

    /// Updates the Prometheus gauge values with the latest measurement.
    /// Energy fields are converted from Joules to average Watts (J / elapsed_secs).
    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>> {
        self.tracker.add_record(&record);
        let es = record.elapsed_secs.max(0.001);
        self.cpu_usage.set(record.cpu_usage);
        self.cpu_energy.set(record.cpu_energy / es);
        self.gpu_usage.set(record.gpu_usage);
        self.gpu_energy.set(record.gpu_energy / es);
        self.mem_usage.set(record.mem_usage);
        // Only update optional gauges when the domain is present on this system.
        // Absent domains leave the gauge at its last value (or 0.0 on first run).
        if let Some(j) = record.mem_energy {
            self.mem_energy.set(j / es);
        }
        if let Some(v) = record.igpu_usage {
            self.igpu_usage.set(v);
        }
        if let Some(j) = record.igpu_energy {
            self.igpu_energy.set(j / es);
        }
        for gm in &record.gpu_metrics {
            let idx = gm.device_index.to_string();
            self.gpu_usage_per_device
                .with_label_values(&[&idx])
                .set(gm.gpu_usage);
            self.gpu_energy_per_device
                .with_label_values(&[&idx])
                .set(gm.gpu_energy / es);
        }
        for sm in &record.socket_metrics {
            let socket_str = sm.socket.to_string();
            self.socket_energy_per_domain
                .with_label_values(&[&socket_str, &sm.domain])
                .set(sm.energy / es);
        }
        Ok(())
    }

    fn export(&mut self) -> Result<(), Box<dyn Error>> {
        println!("\n[PROMETHEUS EXPORT]");
        println!("Prometheus server is running on http://localhost:{}/metrics", self.port);
        Ok(())
    }

    fn export_line(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn get_or_create_project(
        &mut self,
        _project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        Ok(ProjectInfo {
            id: 0,
            selected_collaborator_name: collaborator_name.map(str::to_owned),
            collaborators: collaborator_name.into_iter().map(str::to_owned).collect(),
            is_new: false,
        })
    }
    fn create_run(
        &mut self,
        _run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        _started_at: Option<&str>,
        _command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(1)
    }

    fn save_system_info(
        &mut self,
        _run_id: i64,
        _info: SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn complete_run(
        &mut self,
        _run_id: i64,
        _status: &str,
        _ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    fn update_project_config(
        &mut self,
        _project_id: i64,
        intensity: Option<f64>,
        price: Option<f64>,
        _address: Option<String>,
        _use_api_for_cost: Option<bool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.update_config(intensity, price);
        Ok(())
    }
    fn get_carbon_config(
        &mut self,
        _project_id: i64,
    ) -> Result<CarbonConfig, Box<dyn std::error::Error>> {
        let intensity_val = self.tracker.intensity.unwrap_or(445.0);
        Ok(CarbonConfig {
            resolved_intensity_g_per_kwh: intensity_val,
            pue_factor: 1.0,
            intensity_source: if self.tracker.intensity.is_some() { "cli_override".to_string() } else { "static_WORLD_fallback".to_string() },
            manual_cost_per_kwh: self.tracker.price,
            manual_currency: Some("USD".to_string()),
            use_api_for_cost: false,
        })
    }
    fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.tracker.print_summary();
        Ok(())
    }
}

fn encode_metrics(registry: &Registry) -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        return format!("# Error encoding metrics: {}\n", e);
    }
    String::from_utf8(buffer).unwrap_or_else(|_| "# Error: metrics contained invalid UTF-8\n".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn get(port: u16, path: &str) -> String {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_latest_record_as_watts() {
        let mut exporter = PrometheusExporter::listen("127.0.0.1:0").unwrap();
        let record: Record = serde_json::from_str(
            r#"{"id":1,"run_id":0,"pid":5,"timestamp":"t","elapsed_secs":2.0,"cpu_usage":50.0,"cpu_energy":30.0,
                "gpu_usage":0.0,"gpu_energy":0.0,"mem_usage":3.0,"mem_energy":4.0,"igpu_usage":null,
                "igpu_energy":null,"gpu_metrics":[{"device_index":0,"gpu_usage":10.0,"gpu_energy":8.0}],
                "socket_metrics":[{"socket":0,"domain":"cpu","energy":30.0}],"phase":null,"tokens_processed":null,
                "training_step":null,"loss":null,"learning_rate":null,"sequence_length":null}"#,
        )
        .unwrap();
        exporter.add_record(record).unwrap();
        let body = get(exporter.port, "/metrics");
        assert!(body.contains("ecocode_cpu_energy_watts 15"), "{body}");
        assert!(body.contains("ecocode_mem_energy_watts 2"), "{body}");
        assert!(body.contains(r#"ecocode_gpu_energy_watts_per_device{device_index="0"} 4"#), "{body}");
        assert!(body.contains(r#"ecocode_socket_energy_watts_per_domain{domain="cpu",socket="0"} 15"#), "{body}");
        assert!(get(exporter.port, "/other").starts_with("HTTP/1.0 404") || get(exporter.port, "/other").contains(" 404 "));
    }

    #[test]
    fn port_in_use_is_a_clear_error() {
        let first = PrometheusExporter::listen("127.0.0.1:0").unwrap();
        let addr = format!("127.0.0.1:{}", first.port);
        let err = PrometheusExporter::listen(&addr).err().unwrap().to_string();
        assert!(err.contains("could not listen"), "{err}");
    }
}

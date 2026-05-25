//! Prometheus exporter — exposes measurement metrics via an HTTP `/metrics` endpoint.
//!
//! Starts a background HTTP server on port 9091 using `warp`, serving metrics
//! in the Prometheus text exposition format.

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;
use prometheus::{Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder};
use std::error::Error;
use warp::Filter;

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
}

impl PrometheusExporter {
    /// Creates a new Prometheus exporter and spawns the HTTP server in the background.
    pub fn new() -> Result<PrometheusExporter, Box<dyn Error>> {
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

        let exporter = PrometheusExporter {
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
        };

        // Spawn a warp HTTP server in the background to serve metrics
        let registry_clone = exporter.registry.clone();
        let metrics_route = warp::path("metrics")
            .map(move || encode_metrics(&registry_clone))
            .boxed();

        tokio::spawn(async move {
            println!("Prometheus metrics available at http://localhost:9091/metrics");
            warp::serve(metrics_route).run(([0, 0, 0, 0], 9091)).await;
        });

        Ok(exporter)
    }
}

#[async_trait(?Send)]
impl Exporter for PrometheusExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Prometheus
    }

    /// Updates the Prometheus gauge values with the latest measurement.
    /// Energy fields are converted from Joules to average Watts (J / elapsed_secs).
    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>> {
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

    async fn export(&mut self) -> Result<(), Box<dyn Error>> {
        println!("\n[PROMETHEUS EXPORT]");
        println!("Prometheus server is running on http://localhost:9091/metrics");
        Ok(())
    }

    async fn export_line(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    async fn get_or_create_project(
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
    async fn create_run(
        &mut self,
        _run_name: &str,
        _project_id: i64,
        _collaborator_name: Option<&str>,
        _started_at: Option<&str>,
        _command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(1)
    }

    async fn save_system_info(
        &mut self,
        _run_id: i64,
        _info: crate::exporter::SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn complete_run(
        &mut self,
        _run_id: i64,
        _status: &str,
        _ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    async fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
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

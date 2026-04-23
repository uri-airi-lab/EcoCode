use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record};
use async_trait::async_trait;

pub struct TerminalExporter {
    records: Vec<Record>,
    pub first_record: bool,
}

impl TerminalExporter {
    pub fn new() -> TerminalExporter {
        TerminalExporter {
            records: Vec::new(),
            first_record: true,
        }
    }
}

#[async_trait(?Send)]
impl Exporter for TerminalExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Terminal
    }
    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        self.records.push(record);
        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n[TERMINAL EXPORT]");
        println!("\n{}", "=".repeat(128));
        println!(
            "{:<5} {:<8} {:<32} {:<10} {:<10} {:<12} {:<12} {:<12} {:<12} {:<12} {:<12}",
            "ID",
            "PID",
            "Timestamp",
            "CPU%",
            "CPU(J)",
            "GPU%",
            "GPU(J)",
            "MEM%",
            "MEM(J)",
            "IGPU%",
            "IGPU(J)"
        );
        println!("{}", "-".repeat(128));

        for record in &self.records {
            let mem_energy_s = record
                .mem_energy
                .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
            let igpu_usage_s = record
                .igpu_usage
                .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
            let igpu_energy_s = record
                .igpu_energy
                .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
            println!(
                "{:<5} {:<8} {:<32} {:<10.2} {:<10.2} {:<12.3} {:<12.3} {:<12.3} {:<12} {:<12} {:<12}",
                record.id,
                record.pid,
                record.timestamp,
                record.cpu_usage,
                record.cpu_energy,
                record.gpu_usage,
                record.gpu_energy,
                record.mem_usage,
                mem_energy_s,
                igpu_usage_s,
                igpu_energy_s
            );
            for gm in &record.gpu_metrics {
                println!(
                    "      └─ GPU {}: Usage {:.1}%, Energy {:.3} J",
                    gm.device_index, gm.gpu_usage, gm.gpu_energy
                );
            }
            for sm in &record.socket_metrics {
                println!(
                    "      └─ Socket {} ({}): {:.4} J",
                    sm.socket, sm.domain, sm.energy
                );
            }
        }
        println!("{}\n", "=".repeat(128));

        Ok(())
    }

    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let r = self.records.last().unwrap();

        if self.first_record {
            println!("\n{}", "=".repeat(128));
            println!(
                "{:<5} {:<8} {:<40} {:<10} {:<10} {:<12} {:<12} {:<12} {:<12} {:<12} {:<12}",
                "ID",
                "PID",
                "Timestamp",
                "CPU%",
                "CPU(J)",
                "GPU%",
                "GPU(J)",
                "MEM%",
                "MEM(J)",
                "IGPU%",
                "IGPU(J)"
            );
            println!("{}", "-".repeat(128));
            self.first_record = false;
        }

        let mem_energy_s = r
            .mem_energy
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
        let igpu_usage_s = r
            .igpu_usage
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));
        let igpu_energy_s = r
            .igpu_energy
            .map_or_else(|| "N/A".to_string(), |v| format!("{:.3}", v));

        println!(
            "{:<5} {:<8} {:<40} {:<10.2} {:<10.2} {:<12.3} {:<12.3} {:<12.3} {:<12} {:<12} {:<12}",
            r.id,
            r.pid,
            r.timestamp,
            r.cpu_usage,
            r.cpu_energy,
            r.gpu_usage,
            r.gpu_energy,
            r.mem_usage,
            mem_energy_s,
            igpu_usage_s,
            igpu_energy_s
        );

        for gm in &r.gpu_metrics {
            println!(
                "      └─ GPU {}: Usage {:.1}%, Energy {:.3} J",
                gm.device_index, gm.gpu_usage, gm.gpu_energy
            );
        }
        for sm in &r.socket_metrics {
            println!(
                "      └─ Socket {} ({}): {:.4} J",
                sm.socket, sm.domain, sm.energy
            );
        }

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
        Ok(0)
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
        Ok(()) //TODO: implement
    }
}

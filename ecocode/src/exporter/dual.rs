use crate::exporter::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};
use crate::exporter::online::OnlineExporter;
use async_trait::async_trait;

pub struct DualExporter {
    primary: Box<dyn Exporter>,
    online: OnlineExporter,
    online_project_id: Option<i64>,
    online_run_id: Option<i64>,
}

impl DualExporter {
    pub fn new(primary: Box<dyn Exporter>, online: OnlineExporter) -> Self {
        DualExporter {
            primary,
            online,
            online_project_id: None,
            online_run_id: None,
        }
    }
}

#[async_trait(?Send)]
impl Exporter for DualExporter {
    fn exporter_type(&self) -> ExporterType {
        self.primary.exporter_type()
    }

    async fn get_or_create_project(
        &mut self,
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        match self.online.get_or_create_project(project_name, collaborator_name).await {
            Ok(info) => { self.online_project_id = Some(info.id); }
            Err(e) => { eprintln!("[online] get_or_create_project warning: {}", e); }
        }
        self.primary.get_or_create_project(project_name, collaborator_name).await
    }

    async fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        if let Some(online_project_id) = self.online_project_id {
            match self.online.create_run(run_name, online_project_id, collaborator_name, started_at, command).await {
                Ok(id) => { self.online_run_id = Some(id); }
                Err(e) => { eprintln!("[online] create_run warning: {}", e); }
            }
        }
        self.primary.create_run(run_name, project_id, collaborator_name, started_at, command).await
    }

    async fn save_system_info(
        &mut self,
        run_id: i64,
        info: SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(online_run_id) = self.online_run_id {
            if let Err(e) = self.online.save_system_info(online_run_id, info.clone()).await {
                eprintln!("[online] save_system_info warning: {}", e);
            }
        }
        self.primary.save_system_info(run_id, info).await
    }

    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(online_run_id) = self.online_run_id {
            let mut online_record = record.clone();
            online_record.run_id = online_run_id;
            if let Err(e) = self.online.add_record(online_record).await {
                eprintln!("[online] add_record warning: {}", e);
            }
        }
        self.primary.add_record(record).await
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.primary.export().await
    }

    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.primary.export_line().await
    }

    async fn complete_run(
        &mut self,
        run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(online_run_id) = self.online_run_id {
            if let Err(e) = self.online.complete_run(online_run_id, status, ended_at).await {
                eprintln!("[online] complete_run warning: {}", e);
            }
        }
        self.primary.complete_run(run_id, status, ended_at).await
    }

    async fn run_summary(&mut self, run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.primary.run_summary(run_id).await
    }

    async fn get_carbon_config(
        &mut self,
        project_id: i64,
    ) -> Result<CarbonConfig, Box<dyn std::error::Error>> {
        if let Some(online_project_id) = self.online_project_id {
            match self.online.get_carbon_config(online_project_id).await {
                Ok(config) => return Ok(config),
                Err(e) => { eprintln!("[online] get_carbon_config warning: {}", e); }
            }
        }
        self.primary.get_carbon_config(project_id).await
    }

    async fn update_project_config(
        &mut self,
        project_id: i64,
        intensity: Option<f64>,
        price: Option<f64>,
        address: Option<String>,
        use_api_for_cost: Option<bool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.primary.update_project_config(project_id, intensity, price, address, use_api_for_cost).await
    }
}

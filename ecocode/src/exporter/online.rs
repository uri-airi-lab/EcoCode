//! Online exporter — sends measurement records to the EcoCode web platform via REST API.
//! Requires a workspace-scoped API key (`ecocode_...`) for authentication.

use crate::exporter::{Exporter, ExporterType, ProjectInfo, Record, SystemInfo, CarbonConfig};
use async_trait::async_trait;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

pub struct OnlineExporter {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Serialize, Deserialize)]
struct EcoCodeConfig {
    api_key: Option<String>,
    api_url: Option<String>,
}

impl OnlineExporter {
    /// Get the path to the EcoCode config file (~/.ecocode/config.json)
    pub fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .map_err(|_| "Could not determine home directory")?;
        let config_dir = PathBuf::from(home).join(".ecocode");
        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("config.json"))
    }

    /// Load configuration from ~/.ecocode/config.json
    fn load_config() -> Result<EcoCodeConfig, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            let config: EcoCodeConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(EcoCodeConfig {
                api_key: None,
                api_url: None,
            })
        }
    }

    /// Save API key to ~/.ecocode/config.json
    pub fn save_api_key(api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        let mut config = Self::load_config().unwrap_or(EcoCodeConfig {
            api_key: None,
            api_url: None,
        });
        config.api_key = Some(api_key.to_string());
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// Clear stored API key from ~/.ecocode/config.json
    pub fn clear_api_key() -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let mut config = Self::load_config()?;
            config.api_key = None;
            let content = serde_json::to_string_pretty(&config)?;
            fs::write(config_path, content)?;
        }
        Ok(())
    }

    pub fn new(api_key_arg: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        // Priority order:
        // 1. Command-line argument (--api-key)
        // 2. Environment variable (ECOCODE_API_KEY)
        // 3. Stored config file (~/.ecocode/config.json)
        let config = Self::load_config().ok();
        
        let api_key = api_key_arg.clone()
            .or_else(|| env::var("ECOCODE_API_KEY").ok())
            .or_else(|| config.as_ref().and_then(|c| c.api_key.clone()))
            .ok_or_else(|| {
                "Error: API Key is missing.\n\
                 Please provide it via:\n\
                 1. Command line: --api-key <KEY>\n\
                 2. Environment variable: ECOCODE_API_KEY\n\
                 3. Save it once: ecocode config set-api-key <KEY>\n\
                 \n\
                 You can get your key in the web platform under Settings > Developer."
            })?;

        // If API key was provided via command line, offer to save it
        if api_key_arg.is_some() && config.as_ref().and_then(|c| c.api_key.as_ref()) != Some(&api_key) {
            println!("💡 Tip: Save your API key to avoid entering it each time:");
            println!("   ecocode config set-api-key {}", api_key);
        }

        let base_url = env::var("ECOCODE_API_URL")
            .or_else(|_| config.and_then(|c| c.api_url).ok_or(""))
            .unwrap_or_else(|_| "http://localhost:3001".to_string());
        
        let mut headers = header::HeaderMap::new();
        let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", api_key))?;
        headers.insert(header::AUTHORIZATION, auth_value);
        
        let client = Client::builder()
            .default_headers(headers)
            .build()?;
            
        Ok(OnlineExporter { client, base_url, api_key })
    }
}

#[derive(Serialize)]
struct CreateRunPayload {
    project_id: i64,
    name: String,
    collaborator_name: Option<String>,
    started_at: Option<String>,
    command: Option<String>,
}

#[derive(Deserialize)]
struct CreateRunResponse {
    id: i64,
}

#[async_trait(?Send)]
impl Exporter for OnlineExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Online
    }

    async fn add_record(&mut self, record: Record) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/record", self.base_url, record.run_id);
        
        // Convert Record to the ingestion payload format expected by the backend
        let res = self.client.post(&url)
            .json(&record)
            .send()
            .await?;
            
        if !res.status().is_success() {
            return Err(format!("Failed to ingest record: {}", res.status()).into());
        }
        Ok(())
    }

    async fn export(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn export_line(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn get_or_create_project(
        &mut self,
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
        // For simplicity in this first version, we'll assume the project is already created
        // or we use the name to find it.
        // Actually, let's fetch projects and find by name.
        let url = format!("{}/api/projects", self.base_url);
        let res = self.client.get(&url).send().await?;
        
        #[derive(Deserialize)]
        struct Project {
            id: i64,
            name: String,
        }
        
        let projects: Vec<Project> = res.json().await?;
        let project = projects.into_iter().find(|p| p.name == project_name)
            .ok_or_else(|| format!("Project '{}' not found in your workspaces. Please create it first in the web app.", project_name))?;
            
        Ok(ProjectInfo {
            id: project.id,
            selected_collaborator_name: collaborator_name.map(|s| s.to_string()),
            collaborators: vec![],
            is_new: false,
        })
    }

    async fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run", self.base_url);
        let payload = CreateRunPayload {
            project_id,
            name: run_name.to_string(),
            collaborator_name: collaborator_name.map(|s| s.to_string()),
            started_at: started_at.map(|s| s.to_string()),
            command: command.map(|s| s.to_string()),
        };
        
        let res = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;
            
        if !res.status().is_success() {
            return Err(format!("Failed to create run: {}", res.status()).into());
        }
        
        let data: CreateRunResponse = res.json().await?;
        Ok(data.id)
    }

    async fn save_system_info(
        &mut self,
        run_id: i64,
        info: SystemInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/system-info", self.base_url, run_id);
        
        #[derive(Serialize)]
        struct SystemInfoPayload {
            cpu_name: String,
            cpu_cores: i32,
            ram_gb: f64,
            gpu_count: i32,
            gpu_names: String,
        }
        
        let payload = SystemInfoPayload {
            cpu_name: info.cpu_name,
            cpu_cores: info.cpu_cores as i32,
            ram_gb: info.ram_gb,
            gpu_count: info.gpu_count as i32,
            gpu_names: info.gpu_names.join(", "),
        };
        
        let res = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;
            
        if !res.status().is_success() {
            return Err(format!("Failed to save system info: {}", res.status()).into());
        }
        Ok(())
    }

    async fn complete_run(
        &mut self,
        run_id: i64,
        status: &str,
        ended_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/ingest/run/{}/complete", self.base_url, run_id);
        
        #[derive(Serialize)]
        struct CompletePayload {
            status: String,
            ended_at: Option<String>,
        }
        
        let payload = CompletePayload {
            status: status.to_string(),
            ended_at: ended_at.map(|s| s.to_string()),
        };
        
        let res = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;
            
        if !res.status().is_success() {
            return Err(format!("Failed to complete run: {}", res.status()).into());
        }
        Ok(())
    }

    async fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn std::error::Error>> {
        // Summary is calculated server-side in this exporter
        Ok(())
    }

    async fn get_carbon_config(
        &mut self,
        project_id: i64,
    ) -> Result<CarbonConfig, Box<dyn std::error::Error>> {
        let url = format!("{}/api/project/{}/carbon-config", self.base_url, project_id);
        let res = self.client.get(&url).send().await?;
        
        if !res.status().is_success() {
            return Ok(CarbonConfig {
                resolved_intensity_g_per_kwh: 445.0, // IEA 2025 global average fallback
                pue_factor: 1.0,
                intensity_source: "static_WORLD_fallback".to_string(),
                manual_cost_per_kwh: None,
                manual_currency: Some("USD".to_string()),
                use_api_for_cost: false,
            });
        }
        
        let config: CarbonConfig = res.json().await?;
        Ok(config)
    }
}

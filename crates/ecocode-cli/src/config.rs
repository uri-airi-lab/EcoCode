//! EcoCode configuration file (`~/.ecocode/config.json`): stored API key.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Contents of the config file. Unknown keys (such as the `api_url` older
/// versions wrote) are ignored: the backend URL is fixed.
#[derive(Default, Serialize, Deserialize)]
pub struct EcoCodeConfig {
    pub api_key: Option<String>,
}

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
pub fn load_config() -> Result<EcoCodeConfig, Box<dyn std::error::Error>> {
    let config_path = config_path()?;
    if config_path.exists() {
        let content = fs::read_to_string(config_path)?;
        let config: EcoCodeConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(EcoCodeConfig::default())
    }
}

/// Save API key to ~/.ecocode/config.json
pub fn save_api_key(api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = config_path()?;
    let mut config = load_config().unwrap_or_default();
    config.api_key = Some(api_key.to_string());
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Clear stored API key from ~/.ecocode/config.json
pub fn clear_api_key() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = config_path()?;
    if config_path.exists() {
        let mut config = load_config()?;
        config.api_key = None;
        let content = serde_json::to_string_pretty(&config)?;
        fs::write(config_path, content)?;
    }
    Ok(())
}

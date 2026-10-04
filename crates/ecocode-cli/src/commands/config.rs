use crate::cli::ConfigAction;
use crate::config;

/// Dispatches `ecocode config` subcommands (set-api-key, clear-api-key, show).
pub fn handle_config_command(action: ConfigAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::SetApiKey { api_key } => {
            config::save_api_key(&api_key)?;
            println!("API key saved successfully to ~/.ecocode/config.json");
            println!("You can now use the online exporter without --api-key flag");
            Ok(())
        }
        ConfigAction::ClearApiKey => {
            config::clear_api_key()?;
            println!("API key cleared from ~/.ecocode/config.json");
            Ok(())
        }
        ConfigAction::Show => {
            let config_path = config::config_path()?;
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                let settings: serde_json::Value = serde_json::from_str(&content)?;

                println!("EcoCode Configuration");
                println!("Location: {}", config_path.display());
                println!();

                if let Some(api_key) = settings.get("api_key").and_then(|v| v.as_str()) {
                    let masked = if api_key.len() > 8 {
                        format!("{}...{}", &api_key[..4], &api_key[api_key.len() - 4..])
                    } else {
                        "***".to_string()
                    };
                    println!("API Key: {}", masked);
                } else {
                    println!("API Key: (not set)");
                }

                println!("API URL: {}", ecocode_api::BASE_URL);
            } else {
                println!("EcoCode Configuration");
                println!("Location: {}", config_path.display());
                println!();
                println!("No configuration file found.");
                println!("Use 'ecocode config set-api-key <KEY>' to save your API key.");
            }
            Ok(())
        }
    }
}

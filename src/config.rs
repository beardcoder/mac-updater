use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub skip_steps: Vec<String>,
    pub custom_commands: Vec<CustomCommand>,
    pub cleanup_settings: CleanupSettings,
    pub notification_settings: NotificationSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomCommand {
    pub name: String,
    pub commands: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CleanupSettings {
    pub downloads_days_old: u32,
    pub screenshots_days_old: u32,
    pub dmg_files_days_old: u32,
    pub clear_browser_caches: bool,
    pub clear_system_logs: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub success_only: bool,
    pub include_stats: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            skip_steps: vec![],
            custom_commands: vec![
                CustomCommand {
                    name: "Update Homebrew Casks".to_string(),
                    commands: vec!["brew upgrade --cask".to_string()],
                    enabled: true,
                },
                CustomCommand {
                    name: "Clean iOS Simulator".to_string(),
                    commands: vec![
                        "xcrun simctl erase all".to_string(),
                        "xcrun simctl delete unavailable".to_string(),
                    ],
                    enabled: false,
                },
            ],
            cleanup_settings: CleanupSettings {
                downloads_days_old: 30,
                screenshots_days_old: 14,
                dmg_files_days_old: 7,
                clear_browser_caches: true,
                clear_system_logs: true,
            },
            notification_settings: NotificationSettings {
                enabled: true,
                success_only: false,
                include_stats: true,
            },
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path()?;
        
        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }
        
        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path()?;
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
    
    fn config_path() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".config/mac-updater/config.json"))
    }
}

use config::{Config as ConfigLoader, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

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
            custom_commands: vec![CustomCommand {
                name: "Update Homebrew Casks".to_string(),
                commands: vec!["brew upgrade --cask".to_string()],
                enabled: false,
            }],
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
    pub fn load() -> Result<Self, ConfigError> {
        let settings = ConfigLoader::builder()
            .add_source(File::with_name(".config/mac-updater/config").required(false))
            .add_source(Environment::with_prefix("MAC_UPDATER"))
            .build()?;

        // If no configuration file exists, return the default configuration
        match settings.try_deserialize::<Config>() {
            Ok(config) => Ok(config),
            Err(_) => Ok(Config::default()),
        }
    }
}

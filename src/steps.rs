//! Enthält die Definitionen für UpdaterStep und CommandStep.
use anyhow::Result;
use async_trait::async_trait;
use console::style;
use indicatif::ProgressBar;
use log::{error, info};

#[derive(Debug, Clone)]
pub struct UpdatedApp {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub package_manager: String,
}

#[derive(Debug, Clone, Default)]
pub struct StepResult {
    pub updated_apps: Vec<UpdatedApp>,
    pub success: bool,
}

#[async_trait]
pub trait UpdaterStep {
    fn description(&self) -> &str;
    async fn run(&self, pb: &ProgressBar) -> Result<StepResult>;
}

pub struct CommandStep {
    pub description: String,
    pub cmds: Vec<String>,
    pub run_command: Box<
        dyn Fn(
                String,
                ProgressBar,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
            + Send
            + Sync,
    >,
}

impl CommandStep {
    pub fn new<S: Into<String>, F>(description: S, cmds: Vec<S>, run_command: F) -> Self
    where
        F: Fn(
                String,
                ProgressBar,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            description: description.into(),
            cmds: cmds.into_iter().map(Into::into).collect(),
            run_command: Box::new(run_command),
        }
    }
    
    async fn parse_updated_apps(&self, cmd: &str, output: &str) -> Vec<UpdatedApp> {
        let mut updated_apps = Vec::new();
        
        // Parse Homebrew upgrade output
        if cmd.contains("brew upgrade") {
            // Parse Homebrew upgrade output - look for lines like "==> Upgrading 3 outdated packages:"
            // and package names in subsequent lines
            for line in output.lines() {
                if line.starts_with("==> Upgrading") || line.contains("Installing") {
                    // Skip summary lines
                    continue;
                }
                // Look for package upgrade lines (typically have version info)
                if line.contains("->") || (line.trim().len() > 0 && !line.starts_with("==>") && !line.starts_with("Warning") && !line.starts_with("Error")) {
                    let package_name = line.split_whitespace().next().unwrap_or("unknown").to_string();
                    if !package_name.is_empty() && package_name != "unknown" {
                        updated_apps.push(UpdatedApp {
                            name: package_name,
                            old_version: None,
                            new_version: None,
                            package_manager: "Homebrew".to_string(),
                        });
                    }
                }
            }
            // If no specific packages found but command ran, add generic entry
            if updated_apps.is_empty() && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "Homebrew packages updated".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "Homebrew".to_string(),
                });
            }
        }
        // Parse Mac App Store upgrades
        else if cmd.contains("mas upgrade") {
            // mas upgrade output format: "Installing AppName (version)"
            for line in output.lines() {
                if line.starts_with("Installing") {
                    if let Some(app_part) = line.strip_prefix("Installing ") {
                        if let Some(app_name) = app_part.split(" (").next() {
                            updated_apps.push(UpdatedApp {
                                name: app_name.to_string(),
                                old_version: None,
                                new_version: None,
                                package_manager: "Mac App Store".to_string(),
                            });
                        }
                    }
                }
            }
            if updated_apps.is_empty() && !output.contains("No updates available") && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "Mac App Store apps updated".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "Mac App Store".to_string(),
                });
            }
        }
        // Parse npm package updates
        else if cmd.contains("npm install -g") && cmd.contains("outdated") {
            // Look for npm install output lines
            for line in output.lines() {
                if line.contains("+ ") && line.contains("@") {
                    if let Some(package_info) = line.split("+ ").nth(1) {
                        if let Some(package_name) = package_info.split("@").next() {
                            updated_apps.push(UpdatedApp {
                                name: package_name.to_string(),
                                old_version: None,
                                new_version: None,
                                package_manager: "npm".to_string(),
                            });
                        }
                    }
                }
            }
            if updated_apps.is_empty() && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "npm packages updated".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "npm".to_string(),
                });
            }
        }
        // Parse Composer updates
        else if cmd.contains("composer global update") {
            // Look for package update lines in composer output
            for line in output.lines() {
                if line.contains("Updating ") && line.contains(" (") {
                    if let Some(package_part) = line.split("Updating ").nth(1) {
                        if let Some(package_name) = package_part.split(" (").next() {
                            updated_apps.push(UpdatedApp {
                                name: package_name.to_string(),
                                old_version: None,
                                new_version: None,
                                package_manager: "Composer".to_string(),
                            });
                        }
                    }
                }
            }
            if updated_apps.is_empty() && !output.contains("Nothing to update") && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "Composer packages updated".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "Composer".to_string(),
                });
            }
        }
        // Parse software updates
        else if cmd.contains("softwareupdate -ia") {
            // Look for "Installing" lines in softwareupdate output
            for line in output.lines() {
                if line.starts_with("Installing ") {
                    if let Some(update_name) = line.strip_prefix("Installing ") {
                        updated_apps.push(UpdatedApp {
                            name: update_name.to_string(),
                            old_version: None,
                            new_version: None,
                            package_manager: "macOS Software Update".to_string(),
                        });
                    }
                } else if line.contains("downloaded") || line.contains("installed") {
                    // Alternative parsing for different softwareupdate output formats
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() > 1 {
                        updated_apps.push(UpdatedApp {
                            name: parts[0].to_string(),
                            old_version: None,
                            new_version: None,
                            package_manager: "macOS Software Update".to_string(),
                        });
                    }
                }
            }
            if updated_apps.is_empty() && !output.contains("No updates available") && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "macOS system updates installed".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "macOS Software Update".to_string(),
                });
            }
        }
        // Parse Ruby gem updates
        else if cmd.contains("gem update") && !cmd.contains("cleanup") {
            // Look for "Updating" lines in gem output
            for line in output.lines() {
                if line.starts_with("Updating ") {
                    if let Some(gem_name) = line.strip_prefix("Updating ") {
                        if let Some(name) = gem_name.split_whitespace().next() {
                            updated_apps.push(UpdatedApp {
                                name: name.to_string(),
                                old_version: None,
                                new_version: None,
                                package_manager: "RubyGems".to_string(),
                            });
                        }
                    }
                }
            }
            if updated_apps.is_empty() && !output.contains("Nothing to update") && !output.trim().is_empty() {
                updated_apps.push(UpdatedApp {
                    name: "Ruby gems updated".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "RubyGems".to_string(),
                });
            }
        }
        // Parse oh-my-zsh updates
        else if cmd.contains(".oh-my-zsh/tools/upgrade.sh") {
            if output.contains("Hooray! Oh My Zsh has been updated") || output.contains("upgraded") {
                updated_apps.push(UpdatedApp {
                    name: "oh-my-zsh framework".to_string(),
                    old_version: None,
                    new_version: None,
                    package_manager: "oh-my-zsh".to_string(),
                });
            }
        }
        
        updated_apps
    }
}

#[async_trait]
impl UpdaterStep for CommandStep {
    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, pb: &ProgressBar) -> Result<StepResult> {
        let total_cmds = self.cmds.len();
        let mut step_result = StepResult::default();
        step_result.success = true;
        
        info!(
            "Starting step: {} ({} commands)",
            self.description, total_cmds
        );

        for (i, cmd) in self.cmds.iter().enumerate() {
            if total_cmds > 1 {
                pb.set_message(
                    style(format!(
                        "{} (step {} of {})",
                        self.description,
                        i + 1,
                        total_cmds
                    ))
                    .dim()
                    .to_string(),
                );
            }

            info!("Executing command {}/{}: {}", i + 1, total_cmds, cmd);

            match (self.run_command)(cmd.clone(), pb.clone()).await {
                Ok(output) => {
                    pb.println(
                        style(format!("✅ Command succeeded: {}", cmd))
                            .green()
                            .to_string(),
                    );
                    info!("Command succeeded: {}", cmd);
                    
                    // Parse output for app updates based on the command
                    let updated_apps = self.parse_updated_apps(cmd, &output).await;
                    step_result.updated_apps.extend(updated_apps);
                }
                Err(e) => {
                    pb.println(
                        style(format!("⚠️ Command failed: {} - {}", cmd, e))
                            .red()
                            .to_string(),
                    );
                    error!("Command `{}` failed: {:?}", cmd, e);
                    step_result.success = false;
                }
            }
        }

        info!("Completed step: {}", self.description);
        Ok(step_result)
    }
}

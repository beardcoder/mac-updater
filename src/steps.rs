//! Enthält die Definitionen für UpdaterStep und CommandStep.
use anyhow::Result;
use async_trait::async_trait;
use console::style;
use indicatif::ProgressBar;
use log::{error, info};

#[async_trait]
pub trait UpdaterStep {
    fn description(&self) -> &str;
    async fn run(&self, pb: &ProgressBar) -> Result<()>;
}

pub struct CommandStep {
    pub description: String,
    pub cmds: Vec<String>,
    pub run_command: Box<
        dyn Fn(
                String,
                ProgressBar,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
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
                -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
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
}

#[async_trait]
impl UpdaterStep for CommandStep {
    fn description(&self) -> &str {
        &self.description
    }

    async fn run(&self, pb: &ProgressBar) -> Result<()> {
        let total_cmds = self.cmds.len();
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
                Ok(_) => {
                    pb.println(
                        style(format!("✅ Command succeeded: {}", cmd))
                            .green()
                            .to_string(),
                    );
                    info!("Command succeeded: {}", cmd);
                }
                Err(e) => {
                    pb.println(
                        style(format!("⚠️ Command failed: {} - {}", cmd, e))
                            .red()
                            .to_string(),
                    );
                    error!("Command `{}` failed: {:?}", cmd, e);
                }
            }
        }

        info!("Completed step: {}", self.description);
        Ok(())
    }
}

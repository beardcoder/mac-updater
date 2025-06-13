//! Enthält die Definitionen für UpdaterStep und CommandStep.
use anyhow::Result;
use async_trait::async_trait;
use console::style;
use indicatif::ProgressBar;
use tracing::error;

#[async_trait]
pub trait UpdaterStep {
    fn description(&self) -> &str;
    async fn run(&self, pb: &ProgressBar) -> Result<()>;
}

pub struct CommandStep {
    description: String,
    cmds: Vec<String>,
    run_command: Box<
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
            if let Err(e) = (self.run_command)(cmd.clone(), pb.clone()).await {
                pb.println(
                    style(format!("⚠️ Command failed: {} - {}", cmd, e))
                        .red()
                        .to_string(),
                );
                error!("Command `{}` failed: {:?}", cmd, e);
            }
        }
        Ok(())
    }
}

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::env;
use std::io;
use std::io::Write;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

mod config;
mod logger;
mod notification;
mod steps;
mod user_input;

use config::Config;
use notification::send_notification;
use steps::{CommandStep, UpdaterStep};
use user_input::confirm;
struct Updater {
    interactive: bool,
    quiet: bool,
    steps: Vec<Box<dyn UpdaterStep + Send + Sync>>,
    multi: MultiProgress,
    #[allow(dead_code)]
    config: Config,
    stats: UpdateStats,
}

#[derive(Debug, Clone)]
struct UpdateStats {
    total_steps: usize,
    completed_steps: usize,
    skipped_steps: usize,
    failed_steps: usize,
    start_time: chrono::DateTime<Local>,
}

impl UpdateStats {
    fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            skipped_steps: 0,
            failed_steps: 0,
            start_time: Local::now(),
        }
    }

    fn duration(&self) -> chrono::Duration {
        Local::now() - self.start_time
    }
}

impl Updater {
    fn new(
        interactive: bool,
        quiet: bool,
        steps: Vec<Box<dyn UpdaterStep + Send + Sync>>,
        config: Config,
    ) -> Self {
        let total_steps = steps.len();
        Updater {
            interactive,
            quiet,
            steps,
            multi: MultiProgress::new(),
            config,
            stats: UpdateStats::new(total_steps),
        }
    }

    async fn run(mut self) -> Result<()> {
        let total_steps = self.steps.len();

        if !self.quiet {
            println!("🔧 Starting {} maintenance steps...\n", total_steps);
        }

        for (step_idx, step) in self.steps.into_iter().enumerate() {
            let desc = step.description();
            let step_num = step_idx + 1;

            if self.interactive && !confirm(desc)? {
                if !self.quiet {
                    println!(
                        "⏭️ [{}/{}] {}",
                        step_num,
                        total_steps,
                        style("Skipped.").yellow()
                    );
                }
                info!("Skipped: {}", desc);
                self.stats.skipped_steps += 1;
                continue;
            }

            if self.quiet {
                print!("\r🔧 [{}/{}] {}...", step_num, total_steps, desc);
                io::stdout().flush().ok();
            } else {
                let pb = self.multi.add(ProgressBar::new_spinner());
                pb.set_message(
                    style(format!("[{}/{}] {}...", step_num, total_steps, desc))
                        .white()
                        .to_string(),
                );
                pb.enable_steady_tick(Duration::from_millis(120));
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.green.bold} {msg}")
                        .unwrap()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "✅"]),
                );

                info!("Starting: {}", desc);

                if let Err(e) = step.run(&pb).await {
                    pb.finish_with_message(
                        style(format!(
                            "[{}/{}] ❌ Failed: {}",
                            step_num, total_steps, desc
                        ))
                        .red()
                        .bold()
                        .to_string(),
                    );
                    error!("Failed: {}: {:?}", desc, e);
                    self.stats.failed_steps += 1;
                    continue;
                }

                pb.finish_with_message(
                    style(format!("[{}/{}] ✅ {}", step_num, total_steps, desc))
                        .green()
                        .bold()
                        .to_string(),
                );
                info!("Finished: {}", desc);
                self.stats.completed_steps += 1;
                sleep(Duration::from_millis(150)).await;
            }

            if self.quiet {
                if let Err(_e) = step.run(&ProgressBar::hidden()).await {
                    print!(" ❌");
                    self.stats.failed_steps += 1;
                } else {
                    print!(" ✅");
                    self.stats.completed_steps += 1;
                }
                io::stdout().flush().ok();
            }
        }

        if self.quiet {}

        info!("Update completed: {:?}", self.stats);

        let duration = self.stats.duration();
        let minutes = duration.num_minutes();
        let seconds = duration.num_seconds() % 60;

        println!("\n{}", style("📊 Update Summary").cyan().bold());
        println!(
            "   {} Total steps: {}",
            style("✅").green(),
            self.stats.total_steps
        );
        println!(
            "   {} Completed: {}",
            style("✅").green(),
            self.stats.completed_steps
        );
        if self.stats.skipped_steps > 0 {
            println!(
                "   {} Skipped: {}",
                style("⏭️").yellow(),
                self.stats.skipped_steps
            );
        }
        if self.stats.failed_steps > 0 {
            println!(
                "   {} Failed: {}",
                style("❌").red(),
                self.stats.failed_steps
            );
        }
        println!(
            "   {} Duration: {}m {}s",
            style("⏱️").blue(),
            minutes,
            seconds
        );

        Ok(())
    }
}

#[derive(Parser)]
#[command(
    name = "Mac Updater",
    version,
    about = "Your sleek system update assistant 🧼💻"
)]
struct Args {
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
}
pub async fn run_command_with_output(cmd: String, pb: ProgressBar) -> anyhow::Result<()> {
    // Use shell to execute complex commands with pipes, redirections, etc.
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await
        .map_err(anyhow::Error::from)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Log stdout wenn vorhanden
    if !stdout.trim().is_empty() {
        info!("Command `{}` stdout: {}", cmd, stdout.trim());
    }

    // Log stderr wenn vorhanden
    if !stderr.trim().is_empty() {
        info!("Command `{}` stderr: {}", cmd, stderr.trim());
    }

    if !output.status.success() {
        pb.println(
            style(format!(
                "❌ Command `{}` failed with exit code {}. Error: {}",
                cmd,
                output.status.code().unwrap_or(-1),
                stderr.trim()
            ))
            .red()
            .to_string(),
        );
        error!(
            "Command `{}` failed with exit code {}. Error: {}",
            cmd,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
        return Err(anyhow::anyhow!(
            "Command `{}` failed with exit code {}. Error: {}",
            cmd,
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    pb.println(
        style(format!("✅ Command `{}` finished successfully", cmd))
            .green()
            .to_string(),
    );
    info!("Command `{}` completed successfully", cmd);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    logger::init_logger();

    let args = Args::parse();

    let config = Config::load().context("Failed to load configuration")?;

    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().ok();

    info!("🔧 Starting macOS maintenance and updates...");
    println!(
        "{}",
        style("🔧 Starting macOS maintenance and updates...")
            .cyan()
            .bold()
    );

    let run_command = |cmd: String, pb: ProgressBar| {
        Box::pin(run_command_with_output(cmd, pb))
            as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>
    };
    let update_steps: Vec<Box<dyn UpdaterStep + Send + Sync>> = vec![
        Box::new(CommandStep::new(
            "Updating Homebrew",
            vec!["brew update", "brew upgrade", "brew cleanup"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Upgrading App Store apps",
            vec!["mas upgrade"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Updating npm packages",
            vec![
                "npm outdated -g --parseable --depth=0 | cut -d: -f4 | xargs -I {} npm install -g {}",
            ],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Updating Composer packages",
            vec!["composer global update"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Installing system updates",
            vec!["softwareupdate -ia"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Updating Ruby gems",
            vec!["gem update --user-install", "gem cleanup"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Updating oh-my-zsh",
            vec![&format!(
                "{}/.oh-my-zsh/tools/upgrade.sh",
                env::home_dir().unwrap().display()
            )],
            run_command,
        )),
    ];

    let maintenance_steps: Vec<Box<dyn UpdaterStep + Send + Sync>> = vec![
        Box::new(CommandStep::new(
            "Clearing system caches",
            vec![
                "sudo dscacheutil -flushcache",
                "sudo killall -HUP mDNSResponder",
            ],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Cleaning download folders",
            vec![
                "[ -d ~/Downloads ] && find ~/Downloads -type f -mtime +30 -delete 2>/dev/null || true",
                "[ -d ~/Desktop ] && find ~/Desktop -name '*.dmg' -mtime +7 -delete 2>/dev/null || true",
                "[ -d ~/Desktop ] && find ~/Desktop -name 'Screenshot*' -mtime +14 -delete 2>/dev/null || true",
            ],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Optimizing disk space",
            vec![
                "sudo tmutil thinlocalsnapshots / 10000000000 4 2>/dev/null || true",
                "sudo purge",
            ],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Clearing logs and temp files",
            vec![
                "sudo rm -rf /private/var/log/asl/*.asl 2>/dev/null || true",
                "sudo rm -rf /Library/Logs/DiagnosticReports/* 2>/dev/null || true",
                "sudo rm -rf /var/folders/*/*/*/C/* 2>/dev/null || true",
                "rm -rf ~/Library/Application\\ Support/CrashReporter/* 2>/dev/null || true",
            ],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Updating Mac App Store CLI",
            vec!["mas outdated"],
            run_command,
        )),
        Box::new(CommandStep::new(
            "Optimizing Spotlight index",
            vec![
                "sudo mdutil -i off / 2>/dev/null || true",
                "sudo mdutil -E / 2>/dev/null || true",
                "sudo mdutil -i on / 2>/dev/null || true",
            ],
            run_command,
        )),
    ];

    println!(
        "\n{}",
        style("== Package and System Updates ==").cyan().bold()
    );
    let mut steps: Vec<Box<dyn UpdaterStep + Send + Sync>> = vec![];
    steps.extend(update_steps);
    println!(
        "\n{}",
        style("== System Maintenance and Optimization ==")
            .cyan()
            .bold()
    );
    steps.extend(maintenance_steps);

    Updater::new(args.interactive, args.quiet, steps, config.clone())
        .run()
        .await?;

    println!(
        "{}",
        style("🎉 All updates complete! Your system is squeaky clean!")
            .green()
            .bold()
    );
    info!("All updates complete.");

    send_notification(
        "macOS Maintenance Complete",
        "Your system has been updated and cleaned successfully.",
    )?;
    Ok(())
}

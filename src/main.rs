use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Local;
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use notify_rust::Notification;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use tracing_appender::non_blocking;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;
use which::which;

mod config;
use config::Config;

// Trait for update steps
#[async_trait]
trait UpdaterStep {
    fn description(&self) -> &str;
    async fn run(&self, pb: &ProgressBar) -> Result<()>;
}

// Concrete step executing shell commands
struct CommandStep {
    description: String,
    cmds: Vec<String>,
}

impl CommandStep {
    fn new<S: Into<String>>(description: S, cmds: Vec<S>) -> Self {
        Self {
            description: description.into(),
            cmds: cmds.into_iter().map(Into::into).collect(),
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
            // Only show command details if there are multiple commands or if it fails
            if total_cmds > 1 {
                pb.set_message(style(format!("{} (step {} of {})", self.description, i + 1, total_cmds)).dim().to_string());
            }

            if let Err(e) = run_command_with_output(cmd, pb).await {
                pb.println(style(format!("⚠️ Command failed: {} - {}", cmd, e)).red().to_string());
                error!("Command `{}` failed: {:?}", cmd, e);
            }
        }
        Ok(())
    }
}

// Orchestrator for update steps
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
    fn new(interactive: bool, quiet: bool, steps: Vec<Box<dyn UpdaterStep + Send + Sync>>, config: Config) -> Self {
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
                    println!("⏭️ [{}/{}] {}", step_num, total_steps, style("Skipped.").yellow());
                }
                info!("Skipped: {}", desc);
                self.stats.skipped_steps += 1;
                continue;
            }

            if self.quiet {
                // In quiet mode, just show a simple progress indicator
                print!("\r🔧 [{}/{}] {}...", step_num, total_steps, desc);
                io::stdout().flush().ok();
            } else {
                let pb = self.multi.add(ProgressBar::new_spinner());
                pb.set_message(style(format!("[{}/{}] {}...", step_num, total_steps, desc)).white().to_string());
                pb.enable_steady_tick(Duration::from_millis(120));
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.green.bold} {msg}")
                        .unwrap()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "✅"]),
                );
                
                info!("Starting: {}", desc);

                if let Err(e) = step.run(&pb).await {
                    pb.finish_with_message(style(format!("[{}/{}] ❌ Failed: {}", step_num, total_steps, desc)).red().bold().to_string());
                    error!("Failed: {}: {:?}", desc, e);
                    self.stats.failed_steps += 1;
                    continue;
                }

                pb.finish_with_message(style(format!("[{}/{}] ✅ {}", step_num, total_steps, desc)).green().bold().to_string());
                info!("Finished: {}", desc);
                self.stats.completed_steps += 1;
                sleep(Duration::from_millis(150)).await;
            }
            
            if self.quiet {
                // For quiet mode, run without progress bar
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
        
        if self.quiet {
            println!(); // New line after progress indicators
        }

        // Final statistics
        info!("Update completed: {:?}", self.stats);
        
        let duration = self.stats.duration();
        let minutes = duration.num_minutes();
        let seconds = duration.num_seconds() % 60;
        
        println!("\n{}", style("📊 Update Summary").cyan().bold());
        println!("   {} Total steps: {}", style("✅").green(), self.stats.total_steps);
        println!("   {} Completed: {}", style("✅").green(), self.stats.completed_steps);
        if self.stats.skipped_steps > 0 {
            println!("   {} Skipped: {}", style("⏭️").yellow(), self.stats.skipped_steps);
        }
        if self.stats.failed_steps > 0 {
            println!("   {} Failed: {}", style("❌").red(), self.stats.failed_steps);
        }
        println!("   {} Duration: {}m {}s", style("⏱️").blue(), minutes, seconds);

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
    /// Run in interactive mode (ask for confirmations)
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,
    /// Reduce output verbosity (show only essential information)
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logger()?;
    let args = Args::parse();
    
    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Clear terminal screen
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().ok();

    info!("🔧 Starting macOS maintenance and updates...");
    println!(
        "{}",
        style("🔧 Starting macOS maintenance and updates...")
            .cyan()
            .bold()
    );

    // Build steps
    let steps: Vec<Box<dyn UpdaterStep + Send + Sync>> = vec![
        Box::new(CommandStep::new(
            "Updating Homebrew",
            vec!["brew update", "brew upgrade", "brew cleanup"],
        )),
        Box::new(CommandStep::new(
            "Upgrading App Store apps",
            vec!["mas upgrade"],
        )),
        Box::new(CommandStep::new(
            "Updating npm packages",
            vec!["npm update -g"],
        )),
        Box::new(CommandStep::new(
            "Updating Composer packages",
            vec!["composer global update"],
        )),
        Box::new(CommandStep::new(
            "Installing system updates",
            vec!["softwareupdate -ia"],
        )),
        Box::new(CommandStep::new(
            "Updating Rust tools",
            vec!["cargo install-update -a"],
        )),
        Box::new(CommandStep::new(
            "Updating oh-my-zsh",
            vec![&format!(
                "{}/.oh-my-zsh/tools/upgrade.sh",
                env::home_dir().unwrap().display()
            )],
        )),
        // Neue Optimierungen:
        Box::new(CommandStep::new(
            "Clearing system caches",
            vec![
                "sudo dscacheutil -flushcache",
                "sudo killall -HUP mDNSResponder",
                "rm -rf ~/Library/Caches/com.apple.Safari/WebKitCache 2>/dev/null || true",
                "rm -rf ~/Library/Caches/Google/Chrome/Default/Cache 2>/dev/null || true",
                "rm -rf ~/Library/Caches/Firefox/Profiles/*/cache2 2>/dev/null || true",
            ],
        )),
        Box::new(CommandStep::new(
            "Cleaning download folders",
            vec![
                "[ -d ~/Downloads ] && find ~/Downloads -type f -mtime +30 -delete 2>/dev/null || true",
                "[ -d ~/Desktop ] && find ~/Desktop -name '*.dmg' -mtime +7 -delete 2>/dev/null || true",
                "[ -d ~/Desktop ] && find ~/Desktop -name 'Screenshot*' -mtime +14 -delete 2>/dev/null || true",
            ],
        )),
        Box::new(CommandStep::new(
            "Optimizing disk space",
            vec![
                "sudo tmutil thinlocalsnapshots / 10000000000 4 2>/dev/null || true",
                "sudo purge",
                "sudo periodic daily weekly monthly",
            ],
        )),
        Box::new(CommandStep::new(
            "Updating Ruby gems",
            vec!["gem update", "gem cleanup"],
        )),
        Box::new(CommandStep::new(
            "Optimizing Xcode",
            vec![
                "rm -rf ~/Library/Developer/Xcode/DerivedData 2>/dev/null || true",
                "rm -rf ~/Library/Developer/Xcode/Archives 2>/dev/null || true",
                "xcrun simctl delete unavailable",
            ],
        )),
        Box::new(CommandStep::new(
            "Clearing logs and temp files",
            vec![
                "sudo rm -rf /private/var/log/asl/*.asl 2>/dev/null || true",
                "sudo rm -rf /Library/Logs/DiagnosticReports/* 2>/dev/null || true",
                "sudo rm -rf /var/folders/*/*/*/C/* 2>/dev/null || true",
                "rm -rf ~/Library/Application\\ Support/CrashReporter/* 2>/dev/null || true",
            ],
        )),
        Box::new(CommandStep::new(
            "Rebuilding Launch Services",
            vec![
                "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user 2>/dev/null || true",
                "killall Finder 2>/dev/null || true",
            ],
        )),
        Box::new(CommandStep::new(
            "Updating Mac App Store CLI",
            vec!["mas outdated"],
        )),
        Box::new(CommandStep::new(
            "Optimizing Spotlight index",
            vec![
                "sudo mdutil -i off / 2>/dev/null || true",
                "sudo mdutil -E / 2>/dev/null || true",
                "sudo mdutil -i on / 2>/dev/null || true",
            ],
        )),
    ];

    // Run updater
    Updater::new(args.interactive, args.quiet, steps, config.clone()).run().await?;

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

async fn is_xcode_available() -> bool {
    // Check if Xcode is installed by trying to get the developer directory
    match Command::new("xcode-select")
        .arg("-p")
        .output()
        .await
    {
        Ok(output) => {
            if output.status.success() {
                let dev_dir = String::from_utf8_lossy(&output.stdout);
                let dev_path = dev_dir.trim();
                // Check if the developer directory actually exists and contains Xcode
                std::path::Path::new(dev_path).exists() && 
                std::path::Path::new(&format!("{}/usr/bin/simctl", dev_path)).exists()
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

async fn run_command_with_output(cmd: &str, pb: &ProgressBar) -> Result<()> {
    let mut parts = cmd.split_whitespace();
    let bin = parts.next().context("Empty command")?;
    
    // Special handling for Xcode commands
    if bin == "xcrun" {
        if which("xcrun").is_err() || !is_xcode_available().await {
            pb.println(
                style("Xcode not found, skipping Xcode-specific commands.")
                    .yellow()
                    .to_string(),
            );
            return Ok(());
        }
    } else if which(bin).is_err() {
        pb.println(
            style(format!("{} not found, skipping.", bin))
                .yellow()
                .to_string(),
        );
        return Ok(());
    }

    let mut child = Command::new(bin)
        .args(parts)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn process")?;

    // Capture both stdout and stderr
    let mut tasks = vec![];

    if let Some(stdout) = child.stdout.take() {
        let pb_clone = pb.clone();
        tasks.push(tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                pb_clone.println(line);
            }
        }));
    }

    if let Some(stderr) = child.stderr.take() {
        let pb_clone = pb.clone();
        tasks.push(tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                pb_clone.println(line);
            }
        }));
    }

    // Wait for all tasks to complete
    for task in tasks {
        let _ = task.await;
    }

    let status = child.wait().await.context("Process execution failed")?;
    info!("Command `{}` exited with {}", cmd, status);
    Ok(())
}

fn confirm(desc: &str) -> Result<bool> {
    let prompt = format!("Proceed with: {}?", desc);
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(true)
        .interact()?)
}

fn setup_logger() -> Result<()> {
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/mac_updater");
    fs::create_dir_all(&log_dir).context("Could not create log directory")?;

    let file_appender = rolling::never(&log_dir, "update.log");
    let (non_blocking, _guard) = non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    info!("Logger initialized at {:?}", log_dir);
    Ok(())
}

fn send_notification(summary: &str, body: &str) -> Result<()> {
    Notification::new()
        .summary(summary)
        .body(body)
        .icon("system-software-update")
        .show()
        .context("Notification failed")?;
    info!("Notification sent: {} - {}", summary, body);
    Ok(())
}

use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use notify_rust::Notification;
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

#[derive(Parser)]
#[command(
    name = "Mac Updater",
    version,
    about = "Your sleek system update assistant 🧼💻"
)]
struct Args {
    /// Run in non-interactive mode (skip confirmations)
    #[arg(short, long)]
    yes: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_logger()?;
    let args = Args::parse();

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

    let multi = MultiProgress::new();
    let steps = vec![
        (
            "Updating Homebrew",
            vec!["brew update", "brew upgrade", "brew cleanup"],
        ),
        ("Upgrading App Store apps", vec!["mas upgrade"]),
        ("Updating npm packages", vec!["npm update -g"]),
        ("Updating Composer packages", vec!["composer global update"]),
        ("Installing system updates", vec!["softwareupdate -ia"]),
        ("Updating Rust tools", vec!["cargo install-update -a"]),
        ("Updating oh-my-zsh", vec!["zsh -ic 'omz update'"]),
    ];

    for (desc, cmds) in steps {
        if !args.yes && !confirm(desc)? {
            println!("⏭️ {}", style("Skipped.").yellow());
            info!("Skipped: {}", desc);
            continue;
        }

        // Spinner for current step
        let pb = multi.add(ProgressBar::new_spinner());

        pb.set_message(format!("{}", style(format!("{}...", desc)).white()));

        pb.enable_steady_tick(Duration::from_millis(80));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green.bold} {msg}")
                .unwrap()
                .tick_strings(&[
                    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧",  // spinner frames
                    "✅", // ← shown when finished
                ]),
        );
        info!("Starting: {}", desc);

        for (i, cmd) in cmds.iter().enumerate() {
            pb.println(
                style(format!("▶️ [{} of {}] Running: {}", i + 1, cmds.len(), cmd))
                    .cyan()
                    .bold()
                    .to_string(),
            );

            if let Err(e) = run_command_with_output(cmd, &pb).await {
                pb.println(style(format!("⚠️ Error: {}", e)).red().bold().to_string());
                error!("Command `{}` failed: {:?}", cmd, e);
            }
        }

        pb.finish_with_message(style(format!("Done: {}", desc)).green().bold().to_string());
        info!("Finished: {}", desc);
        sleep(Duration::from_millis(300)).await;
    }

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

async fn run_command_with_output(cmd: &str, pb: &ProgressBar) -> Result<()> {
    let mut parts = cmd.split_whitespace();
    let bin = parts.next().context("Empty command")?;
    if which(bin).is_err() {
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

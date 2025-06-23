# Mac Updater

A sleek, modular, and automated system maintenance and update assistant for macOS, written in Rust.

## Features

- Runs and groups common update and maintenance tasks for macOS
- Interactive and quiet modes
- Progress bars and colored output
- Modular and extensible codebase

## Usage

### Run all steps interactively

```
cargo run --release -- --interactive
```

### Run in quiet mode (minimal output)

```
cargo run --release -- --quiet
```

## Requirements

- macOS
- Rust toolchain (`cargo`)
- Homebrew, mas, npm, composer, etc. (depending on which steps you want to use)

## Customization

You can add or remove steps in `src/main.rs` or extend the modular structure for your own needs.

## Configuration

The application uses a configuration file located at `~/.config/mac-updater/config.toml`. If the file does not exist, a default configuration will be used. You can customize the following settings:

### Example Configuration

```toml
# Steps to skip during execution
skip_steps = []

# Custom commands to execute
[[custom_commands]]
name = "Update Homebrew Casks"
commands = ["brew upgrade --cask"]
enabled = false

# Cleanup settings
[cleanup_settings]
downloads_days_old = 30
screenshots_days_old = 14
dmg_files_days_old = 7
clear_browser_caches = true
clear_system_logs = true

# Notification settings
[notification_settings]
enabled = true
success_only = false
include_stats = true
```

### How to Create the Configuration File

1. Create the directory if it does not exist:
   ```bash
   mkdir -p ~/.config/mac-updater
   ```
2. Create the configuration file:
   ```bash
   touch ~/.config/mac-updater/config.toml
   ```
3. Copy the example configuration above into the file and modify it as needed.

### Skippable Steps

You can configure the application to skip specific steps during execution by adding their descriptions to the `skip_steps` array in the configuration file. Below is a list of steps that can be skipped:

- Update Homebrew Casks
- Cleanup Downloads Folder
- Cleanup Screenshots Folder
- Cleanup DMG Files
- Clear Browser Caches
- Clear System Logs

To skip a step, add its description to the `skip_steps` array in your `config.toml` file. For example:

```toml
skip_steps = ["Update Homebrew Casks", "Cleanup Downloads Folder"]
```

## License

MIT

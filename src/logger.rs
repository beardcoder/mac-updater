use flexi_logger::{FileSpec, Logger, WriteMode};
use std::path::PathBuf;

pub fn init_logger() {
    let log_dir = dirs::home_dir()
        .map(|home| home.join("Library/Logs/mac-updater"))
        .unwrap_or_else(|| PathBuf::from("./logs"));

    Logger::try_with_str("info")
        .unwrap()
        .log_to_file(FileSpec::default().directory(log_dir))
        .write_mode(WriteMode::BufferAndFlush)
        .format(|w, now, record| {
            write!(
                w,
                "{} [{}] {}: {}",
                now.now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .start()
        .unwrap_or_else(|e| panic!("Logger initialization failed: {}", e));
}

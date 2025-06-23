use anyhow::Result;
use notify_rust::Notification;

pub fn send_notification(summary: &str, body: &str) -> Result<()> {
    Notification::new().summary(summary).body(body).show()?;
    Ok(())
}

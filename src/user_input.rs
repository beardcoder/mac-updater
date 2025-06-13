//! Modul für Benutzereingaben (z.B. Bestätigungsdialoge)
use anyhow::Result;
use dialoguer::Confirm;

pub fn confirm(desc: &str) -> Result<bool> {
    Ok(Confirm::new().with_prompt(desc).interact()?)
}

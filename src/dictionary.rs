use anyhow::Result;
use std::process::Command;

// Lightweight check if query is a define command
pub fn is_define_command(query: &str) -> Option<String> {
    let lower = query.to_lowercase();

    if lower.starts_with("define ") {
        return Some(query[7..].trim().to_string());
    }

    if lower.starts_with("def ") {
        return Some(query[4..].trim().to_string());
    }

    if lower.starts_with("d ") && query.len() > 2 {
        return Some(query[2..].trim().to_string());
    }

    None
}

pub fn open_dictionary(word: &str) -> Result<()> {
    Command::new("open")
        .arg(format!("dict://{}", word))
        .spawn()?;
    Ok(())
}

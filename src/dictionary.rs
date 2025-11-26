use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub word: String,
    pub definition: String,
    pub part_of_speech: Option<String>,
}

pub fn define_word(word: &str) -> Result<Vec<Definition>> {
    // Use macOS built-in dictionary via command line
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo '{}' | /usr/bin/open dict://", word))
        .output()?;

    // For now, return a simple definition structure
    // In production, we'd parse the dict:// protocol or use an API
    Ok(vec![Definition {
        word: word.to_string(),
        definition: format!("Opening dictionary for '{}'...", word),
        part_of_speech: None,
    }])
}

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

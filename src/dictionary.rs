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
    let encoded = urlencoding::encode(word);

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(format!("dict://{}", encoded))
            .spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                &format!("https://www.merriam-webster.com/dictionary/{}", encoded),
            ])
            .spawn()?;
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Command::new("xdg-open")
            .arg(format!(
                "https://www.merriam-webster.com/dictionary/{}",
                encoded
            ))
            .spawn()?;
    }

    Ok(())
}

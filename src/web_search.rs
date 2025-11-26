use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearch {
    pub query: String,
    pub engine: String,
    pub url: String,
}

pub fn search_web(query: &str, engine: Option<&str>) -> Result<WebSearch> {
    let engine = engine.unwrap_or("google");

    let url = match engine {
        "google" => format!(
            "https://www.google.com/search?q={}",
            urlencoding::encode(query)
        ),
        "duckduckgo" | "ddg" => format!("https://duckduckgo.com/?q={}", urlencoding::encode(query)),
        "bing" => format!(
            "https://www.bing.com/search?q={}",
            urlencoding::encode(query)
        ),
        "youtube" | "yt" => format!(
            "https://www.youtube.com/results?search_query={}",
            urlencoding::encode(query)
        ),
        "github" | "gh" => format!("https://github.com/search?q={}", urlencoding::encode(query)),
        _ => format!(
            "https://www.google.com/search?q={}",
            urlencoding::encode(query)
        ),
    };

    Ok(WebSearch {
        query: query.to_string(),
        engine: engine.to_string(),
        url,
    })
}

pub fn open_web_search(url: &str) -> Result<()> {
    Command::new("open").arg(url).spawn()?;
    Ok(())
}

// Check if query is a web search command
pub fn is_web_search_command(query: &str) -> Option<(String, Option<String>)> {
    let lower = query.to_lowercase();

    // "search <query>"
    if lower.starts_with("search ") {
        return Some((query[7..].trim().to_string(), None));
    }

    // "google <query>"
    if lower.starts_with("google ") {
        return Some((query[7..].trim().to_string(), Some("google".into())));
    }

    // "ddg <query>" or "duckduckgo <query>"
    if lower.starts_with("ddg ") {
        return Some((query[4..].trim().to_string(), Some("duckduckgo".into())));
    }
    if lower.starts_with("duckduckgo ") {
        return Some((query[11..].trim().to_string(), Some("duckduckgo".into())));
    }

    // "youtube <query>" or "yt <query>"
    if lower.starts_with("youtube ") {
        return Some((query[8..].trim().to_string(), Some("youtube".into())));
    }
    if lower.starts_with("yt ") {
        return Some((query[3..].trim().to_string(), Some("youtube".into())));
    }

    // "github <query>" or "gh <query>"
    if lower.starts_with("github ") {
        return Some((query[7..].trim().to_string(), Some("github".into())));
    }
    if lower.starts_with("gh ") {
        return Some((query[3..].trim().to_string(), Some("github".into())));
    }

    None
}

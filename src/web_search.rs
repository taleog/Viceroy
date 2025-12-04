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

#[cfg(test)]
mod tests {
    use super::*;

    // Test URL building for different search engines
    #[test]
    fn test_search_web_google() {
        let result = search_web("rust programming", Some("google")).unwrap();
        assert_eq!(result.engine, "google");
        assert!(result.url.contains("google.com"));
        assert!(result.url.contains("rust%20programming"));
    }

    #[test]
    fn test_search_web_duckduckgo() {
        let result = search_web("rust programming", Some("duckduckgo")).unwrap();
        assert_eq!(result.engine, "duckduckgo");
        assert!(result.url.contains("duckduckgo.com"));
    }

    #[test]
    fn test_search_web_ddg_alias() {
        let result = search_web("test query", Some("ddg")).unwrap();
        assert_eq!(result.engine, "ddg");
        assert!(result.url.contains("duckduckgo.com"));
    }

    #[test]
    fn test_search_web_bing() {
        let result = search_web("test query", Some("bing")).unwrap();
        assert_eq!(result.engine, "bing");
        assert!(result.url.contains("bing.com"));
    }

    #[test]
    fn test_search_web_youtube() {
        let result = search_web("rust tutorial", Some("youtube")).unwrap();
        assert_eq!(result.engine, "youtube");
        assert!(result.url.contains("youtube.com"));
    }

    #[test]
    fn test_search_web_youtube_alias() {
        let result = search_web("rust tutorial", Some("yt")).unwrap();
        assert_eq!(result.engine, "yt");
        assert!(result.url.contains("youtube.com"));
    }

    #[test]
    fn test_search_web_github() {
        let result = search_web("viceroy", Some("github")).unwrap();
        assert_eq!(result.engine, "github");
        assert!(result.url.contains("github.com"));
    }

    #[test]
    fn test_search_web_github_alias() {
        let result = search_web("viceroy", Some("gh")).unwrap();
        assert_eq!(result.engine, "gh");
        assert!(result.url.contains("github.com"));
    }

    #[test]
    fn test_search_web_default_engine() {
        let result = search_web("test query", None).unwrap();
        assert_eq!(result.engine, "google");
        assert!(result.url.contains("google.com"));
    }

    #[test]
    fn test_search_web_unknown_engine_defaults_to_google() {
        let result = search_web("test query", Some("unknown_engine")).unwrap();
        assert!(result.url.contains("google.com"));
    }

    // Test query encoding
    #[test]
    fn test_search_web_encodes_spaces() {
        let result = search_web("hello world", Some("google")).unwrap();
        assert!(result.url.contains("hello%20world"));
    }

    #[test]
    fn test_search_web_encodes_special_characters() {
        let result = search_web("test+query&param=value", Some("google")).unwrap();
        // URL encoding converts special chars
        assert!(result.url.contains("%"));
    }

    #[test]
    fn test_search_web_query_preserved() {
        let result = search_web("my search query", Some("google")).unwrap();
        assert_eq!(result.query, "my search query");
    }

    // Test is_web_search_command
    #[test]
    fn test_is_web_search_command_search() {
        let result = is_web_search_command("search hello world");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "hello world");
        assert!(engine.is_none());
    }

    #[test]
    fn test_is_web_search_command_google() {
        let result = is_web_search_command("google rust programming");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "rust programming");
        assert_eq!(engine, Some("google".to_string()));
    }

    #[test]
    fn test_is_web_search_command_ddg() {
        let result = is_web_search_command("ddg test query");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "test query");
        assert_eq!(engine, Some("duckduckgo".to_string()));
    }

    #[test]
    fn test_is_web_search_command_duckduckgo() {
        let result = is_web_search_command("duckduckgo test query");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "test query");
        assert_eq!(engine, Some("duckduckgo".to_string()));
    }

    #[test]
    fn test_is_web_search_command_youtube() {
        let result = is_web_search_command("youtube rust tutorial");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "rust tutorial");
        assert_eq!(engine, Some("youtube".to_string()));
    }

    #[test]
    fn test_is_web_search_command_yt() {
        let result = is_web_search_command("yt rust tutorial");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "rust tutorial");
        assert_eq!(engine, Some("youtube".to_string()));
    }

    #[test]
    fn test_is_web_search_command_github() {
        let result = is_web_search_command("github viceroy");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "viceroy");
        assert_eq!(engine, Some("github".to_string()));
    }

    #[test]
    fn test_is_web_search_command_gh() {
        let result = is_web_search_command("gh viceroy");
        assert!(result.is_some());
        let (query, engine) = result.unwrap();
        assert_eq!(query, "viceroy");
        assert_eq!(engine, Some("github".to_string()));
    }

    #[test]
    fn test_is_web_search_command_not_a_search() {
        assert!(is_web_search_command("hello world").is_none());
        assert!(is_web_search_command("safari").is_none());
        assert!(is_web_search_command("").is_none());
    }

    #[test]
    fn test_is_web_search_command_case_insensitive() {
        assert!(is_web_search_command("SEARCH test").is_some());
        assert!(is_web_search_command("Google test").is_some());
        assert!(is_web_search_command("YOUTUBE test").is_some());
    }

    // Test WebSearch struct
    #[test]
    fn test_web_search_struct() {
        let ws = WebSearch {
            query: "test".to_string(),
            engine: "google".to_string(),
            url: "https://google.com/search?q=test".to_string(),
        };
        assert_eq!(ws.query, "test");
        assert_eq!(ws.engine, "google");
        assert!(ws.url.starts_with("https://"));
    }

    #[test]
    fn test_web_search_serialization() {
        let ws = WebSearch {
            query: "test".to_string(),
            engine: "google".to_string(),
            url: "https://google.com/search?q=test".to_string(),
        };
        let json = serde_json::to_string(&ws).unwrap();
        let deserialized: WebSearch = serde_json::from_str(&json).unwrap();

        assert_eq!(ws.query, deserialized.query);
        assert_eq!(ws.engine, deserialized.engine);
        assert_eq!(ws.url, deserialized.url);
    }
}

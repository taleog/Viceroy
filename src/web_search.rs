use anyhow::Result;
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearch {
    pub query: String,
    pub engine: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkTarget {
    pub url: String,
    pub display_url: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkPreviewData {
    pub url: String,
    pub display_url: String,
    pub host: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    pub image_url: Option<String>,
    pub icon_url: Option<String>,
}

impl LinkPreviewData {
    fn from_target(target: &LinkTarget) -> Self {
        Self {
            url: target.url.clone(),
            display_url: target.display_url.clone(),
            host: target.host.clone(),
            title: None,
            description: None,
            site_name: None,
            image_url: None,
            icon_url: None,
        }
    }
}

const MAX_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_PREVIEW_IMAGE_BYTES: usize = 5 * 1024 * 1024;

fn link_preview_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(concat!(
                "Viceroy/",
                env!("CARGO_PKG_VERSION"),
                " link-preview"
            ))
            .build()
            .expect("failed to build link preview client")
    })
}

fn title_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap())
}

pub fn detect_direct_link(input: &str) -> Option<LinkTarget> {
    let trimmed = input
        .trim()
        .trim_matches(|ch| matches!(ch, '<' | '>' | '"' | '\''));
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return None;
    }

    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.starts_with("www.") || looks_like_bare_host(trimmed) {
        format!("https://{trimmed}")
    } else {
        return None;
    };

    let parsed = Url::parse(&candidate).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let host = parsed.host_str()?.to_string();
    Some(LinkTarget {
        url: parsed.to_string(),
        display_url: format_display_url(&parsed),
        host,
    })
}

pub async fn fetch_link_preview(target: &LinkTarget) -> LinkPreviewData {
    let mut preview = LinkPreviewData::from_target(target);

    let response = match link_preview_client().get(&target.url).send().await {
        Ok(response) => response,
        Err(_) => return preview,
    };

    if !response.status().is_success() {
        return preview;
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.is_empty()
        && !content_type.contains("text/html")
        && !content_type.contains("application/xhtml")
    {
        return preview;
    }
    let response_url = response.url().clone();

    let body = match response.bytes().await {
        Ok(body) => body,
        Err(_) => return preview,
    };
    let html = String::from_utf8_lossy(&body[..body.len().min(MAX_PREVIEW_BYTES)]);
    preview.title = extract_title(&html).or_else(|| {
        extract_meta_content(&html, &["og:title", "twitter:title", "application-name"])
    });
    preview.description = extract_meta_content(
        &html,
        &["description", "og:description", "twitter:description"],
    );
    preview.site_name =
        extract_meta_content(&html, &["og:site_name", "application-name", "twitter:site"]);
    preview.image_url = extract_meta_content(
        &html,
        &["og:image", "twitter:image", "twitter:image:src"],
    )
    .and_then(|value| resolve_link(value, &response_url));
    preview.icon_url = extract_icon_url(&html, &response_url);

    preview
}

pub async fn fetch_link_preview_image(image_url: &str) -> Option<Vec<u8>> {
    let response = link_preview_client().get(image_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("image/") {
        return None;
    }

    let body = response.bytes().await.ok()?;
    if body.is_empty() {
        return None;
    }
    Some(body[..body.len().min(MAX_PREVIEW_IMAGE_BYTES)].to_vec())
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
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }

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

fn looks_like_bare_host(input: &str) -> bool {
    let host = input
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('.');
    if host.is_empty() {
        return false;
    }

    let host = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim_matches(|ch| ch == '[' || ch == ']');
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if !host.contains('.') {
        return false;
    }

    let mut parts = host.split('.').peekable();
    if parts.peek().is_none() {
        return false;
    }

    let tld = host.rsplit('.').next().unwrap_or_default();
    if tld.len() < 2 || tld.len() > 24 || !tld.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }

    parts.all(|part| {
        !part.is_empty()
            && !part.starts_with('-')
            && !part.ends_with('-')
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

fn format_display_url(url: &Url) -> String {
    let mut display = url.host_str().unwrap_or_default().to_string();
    let path = url.path();
    if !path.is_empty() && path != "/" {
        display.push_str(path);
    }
    if let Some(query) = url.query() {
        display.push('?');
        display.push_str(query);
    }
    if let Some(fragment) = url.fragment() {
        display.push('#');
        display.push_str(fragment);
    }

    truncate_display(&display, 90)
}

fn resolve_link(raw: String, base: &Url) -> Option<String> {
    if let Ok(url) = Url::parse(&raw) {
        return Some(url.to_string());
    }
    base.join(&raw).ok().map(|url| url.to_string())
}

fn truncate_display(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn extract_title(html: &str) -> Option<String> {
    let capture = title_regex().captures(html)?;
    let raw = capture.get(1)?.as_str();
    clean_html_text(raw)
}

fn extract_meta_content(html: &str, names: &[&str]) -> Option<String> {
    for name in names {
        let escaped = regex::escape(name);
        let forward = format!(
            r#"(?is)<meta[^>]+(?:name|property)\s*=\s*["']{escaped}["'][^>]+content\s*=\s*["']([^"']+)["'][^>]*>"#
        );
        if let Ok(regex) = Regex::new(&forward) {
            if let Some(capture) = regex.captures(html) {
                if let Some(cleaned) = clean_html_text(capture.get(1)?.as_str()) {
                    return Some(cleaned);
                }
            }
        }

        let reverse = format!(
            r#"(?is)<meta[^>]+content\s*=\s*["']([^"']+)["'][^>]+(?:name|property)\s*=\s*["']{escaped}["'][^>]*>"#
        );
        if let Ok(regex) = Regex::new(&reverse) {
            if let Some(capture) = regex.captures(html) {
                if let Some(cleaned) = clean_html_text(capture.get(1)?.as_str()) {
                    return Some(cleaned);
                }
            }
        }
    }

    None
}

fn extract_icon_url(html: &str, base: &Url) -> Option<String> {
    let patterns = [
        r#"(?is)<link[^>]+rel\s*=\s*["'][^"']*apple-touch-icon[^"']*["'][^>]+href\s*=\s*["']([^"']+)["'][^>]*>"#,
        r#"(?is)<link[^>]+href\s*=\s*["']([^"']+)["'][^>]+rel\s*=\s*["'][^"']*apple-touch-icon[^"']*["'][^>]*>"#,
        r#"(?is)<link[^>]+rel\s*=\s*["'][^"']*icon[^"']*["'][^>]+href\s*=\s*["']([^"']+)["'][^>]*>"#,
        r#"(?is)<link[^>]+href\s*=\s*["']([^"']+)["'][^>]+rel\s*=\s*["'][^"']*icon[^"']*["'][^>]*>"#,
    ];

    for pattern in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            if let Some(capture) = regex.captures(html) {
                if let Some(raw) = capture.get(1) {
                    if let Some(resolved) = resolve_link(raw.as_str().to_string(), base) {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    base.join("/favicon.ico")
        .ok()
        .map(|url| url.to_string())
}

fn clean_html_text(raw: &str) -> Option<String> {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let decoded = collapsed
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let cleaned = decoded.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

use serde::{Deserialize, Serialize};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use crate::{app_launcher, file_search, clipboard, system_commands, calculator, emoji, dictionary, web_search};
use anyhow::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SearchMode {
    All,        // Search everything (default)
    Apps,       // Only search applications
    Files,      // Only search files
    Clipboard,  // Only search clipboard history
    Calculator, // Math & conversions
    Emoji,      // Emoji picker
    // Future modes:
    // Notes,
    // Colors,
    // Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SearchResult {
    App {
        name: String,
        path: String,
        score: i64,
        icon: Option<String>,
    },
    File {
        name: String,
        path: String,
        score: i64,
    },
    Clipboard {
        id: i64,
        content: String,
        preview: String,
        app_name: Option<String>,
        timestamp: i64,
        custom_name: Option<String>,
        is_pinned: bool,
        score: i64,
    },
    Command {
        name: String,
        description: String,
        command: String,
        score: i64,
    },
    Calculator {
        expression: String,
        result: String,
        formats: Vec<String>,
    },
    Emoji {
        emoji: String,
        name: String,
        keywords: Vec<String>,
    },
    Dictionary {
        word: String,
        preview: String,
    },
    WebSearch {
        query: String,
        engine: String,
        url: String,
    },
}

pub async fn search(query: &str) -> Result<Vec<SearchResult>> {
    search_with_mode(query, SearchMode::All).await
}

pub async fn search_with_mode(query: &str, mode: SearchMode) -> Result<Vec<SearchResult>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    
    let mut results = Vec::new();
    let matcher = SkimMatcherV2::default();
    
    // Mode-specific filtering or emoji mode
    if mode == SearchMode::Emoji || (query.starts_with(':') && query.len() > 1) {
        let emojis = emoji::search_emojis(query);
        for e in emojis {
            results.push(SearchResult::Emoji {
                emoji: e.emoji,
                name: e.name,
                keywords: e.keywords,
            });
        }
        return Ok(results);
    }
    
    // Calculator mode - try to evaluate as expression
    if mode == SearchMode::Calculator {
        if let Ok(calc_result) = calculator::evaluate(query) {
            results.push(SearchResult::Calculator {
                expression: query.to_string(),
                result: calc_result.decimal.clone(),
                formats: vec![
                    calc_result.decimal,
                    calc_result.hex,
                    calc_result.binary,
                    calc_result.percentage,
                ],
            });
        }
        return Ok(results);
    }
    
    // Check for dictionary command
    if let Some(word) = dictionary::is_define_command(query) {
        results.push(SearchResult::Dictionary {
            word: word.clone(),
            preview: format!("Define '{}'", word),
        });
        return Ok(results);
    }
    
    // Check for web search command
    if let Some((search_query, engine)) = web_search::is_web_search_command(query) {
        if let Ok(web_result) = web_search::search_web(&search_query, engine.as_deref()) {
            results.push(SearchResult::WebSearch {
                query: web_result.query,
                engine: web_result.engine,
                url: web_result.url,
            });
            return Ok(results);
        }
    }
    
    // Check if it's a calculator expression
    if is_calculator_expression(query) {
        if let Ok(calc_result) = calculator::evaluate(query) {
            results.push(SearchResult::Calculator {
                expression: query.to_string(),
                result: calc_result.decimal.clone(),
                formats: vec![
                    calc_result.decimal,
                    calc_result.hex,
                    calc_result.binary,
                    calc_result.percentage,
                ],
            });
        }
    }
    
    // Search apps (unless in clipboard-only mode)
    if mode == SearchMode::All || mode == SearchMode::Apps {
        let apps = app_launcher::search_apps(query)?;
        for app in apps {
            if let Some(score) = matcher.fuzzy_match(&app.name, query) {
                results.push(SearchResult::App {
                    name: app.name,
                    path: app.path,
                    score,
                    icon: None,
                });
            }
        }
    }
    
    // Search files (unless in apps/clipboard-only mode)
    if mode == SearchMode::All || mode == SearchMode::Files {
        let file_limit = if mode == SearchMode::Files { 50 } else { 20 };
        let files = file_search::search_files(query, file_limit)?;
        for file in files {
            if let Some(score) = matcher.fuzzy_match(&file.name, query) {
                results.push(SearchResult::File {
                    name: file.name,
                    path: file.path,
                    score,
                });
            }
        }
    }
    
    // Search clipboard (unless in apps/files-only mode)
    if mode == SearchMode::All || mode == SearchMode::Clipboard {
        let clip_limit = if mode == SearchMode::Clipboard { 50 } else { 10 };
        let clipboard_results = clipboard::search_history(query).await?;
        for entry in clipboard_results.iter().take(clip_limit) {
            let preview = entry.content.chars().take(100).collect::<String>();
            if let Some(score) = matcher.fuzzy_match(&entry.content, query) {
                results.push(SearchResult::Clipboard {
                    id: entry.id,
                    content: entry.content.clone(),
                    preview,
                    app_name: entry.app_name.clone(),
                    timestamp: entry.timestamp,
                    custom_name: entry.custom_name.clone(),
                    is_pinned: entry.is_pinned,
                    score,
                });
            }
        }
    }
    
    // Search system commands (in all mode only, unless specifically requested)
    if mode == SearchMode::All {
        let commands = system_commands::search_commands(query);
        for cmd in commands {
            if let Some(score) = matcher.fuzzy_match(&cmd.name, query) {
                results.push(SearchResult::Command {
                    name: cmd.name,
                    description: cmd.description,
                    command: cmd.command,
                    score,
                });
            }
        }
    }
    
    // Smart ranking based on query context and match quality
    results.sort_by(|a, b| {
        let score_a = get_smart_score(a, query, &matcher);
        let score_b = get_smart_score(b, query, &matcher);
        score_b.cmp(&score_a)
    });
    
    // Return top 50 results
    Ok(results.into_iter().take(50).collect())
}

fn get_smart_score(result: &SearchResult, query: &str, _matcher: &SkimMatcherV2) -> i64 {
    let query_lower = query.to_lowercase();
    let query_len = query.len();
    
    let base_score = match result {
        SearchResult::App { name, score, .. } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;
            
            // Exact match gets huge boost
            if name_lower == query_lower {
                boost += 100000;
            }
            // Starts with query gets big boost (scales with match length)
            else if name_lower.starts_with(&query_lower) {
                boost += 50000 + (query_len as i64 * 1000);
            }
            // Word boundary match gets medium boost
            else if name_lower.split_whitespace().any(|w| w.starts_with(&query_lower)) {
                boost += 25000 + (query_len as i64 * 500);
            }
            // Contains query gets small boost
            else if name_lower.contains(&query_lower) {
                boost += 10000;
            }
            
            // Common/frequently used apps get priority
            let common_apps = ["safari", "chrome", "firefox", "brave", "edge",
                              "vscode", "code", "cursor", "sublime", "atom",
                              "terminal", "iterm", "warp", "kitty",
                              "finder", "mail", "messages", "slack", "discord", "telegram",
                              "spotify", "music", "notes", "notion", "obsidian",
                              "photoshop", "figma", "sketch", "affinity"];
            if common_apps.iter().any(|app| name_lower.contains(app)) {
                boost += 8000;
            }
            
            // Productivity apps extra boost
            let productivity = ["calendar", "reminders", "todoist", "things", "omnifocus"];
            if productivity.iter().any(|app| name_lower.contains(app)) {
                boost += 6000;
            }
            
            boost
        },
        SearchResult::File { name, score, .. } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;
            
            // Exact filename match (without extension)
            let name_without_ext = name_lower.rsplit('.').nth(1).unwrap_or(&name_lower);
            if name_without_ext == query_lower || name_lower == query_lower {
                boost += 80000;
            }
            // Starts with query
            else if name_lower.starts_with(&query_lower) || 
                    name_without_ext.starts_with(&query_lower) {
                boost += 40000 + (query_len as i64 * 800);
            }
            // Contains query
            else if name_lower.contains(&query_lower) {
                boost += 15000;
            }
            
            // Prioritize source code files
            if name.ends_with(".rs") || name.ends_with(".py") || name.ends_with(".js") || 
               name.ends_with(".ts") || name.ends_with(".tsx") || name.ends_with(".jsx") {
                boost += 4000;
            }
            // Documents
            else if name.ends_with(".md") || name.ends_with(".txt") || name.ends_with(".pdf") {
                boost += 3000;
            }
            // Config files
            else if name.ends_with(".json") || name.ends_with(".toml") || 
                    name.ends_with(".yaml") || name.ends_with(".yml") {
                boost += 2500;
            }
            
            // Recently modified files get boost (if in home directory or common dev folders)
            if name_lower.contains("/documents/") || name_lower.contains("/downloads/") ||
               name_lower.contains("/desktop/") {
                boost += 2000;
            }
            
            boost - 5000 // Files slightly lower priority than apps by default
        },
        SearchResult::Clipboard { content, score, is_pinned, custom_name, .. } => {
            let content_lower = content.to_lowercase();
            let mut boost = *score;
            
            // Exact content match
            if content_lower == query_lower {
                boost += 60000;
            }
            // Starts with query
            else if content_lower.starts_with(&query_lower) {
                boost += 30000;
            }
            // Contains query (word boundary)
            else if content_lower.split_whitespace().any(|w| w.starts_with(&query_lower)) {
                boost += 20000;
            }
            // General contains
            else if content_lower.contains(&query_lower) {
                boost += 10000;
            }
            
            // Boost URLs and file paths in clipboard
            if content.starts_with("http://") || content.starts_with("https://") {
                boost += 3000;
            }
            if content.starts_with('/') || content.contains(":/") {
                boost += 2000;
            }
            
            // Pinned items get significant boost
            if *is_pinned {
                boost += 40000;
            }
            // Custom name match boosts more than raw content match
            if let Some(name) = custom_name {
                let name_lower = name.to_lowercase();
                if name_lower == query_lower {
                    boost += 50000;
                } else if name_lower.starts_with(&query_lower) {
                    boost += 25000;
                } else if name_lower.contains(&query_lower) {
                    boost += 12000;
                }
            }
            boost - 15000 // Clipboard lower priority unless very relevant
        },
        SearchResult::Command { name, score, .. } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;
            
            // Exact command match
            if name_lower == query_lower {
                boost += 90000;
            }
            // Starts with query
            else if name_lower.starts_with(&query_lower) {
                boost += 45000 + (query_len as i64 * 1000);
            }
            // Word match
            else if name_lower.split_whitespace().any(|w| w.starts_with(&query_lower)) {
                boost += 30000;
            }
            
            boost
        },
        SearchResult::Calculator { .. } => {
            // Calculator at top when it matches
            i64::MAX
        },
        SearchResult::Emoji { .. } => {
            // Emoji high priority when explicitly requested
            i64::MAX - 1
        },
        SearchResult::Dictionary { .. } => {
            // Dictionary high priority when explicitly requested
            i64::MAX - 2
        },
        SearchResult::WebSearch { .. } => {
            // Web search lowest priority - only when nothing else matches
            i64::MAX - 100000
        },
    };
    
    base_score
}

fn is_calculator_expression(query: &str) -> bool {
    // Simple heuristic: contains numbers and math operators
    let has_number = query.chars().any(|c| c.is_numeric());
    let has_operator = query.chars().any(|c| "+-*/^()".contains(c));
    has_number && has_operator
}

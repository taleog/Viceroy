use crate::{
    app_launcher, calculator, clipboard, dictionary, emoji, file_search, system_commands, usage,
    web_search,
};
use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};

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
        content_type: String,
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

    let matcher = SkimMatcherV2::default();

    // Mode-specific filtering or emoji mode
    if mode == SearchMode::Emoji || (query.starts_with(':') && query.len() > 1) {
        let emojis = emoji::search_emojis(query);
        let mut results = Vec::new();
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
            return Ok(vec![SearchResult::Calculator {
                expression: query.to_string(),
                result: calc_result.decimal.clone(),
                formats: vec![
                    calc_result.decimal,
                    calc_result.hex,
                    calc_result.binary,
                    calc_result.percentage,
                ],
            }]);
        }
        return Ok(Vec::new());
    }

    // Check for dictionary command
    if let Some(word) = dictionary::is_define_command(query) {
        return Ok(vec![SearchResult::Dictionary {
            word: word.clone(),
            preview: format!("Define '{}'", word),
        }]);
    }

    // Check for web search command
    if let Some((search_query, engine)) = web_search::is_web_search_command(query) {
        if let Ok(web_result) = web_search::search_web(&search_query, engine.as_deref()) {
            return Ok(vec![SearchResult::WebSearch {
                query: web_result.query,
                engine: web_result.engine,
                url: web_result.url,
            }]);
        }
    }

    // Prepare futures for parallel execution
    let query_clone = query.to_string();
    let mode_clone = mode;

    // App search future
    let app_future = async {
        let mut results = Vec::new();
        if mode_clone == SearchMode::All || mode_clone == SearchMode::Apps {
            if let Ok(apps) = app_launcher::search_apps(&query_clone) {
                let matcher = SkimMatcherV2::default();
                for app in apps {
                    if let Some(score) = matcher.fuzzy_match(&app.name, &query_clone) {
                        results.push(SearchResult::App {
                            name: app.name,
                            path: app.path,
                            score,
                            icon: None,
                        });
                    }
                }
            }
        }
        results
    };

    // File search future
    let file_future = async {
        let mut results = Vec::new();
        if mode_clone == SearchMode::All || mode_clone == SearchMode::Files {
            // Skip file search for very short queries to improve responsiveness
            if query_clone.len() >= 3 {
                let file_limit = if mode_clone == SearchMode::Files {
                    50
                } else {
                    20
                };
                // Use spawn_blocking for mdfind as it's a blocking IO operation
                let query_inner = query_clone.clone();
                let files_result = tokio::task::spawn_blocking(move || {
                    file_search::search_files(&query_inner, file_limit)
                })
                .await;

                if let Ok(Ok(files)) = files_result {
                    let matcher = SkimMatcherV2::default();
                    for file in files {
                        if let Some(score) = matcher.fuzzy_match(&file.name, &query_clone) {
                            results.push(SearchResult::File {
                                name: file.name,
                                path: file.path,
                                score,
                            });
                        }
                    }
                }
            }
        }
        results
    };

    // Clipboard search future
    let clipboard_future = async {
        let mut results = Vec::new();
        if mode_clone == SearchMode::All || mode_clone == SearchMode::Clipboard {
            let clip_limit = if mode_clone == SearchMode::Clipboard {
                50
            } else {
                10
            };
            if let Ok(clipboard_results) = clipboard::search_history(&query_clone).await {
                let matcher = SkimMatcherV2::default();
                for entry in clipboard_results.iter().take(clip_limit) {
                    let preview = entry.content.chars().take(100).collect::<String>();
                    if let Some(score) = matcher.fuzzy_match(&entry.content, &query_clone) {
                        results.push(SearchResult::Clipboard {
                            id: entry.id,
                            content: entry.content.clone(),
                            preview,
                            content_type: entry.content_type.clone(),
                            app_name: entry.app_name.clone(),
                            timestamp: entry.timestamp,
                            custom_name: entry.custom_name.clone(),
                            is_pinned: entry.is_pinned,
                            score,
                        });
                    }
                }
            }
        }
        results
    };

    // Command search future (fast, can run inline or async)
    let command_future = async {
        let mut results = Vec::new();
        if mode_clone == SearchMode::All {
            let commands = system_commands::search_commands(&query_clone);
            let matcher = SkimMatcherV2::default();
            for cmd in commands {
                if let Some(score) = matcher.fuzzy_match(&cmd.name, &query_clone) {
                    results.push(SearchResult::Command {
                        name: cmd.name,
                        description: cmd.description,
                        command: cmd.command,
                        score,
                    });
                }
            }
        }
        results
    };

    // Calculator check (fast)
    let calc_future = async {
        let mut results = Vec::new();
        if is_calculator_expression(&query_clone) {
            if let Ok(calc_result) = calculator::evaluate(&query_clone) {
                results.push(SearchResult::Calculator {
                    expression: query_clone.clone(),
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
        results
    };

    // Execute all searches in parallel
    let (mut app_results, mut file_results, mut clip_results, mut cmd_results, mut calc_results) = tokio::join!(
        app_future,
        file_future,
        clipboard_future,
        command_future,
        calc_future
    );

    // Detect query intent
    let query_lower = query.to_lowercase();
    let is_file_query = query.contains('.')
        && (query.contains('/')
            || query_lower.ends_with(".txt")
            || query_lower.ends_with(".md")
            || query_lower.ends_with(".pdf")
            || query_lower.ends_with(".rs")
            || query_lower.ends_with(".js")
            || query_lower.ends_with(".py"));
    let is_url =
        query.starts_with("http://") || query.starts_with("https://") || query.contains("://");
    let is_short_query = query.len() <= 3;

    // Cap results per category to ensure diversity
    let max_per_category = if mode == SearchMode::All {
        if is_short_query {
            (10, 3, 2) // (apps, files, clipboard) - apps dominate for short queries
        } else if is_file_query {
            (5, 15, 3) // files dominate for file queries
        } else if is_url {
            (3, 3, 10) // clipboard dominates for URLs
        } else {
            (8, 8, 5) // balanced
        }
    } else {
        (50, 50, 50) // no limits in specific modes
    };

    app_results.truncate(max_per_category.0);
    file_results.truncate(max_per_category.1);
    clip_results.truncate(max_per_category.2);

    // Combine results, de-duplicating .app bundles that already exist as apps
    let mut results = Vec::new();

    // Index app bundle paths for quick lookup
    use std::collections::HashSet;
    let app_paths: HashSet<String> = app_results
        .iter()
        .filter_map(|r| {
            if let SearchResult::App { path, .. } = r {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();

    // Keep all app results first
    results.append(&mut app_results);

    // Filter file results: drop .app bundles that correspond to an App result
    let mut filtered_files = Vec::new();
    for f in file_results.into_iter() {
        if let SearchResult::File { ref path, .. } = f {
            if path.ends_with(".app") && app_paths.contains(path) {
                // Skip duplicate .app file entry
                continue;
            }
        }
        filtered_files.push(f);
    }
    results.append(&mut filtered_files);
    results.append(&mut clip_results);
    results.append(&mut cmd_results);
    results.append(&mut calc_results);

    // Smart ranking based on query context and match quality
    let query_context = QueryContext {
        is_file_query,
        is_url,
        is_short_query,
        length: query.len(),
    };

    results.sort_by(|a, b| {
        let score_a = get_smart_score(a, query, &matcher, &query_context);
        let score_b = get_smart_score(b, query, &matcher, &query_context);
        score_b.cmp(&score_a)
    });

    // Return top 50 results
    Ok(results.into_iter().take(50).collect())
}

struct QueryContext {
    is_file_query: bool,
    is_url: bool,
    is_short_query: bool,
    length: usize,
}

fn get_smart_score(
    result: &SearchResult,
    query: &str,
    _matcher: &SkimMatcherV2,
    context: &QueryContext,
) -> i64 {
    let query_lower = query.to_lowercase();
    let query_len = query.len();

    let base_score = match result {
        SearchResult::App {
            name, path, score, ..
        } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;

            // Context-aware boost: short queries heavily favor apps
            if context.is_short_query {
                boost += 35000; // Massive boost for short queries (1-3 chars)
            } else if context.is_file_query {
                boost -= 20000; // Reduce apps when clearly searching for a file
            }

            // Exact match gets huge boost
            if name_lower == query_lower {
                boost += 100000;
            }
            // Starts with query gets big boost (scales with match length)
            else if name_lower.starts_with(&query_lower) {
                boost += 50000 + (query_len as i64 * 1000);
            }
            // Word boundary match gets medium boost
            else if name_lower
                .split_whitespace()
                .any(|w| w.starts_with(&query_lower))
            {
                boost += 25000 + (query_len as i64 * 500);
            }
            // Contains query gets small boost
            else if name_lower.contains(&query_lower) {
                boost += 10000;
            }

            // Common/frequently used apps get priority
            let common_apps = [
                "safari",
                "chrome",
                "firefox",
                "brave",
                "edge",
                "vscode",
                "code",
                "cursor",
                "sublime",
                "atom",
                "terminal",
                "iterm",
                "warp",
                "kitty",
                "finder",
                "mail",
                "messages",
                "slack",
                "discord",
                "telegram",
                "spotify",
                "music",
                "notes",
                "notion",
                "obsidian",
                "photoshop",
                "figma",
                "sketch",
                "affinity",
            ];
            if common_apps.iter().any(|app| name_lower.contains(app)) {
                boost += 8000;
            }

            // Productivity apps extra boost
            let productivity = ["calendar", "reminders", "todoist", "things", "omnifocus"];
            if productivity.iter().any(|app| name_lower.contains(app)) {
                boost += 6000;
            }

            // Personal usage-based boost: recently and frequently launched apps
            if let Some((last_used, launch_count)) = usage::get_app_usage(path) {
                let now = chrono::Utc::now().timestamp();
                let age = now.saturating_sub(last_used).max(0);

                // Recency tiers (in seconds)
                let day: i64 = 86_400;
                let week: i64 = day * 7;

                if age <= 10 * 60 {
                    // Used in last 10 minutes
                    boost += 40_000;
                } else if age <= day {
                    // Used today
                    boost += 25_000;
                } else if age <= week {
                    // Used this week
                    boost += 15_000;
                } else if age <= 30 * day {
                    // Used this month
                    boost += 5_000;
                }

                // Frequency boost (log-like: diminishing returns)
                let freq_boost = (launch_count as i64).min(50) * 400;
                boost += freq_boost;
            }

            boost
        }
        SearchResult::File {
            name, path, score, ..
        } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;

            // Context-aware boost: file queries heavily favor files
            if context.is_file_query {
                boost += 40000; // Massive boost when query looks like a file
            } else if context.is_url {
                boost -= 25000; // Reduce files for URL queries
            } else if context.is_short_query {
                boost -= 15000; // Reduce files for very short queries (favor apps)
            }

            // Exact filename match (without extension). Use rsplit_once to avoid
            // relying on positional indices that can be brittle.
            let name_without_ext = match name_lower.rsplit_once('.') {
                Some((stem, _)) => stem,
                None => name_lower.as_str(),
            };
            if name_without_ext == query_lower || name_lower == query_lower {
                boost += 80000;
            }
            // Starts with query
            else if name_lower.starts_with(&query_lower)
                || name_without_ext.starts_with(&query_lower)
            {
                boost += 40000 + (query_len as i64 * 800);
            }
            // Contains query
            else if name_lower.contains(&query_lower) {
                boost += 15000;
            }

            // Prioritize source code files
            if name.ends_with(".rs")
                || name.ends_with(".py")
                || name.ends_with(".js")
                || name.ends_with(".ts")
                || name.ends_with(".tsx")
                || name.ends_with(".jsx")
            {
                boost += 4000;
            }
            // Documents
            else if name.ends_with(".md") || name.ends_with(".txt") || name.ends_with(".pdf") {
                boost += 3000;
            }
            // Config files
            else if name.ends_with(".json")
                || name.ends_with(".toml")
                || name.ends_with(".yaml")
                || name.ends_with(".yml")
            {
                boost += 2500;
            }

            // Path-based boosts/penalties
            let path_lower = path.to_lowercase();
            // Strongly downweight noisy system/framework files
            if path_lower.starts_with("/system/library/") || path_lower.contains(".framework/") {
                boost -= 30000;
            }

            // Boost common user locations
            if path_lower.contains("/users/")
                && (path_lower.contains("/documents/")
                    || path_lower.contains("/downloads/")
                    || path_lower.contains("/desktop/"))
            {
                boost += 8000;
            }

            boost - 5000 // Files slightly lower priority than apps by default
        }
        SearchResult::Clipboard {
            content,
            score,
            is_pinned,
            custom_name,
            ..
        } => {
            let content_lower = content.to_lowercase();
            let mut boost = *score;

            // Context-aware boost: URLs and longer queries favor clipboard
            if context.is_url {
                boost += 45000; // Massive boost for URL queries
            } else if context.is_short_query {
                boost -= 30000; // Heavy penalty for short queries (favor apps)
            } else if context.length > 10 {
                boost += 15000; // Longer queries likely searching clipboard
            }

            // Exact content match
            if content_lower == query_lower {
                boost += 60000;
            }
            // Starts with query
            else if content_lower.starts_with(&query_lower) {
                boost += 30000;
            }
            // Contains query (word boundary)
            else if content_lower
                .split_whitespace()
                .any(|w| w.starts_with(&query_lower))
            {
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
        }
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
            else if name_lower
                .split_whitespace()
                .any(|w| w.starts_with(&query_lower))
            {
                boost += 30000;
            }

            boost
        }
        SearchResult::Calculator { .. } => {
            // Calculator at top when it matches
            i64::MAX
        }
        SearchResult::Emoji { .. } => {
            // Emoji high priority when explicitly requested
            i64::MAX - 1
        }
        SearchResult::Dictionary { .. } => {
            // Dictionary high priority when explicitly requested
            i64::MAX - 2
        }
        SearchResult::WebSearch { .. } => {
            // Web search lowest priority - only when nothing else matches
            i64::MAX - 100000
        }
    };

    base_score
}

fn is_calculator_expression(query: &str) -> bool {
    // Simple heuristic: contains numbers and math operators
    let has_number = query.chars().any(|c| c.is_numeric());
    let has_operator = query.chars().any(|c| "+-*/^()".contains(c));
    has_number && has_operator
}

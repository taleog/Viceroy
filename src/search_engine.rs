use crate::{
    app_launcher, calculator, clipboard, dictionary, emoji, file_search, system_commands, usage,
    web_search,
};
use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::time::Instant;

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
        image_width: Option<i64>,
        image_height: Option<i64>,
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
    run_search(query, mode, true, true).await
}

pub async fn search_fast(query: &str) -> Result<Vec<SearchResult>> {
    run_search(query, SearchMode::All, false, false).await
}

async fn run_search(
    query: &str,
    mode: SearchMode,
    include_files: bool,
    include_clipboard: bool,
) -> Result<Vec<SearchResult>> {
    let start_time = Instant::now();
    
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
        if include_files && (mode_clone == SearchMode::All || mode_clone == SearchMode::Files) {
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
        if include_clipboard
            && (mode_clone == SearchMode::All || mode_clone == SearchMode::Clipboard)
        {
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
                            image_width: entry.image_width,
                            image_height: entry.image_height,
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
    let (app_results, file_results, clip_results, cmd_results, calc_results) = tokio::join!(
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

    let query_context = QueryContext {
        is_file_query,
        is_url,
        is_short_query,
        length: query.len(),
    };

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

    // De-duplicate .app bundles that already exist as apps
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

    // Filter file results: drop .app bundles that correspond to an App result
    let mut file_results = file_results;
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
    file_results = filtered_files;

    // Score once per result to keep ranking stable and lightweight
    let mut scored_apps = score_and_sort(app_results, query, &matcher, &query_context);
    let mut scored_files = score_and_sort(file_results, query, &matcher, &query_context);
    let mut scored_clips = score_and_sort(clip_results, query, &matcher, &query_context);
    let mut scored_cmds = score_and_sort(cmd_results, query, &matcher, &query_context);
    let mut scored_calcs = score_and_sort(calc_results, query, &matcher, &query_context);

    truncate_scored(&mut scored_apps, max_per_category.0);
    truncate_scored(&mut scored_files, max_per_category.1);
    truncate_scored(&mut scored_clips, max_per_category.2);

    // Combine scored results
    let mut results: Vec<(i64, SearchResult)> = Vec::new();
    results.append(&mut scored_apps);
    results.append(&mut scored_files);
    results.append(&mut scored_clips);
    results.append(&mut scored_cmds);
    results.append(&mut scored_calcs);

    // Smart ranking based on query context and match quality
    results.sort_by(|a, b| b.0.cmp(&a.0));

    // Log search latency
    let elapsed = start_time.elapsed();
    log::info!(
        "search: query='{}' mode={:?} results={} elapsed={:.1}ms",
        query,
        mode,
        results.len(),
        elapsed.as_secs_f64() * 1000.0
    );

    // Return top 50 results
    Ok(results
        .into_iter()
        .take(50)
        .map(|(_, result)| result)
        .collect())
}

struct QueryContext {
    is_file_query: bool,
    is_url: bool,
    is_short_query: bool,
    length: usize,
}

fn score_and_sort(
    results: Vec<SearchResult>,
    query: &str,
    matcher: &SkimMatcherV2,
    context: &QueryContext,
) -> Vec<(i64, SearchResult)> {
    let mut scored: Vec<(i64, SearchResult)> = results
        .into_iter()
        .map(|result| {
            let score = get_smart_score(&result, query, matcher, context);
            (score, result)
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
}

fn truncate_scored(results: &mut Vec<(i64, SearchResult)>, limit: usize) {
    if results.len() > limit {
        results.truncate(limit);
    }
}

#[derive(Clone, Copy)]
struct MatchWeights {
    exact: i64,
    starts_base: i64,
    starts_per_char: i64,
    word_base: i64,
    word_per_char: i64,
    contains: i64,
}

impl MatchWeights {
    const fn new(
        exact: i64,
        starts_base: i64,
        starts_per_char: i64,
        word_base: i64,
        word_per_char: i64,
        contains: i64,
    ) -> Self {
        Self {
            exact,
            starts_base,
            starts_per_char,
            word_base,
            word_per_char,
            contains,
        }
    }
}

const APP_MATCH_WEIGHTS: MatchWeights =
    MatchWeights::new(100_000, 50_000, 1_000, 25_000, 500, 10_000);
const FILE_MATCH_WEIGHTS: MatchWeights = MatchWeights::new(80_000, 40_000, 800, 0, 0, 15_000);
const CLIPBOARD_MATCH_WEIGHTS: MatchWeights =
    MatchWeights::new(60_000, 30_000, 0, 20_000, 0, 10_000);
const COMMAND_MATCH_WEIGHTS: MatchWeights = MatchWeights::new(90_000, 45_000, 1_000, 30_000, 0, 0);

fn match_score(
    primary: &str,
    alternate: Option<&str>,
    query_lower: &str,
    query_len: usize,
    weights: &MatchWeights,
) -> i64 {
    if primary == query_lower || alternate.map(|alt| alt == query_lower).unwrap_or(false) {
        return weights.exact;
    }

    if primary.starts_with(query_lower)
        || alternate
            .map(|alt| alt.starts_with(query_lower))
            .unwrap_or(false)
    {
        return weights.starts_base + (query_len as i64 * weights.starts_per_char);
    }

    if weights.word_base > 0
        && primary
            .split_whitespace()
            .any(|w| w.starts_with(query_lower))
    {
        return weights.word_base + (query_len as i64 * weights.word_per_char);
    }

    if weights.contains > 0 && primary.contains(query_lower) {
        return weights.contains;
    }

    0
}

/// Compute a context-aware score that keeps better-matching and more relevant
/// results higher in the table without changing the relative ordering inside
/// each category. The heuristics favor short app queries, file-like queries,
/// URLs in clipboard, and explicit calculator/emoji/dictionary/web matches.
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
            boost += match_score(
                &name_lower,
                None,
                &query_lower,
                query_len,
                &APP_MATCH_WEIGHTS,
            );

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

            let name_without_ext = match name_lower.rsplit_once('.') {
                Some((stem, _)) => stem,
                None => name_lower.as_str(),
            };
            boost += match_score(
                &name_lower,
                Some(name_without_ext),
                &query_lower,
                query_len,
                &FILE_MATCH_WEIGHTS,
            );

            // Context-aware boost: file queries heavily favor files
            if context.is_file_query {
                boost += 40000; // Massive boost when query looks like a file
            } else if context.is_url {
                boost -= 25000; // Reduce files for URL queries
            } else if context.is_short_query {
                boost -= 15000; // Reduce files for very short queries (favor apps)
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
            timestamp,
            ..
        } => {
            let content_lower = content.to_lowercase();
            let mut boost = *score;
            boost += match_score(
                &content_lower,
                None,
                &query_lower,
                query_len,
                &CLIPBOARD_MATCH_WEIGHTS,
            );

            // Context-aware boost: URLs and longer queries favor clipboard
            if context.is_url {
                boost += 45000; // Massive boost for URL queries
            } else if context.is_short_query {
                boost -= 30000; // Heavy penalty for short queries (favor apps)
            } else if context.length > 10 {
                boost += 15000; // Longer queries likely searching clipboard
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
                let name_boost = match_score(
                    &name_lower,
                    None,
                    &query_lower,
                    query_len,
                    &MatchWeights::new(50_000, 25_000, 0, 0, 0, 12_000),
                );
                boost += name_boost;
            }

            // Recency boost so the freshest snippets surface first
            let now = chrono::Utc::now().timestamp();
            let age_seconds = now.saturating_sub(*timestamp).max(0);
            if age_seconds <= 120 {
                boost += 18000;
            } else if age_seconds <= 3600 {
                boost += 12000;
            } else if age_seconds <= 86_400 {
                boost += 7000;
            } else if age_seconds <= 3 * 86_400 {
                boost += 3500;
            }

            boost - 15000 // Clipboard lower priority unless very relevant
        }
        SearchResult::Command { name, score, .. } => {
            let name_lower = name.to_lowercase();
            let mut boost = *score;
            boost += match_score(
                &name_lower,
                None,
                &query_lower,
                query_len,
                &COMMAND_MATCH_WEIGHTS,
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn context_for(
        query: &str,
        is_file_query: bool,
        is_url: bool,
        is_short_query: bool,
    ) -> QueryContext {
        QueryContext {
            is_file_query,
            is_url,
            is_short_query,
            length: query.len(),
        }
    }

    #[test]
    fn short_queries_prefer_apps() {
        let matcher = SkimMatcherV2::default();
        let query = "sa";
        let ctx = context_for(query, false, false, true);
        let app = SearchResult::App {
            name: "Safari".to_string(),
            path: "/Applications/Safari.app".to_string(),
            score: 10,
            icon: None,
        };
        let file = SearchResult::File {
            name: "safari.txt".to_string(),
            path: "/tmp/safari.txt".to_string(),
            score: 5,
        };

        let app_score = get_smart_score(&app, query, &matcher, &ctx);
        let file_score = get_smart_score(&file, query, &matcher, &ctx);
        assert!(app_score > file_score);
    }

    #[test]
    fn file_queries_prefer_files() {
        let matcher = SkimMatcherV2::default();
        let query = "report.pdf";
        let ctx = context_for(query, true, false, false);
        let app = SearchResult::App {
            name: "Report Viewer".to_string(),
            path: "/Applications/Report.app".to_string(),
            score: 100,
            icon: None,
        };
        let file = SearchResult::File {
            name: "report.pdf".to_string(),
            path: "/Users/test/Documents/report.pdf".to_string(),
            score: 50,
        };

        let app_score = get_smart_score(&app, query, &matcher, &ctx);
        let file_score = get_smart_score(&file, query, &matcher, &ctx);
        assert!(file_score > app_score);
    }

    #[test]
    fn url_queries_favor_clipboard_entries() {
        let matcher = SkimMatcherV2::default();
        let query = "https://example.com";
        let ctx = context_for(query, false, true, false);
        let clipboard = SearchResult::Clipboard {
            id: 1,
            content: "https://example.com/page".to_string(),
            preview: "Example".to_string(),
            content_type: "text".to_string(),
            app_name: Some("Safari".to_string()),
            timestamp: 0,
            custom_name: None,
            is_pinned: false,
            image_width: None,
            image_height: None,
            score: 0,
        };
        let command = SearchResult::Command {
            name: "open".to_string(),
            description: "Open URL".to_string(),
            command: "open".to_string(),
            score: 0,
        };

        let clip_score = get_smart_score(&clipboard, query, &matcher, &ctx);
        let cmd_score = get_smart_score(&command, query, &matcher, &ctx);
        assert!(clip_score > cmd_score);
    }
}

use crate::{
    app_launcher, calculator, clipboard, dictionary, emoji, file_search, obsidian, settings,
    system_commands, usage, web_search,
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
    Link {
        url: String,
        display_url: String,
        host: String,
    },
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
    Note {
        title: String,
        path: String,
        relative_path: String,
        vault_name: Option<String>,
        score: i64,
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

    let direct_link = web_search::detect_direct_link(query);

    let matcher = SkimMatcherV2::default().ignore_case();

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
                let matcher = SkimMatcherV2::default().ignore_case();
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
                    let matcher = SkimMatcherV2::default().ignore_case();
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
                let matcher = SkimMatcherV2::default().ignore_case();
                for entry in clipboard_results.iter().take(clip_limit) {
                    let is_image = entry.content_type == "image";
                    let custom_name = entry
                        .custom_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|name| !name.is_empty());
                    if is_image && custom_name.is_none() {
                        continue;
                    }

                    let match_target = if is_image {
                        custom_name.unwrap()
                    } else {
                        entry.content.as_str()
                    };

                    if let Some(score) = matcher.fuzzy_match(match_target, &query_clone) {
                        let preview = if is_image {
                            custom_name.unwrap().to_string()
                        } else {
                            entry.content.chars().take(100).collect::<String>()
                        };
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

    // Note search future
    let note_future = async {
        let mut results = Vec::new();
        if mode_clone == SearchMode::All || mode_clone == SearchMode::Files {
            if let Ok(app_settings) = settings::load() {
                if app_settings.obsidian.enabled {
                    if let Some(vault_path) = app_settings.obsidian.vault_path.clone() {
                        let vault_name = app_settings.obsidian.vault_name.clone();
                        let query_inner = query_clone.clone();
                        let notes_result = tokio::task::spawn_blocking(move || {
                            obsidian::search_notes(&query_inner, &vault_path, vault_name.as_deref())
                        })
                        .await;

                        if let Ok(Ok(notes)) = notes_result {
                            let matcher = SkimMatcherV2::default().ignore_case();
                            for note in notes {
                                if let Some(score) = matcher
                                    .fuzzy_match(&note.title, &query_clone)
                                    .or_else(|| matcher.fuzzy_match(&note.relative_path, &query_clone))
                                {
                                    results.push(SearchResult::Note {
                                        title: note.title,
                                        path: note.path,
                                        relative_path: note.relative_path,
                                        vault_name: note.vault_name,
                                        score,
                                    });
                                }
                            }
                        }
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
            let matcher = SkimMatcherV2::default().ignore_case();
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
    let (app_results, file_results, note_results, clip_results, cmd_results, calc_results) = tokio::join!(
        app_future,
        file_future,
        note_future,
        clipboard_future,
        command_future,
        calc_future
    );

    // Detect query intent
    let query_context = QueryContext::from_query(query, direct_link.is_some());

    // Cap results per category to ensure diversity
    let max_per_category = if mode == SearchMode::All {
        if query_context.is_short_query {
            (10, 3, 2) // (apps, files, clipboard) - apps dominate for short queries
        } else if query_context.is_file_query {
            (5, 15, 3) // files dominate for file queries
        } else if query_context.is_url {
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
    let mut scored_notes = score_and_sort(note_results, query, &matcher, &query_context);
    let mut scored_clips = score_and_sort(clip_results, query, &matcher, &query_context);
    let mut scored_cmds = score_and_sort(cmd_results, query, &matcher, &query_context);
    let mut scored_calcs = score_and_sort(calc_results, query, &matcher, &query_context);
    let mut scored_links = direct_link
        .into_iter()
        .map(|link| SearchResult::Link {
            url: link.url,
            display_url: link.display_url,
            host: link.host,
        })
        .map(|result| {
            (
                get_smart_score(&result, query, &matcher, &query_context),
                result,
            )
        })
        .collect::<Vec<_>>();

    truncate_scored(&mut scored_apps, max_per_category.0);
    truncate_scored(&mut scored_files, max_per_category.1);
    truncate_scored(&mut scored_notes, max_per_category.1);
    truncate_scored(&mut scored_clips, max_per_category.2);

    // Combine scored results
    let mut results: Vec<(i64, SearchResult)> = Vec::new();
    results.append(&mut scored_apps);
    results.append(&mut scored_files);
    results.append(&mut scored_notes);
    results.append(&mut scored_clips);
    results.append(&mut scored_cmds);
    results.append(&mut scored_calcs);
    results.append(&mut scored_links);

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

impl QueryContext {
    fn from_query(query: &str, is_url: bool) -> Self {
        let trimmed = query.trim();
        let trimmed_lower = trimmed.to_lowercase();
        let length = trimmed.chars().count();

        Self {
            is_file_query: looks_like_file_query(trimmed, &trimmed_lower),
            is_url,
            is_short_query: length <= 3,
            length,
        }
    }
}

fn looks_like_file_query(query: &str, _query_lower: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    if query.contains('/') || query.contains('\\') || query.starts_with('~') {
        return true;
    }

    let last_segment = query
        .rsplit(|ch| ch == '/' || ch == '\\')
        .next()
        .unwrap_or(query)
        .trim();
    if last_segment.is_empty() {
        return false;
    }

    let lower_segment = last_segment.to_lowercase();
    let stem = lower_segment
        .rsplit_once('.')
        .map(|(name, _)| name)
        .unwrap_or(lower_segment.as_str());

    if lower_segment.starts_with('.') && !lower_segment[1..].contains('.') {
        return true;
    }

    if let Some((name, ext)) = lower_segment.rsplit_once('.') {
        let ext_len = ext.len();
        if !name.is_empty()
            && (1..=8).contains(&ext_len)
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
            && !matches!(
                ext,
                "com"
                    | "net"
                    | "org"
                    | "dev"
                    | "app"
                    | "ai"
                    | "gg"
                    | "io"
                    | "me"
                    | "ca"
                    | "uk"
                    | "co"
            )
        {
            return true;
        }
    }

    let file_hints = [
        "cargo.toml",
        "cargo.lock",
        "package.json",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "dockerfile",
        "compose.yml",
        "compose.yaml",
        "docker-compose.yml",
        "docker-compose.yaml",
        "makefile",
        "readme",
        "readme.md",
        ".env",
        ".gitignore",
        ".zshrc",
        ".bashrc",
        ".bash_profile",
        "tsconfig",
        "tsconfig.json",
    ];

    file_hints.contains(&lower_segment.as_str()) || file_hints.contains(&stem)
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
        SearchResult::Link {
            url,
            display_url,
            host,
        } => {
            let mut boost = i64::MAX - 10_000;
            if context.is_url {
                boost += 5_000;
            }
            let query_lower = query_lower.trim();
            if url.eq_ignore_ascii_case(query) || display_url.eq_ignore_ascii_case(query) {
                boost += 2_500;
            } else if host.eq_ignore_ascii_case(query_lower) {
                boost += 2_000;
            }
            boost
        }
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
        SearchResult::Note {
            title,
            relative_path,
            score,
            ..
        } => {
            let title_lower = title.to_lowercase();
            let relative_lower = relative_path.to_lowercase();
            let mut boost = *score;

            if title_lower == query_lower {
                boost += 95_000;
            } else if title_lower.starts_with(&query_lower) {
                boost += 48_000 + (query_len as i64 * 900);
            } else if title_lower.contains(&query_lower) {
                boost += 16_000;
            } else if relative_lower.contains(&query_lower) {
                boost += 8_000;
            }

            if relative_lower.starts_with("projects/")
                || relative_lower.starts_with("inbox/")
                || relative_lower.starts_with("daily/")
                || relative_lower.starts_with("ideas/")
            {
                boost += 4_000;
            }

            if context.is_short_query {
                boost -= 5_000;
            }

            boost - 2_000
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
            length: query.chars().count(),
        }
    }

    #[test]
    fn short_queries_prefer_apps() {
        let matcher = SkimMatcherV2::default().ignore_case();
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
        let matcher = SkimMatcherV2::default().ignore_case();
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
        let matcher = SkimMatcherV2::default().ignore_case();
        let query = "https://example.com";
        let ctx = context_for(query, false, true, false);
        let link = SearchResult::Link {
            url: "https://example.com/".to_string(),
            display_url: "example.com".to_string(),
            host: "example.com".to_string(),
        };
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
        let link_score = get_smart_score(&link, query, &matcher, &ctx);
        assert!(clip_score > cmd_score);
        assert!(link_score > clip_score);
    }

    #[test]
    fn bare_domains_are_recognized_as_links() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let results = runtime.block_on(search("example.com")).unwrap();
        assert!(results.iter().any(|result| matches!(
            result,
            SearchResult::Link { host, .. } if host == "example.com"
        )));
    }

    #[test]
    fn query_context_marks_common_file_names_as_file_queries() {
        let ctx = QueryContext::from_query("Cargo.toml", false);
        assert!(ctx.is_file_query);
        assert!(!ctx.is_url);
        assert!(!ctx.is_short_query);
    }

    #[test]
    fn query_context_marks_hidden_dotfiles_as_file_queries() {
        let ctx = QueryContext::from_query(".env", false);
        assert!(ctx.is_file_query);
    }

    #[test]
    fn query_context_marks_paths_as_file_queries() {
        let ctx = QueryContext::from_query("src/search_engine.rs", false);
        assert!(ctx.is_file_query);
    }

    #[test]
    fn query_context_does_not_treat_domains_as_file_queries() {
        let ctx = QueryContext::from_query("example.com", true);
        assert!(!ctx.is_file_query);
        assert!(ctx.is_url);
    }
}

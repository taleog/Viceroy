use anyhow::Result;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
}

struct FileCacheEntry {
    query: String,
    results: Vec<FileInfo>,
    timestamp: Instant,
}

lazy_static! {
    static ref FILE_SEARCH_CACHE: Mutex<Option<FileCacheEntry>> = Mutex::new(None);
    static ref FALLBACK_INDEX_CACHE: Mutex<Option<FileCacheEntry>> = Mutex::new(None);
}

const CACHE_TTL: Duration = Duration::from_millis(1500);
const CACHE_STORE_LIMIT: usize = 200;
const FALLBACK_MAX_DEPTH: usize = 6;
const FALLBACK_SCAN_LIMIT: usize = 600;
const FALLBACK_INDEX_LIMIT: usize = 4000;
const FALLBACK_INDEX_TTL: Duration = Duration::from_secs(45);
const FALLBACK_ENV: &str = "VICEROY_FALLBACK_FS";

pub fn search_files(query: &str, limit: usize) -> Result<Vec<FileInfo>> {
    if let Some(mut cached) = try_cached_results(query) {
        cached.truncate(limit);
        return Ok(cached);
    }

    // File search disabled by default to avoid privacy permission prompts
    // Users must explicitly enable with VICEROY_FALLBACK_FS=1 (which enables fallback indexing)
    // Spotlight is broken on many systems so we don't bother with mdfind either
    if !fallback_enabled() {
        return Ok(Vec::new());
    }

    // Only fallback walk if explicitly enabled
    let mut files = fallback_walk(query, CACHE_STORE_LIMIT);

    // Nothing found
    if files.is_empty() {
        return Ok(files);
    }

    // Cache and trim
    if files.len() > CACHE_STORE_LIMIT {
        files.truncate(CACHE_STORE_LIMIT);
    }
    update_cache(query, &files);
    files.truncate(limit);

    Ok(files)
}

fn fallback_walk(query: &str, limit: usize) -> Vec<FileInfo> {
    let index = get_fallback_index();
    let query_lower = query.to_lowercase();
    index
        .into_iter()
        .filter(|f| {
            let name_lower = f.name.to_lowercase();
            name_lower.contains(&query_lower) || f.path.to_lowercase().contains(&query_lower)
        })
        .take(limit)
        .collect()
}

fn should_skip_path(path: &Path) -> bool {
    // Skip common noisy or heavy directories for responsiveness.
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            let lower = name.to_ascii_lowercase();
            if lower == "node_modules"
                || lower == ".git"
                || lower == "target"
                || lower == ".cache"
                || lower == "library"
                || lower == "containers"
            {
                return true;
            }
        }
    }
    false
}

fn fallback_enabled() -> bool {
    // Disabled by default to prevent privacy prompts; users can opt-in via env var
    // Set VICEROY_FALLBACK_FS=1 to enable
    std::env::var(FALLBACK_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

fn path_to_info(path: &str) -> Option<FileInfo> {
    if path.is_empty() {
        return None;
    }
    let name = std::path::Path::new(path)
        .file_name()?
        .to_str()?
        .to_string();
    Some(FileInfo {
        name,
        path: path.to_string(),
    })
}

fn get_fallback_index() -> Vec<FileInfo> {
    if let Ok(mut cache) = FALLBACK_INDEX_CACHE.lock() {
        if let Some(entry) = cache.as_ref() {
            if entry.timestamp.elapsed() < FALLBACK_INDEX_TTL {
                return entry.results.clone();
            }
        }

        let rebuilt = build_fallback_index();
        *cache = Some(FileCacheEntry {
            query: String::new(),
            results: rebuilt.clone(),
            timestamp: Instant::now(),
        });
        return rebuilt;
    }
    build_fallback_index()
}

fn build_fallback_index() -> Vec<FileInfo> {
    // Build fallback index from user directories (Documents, Downloads, Desktop)
    // Only called when VICEROY_FALLBACK_FS=1 is explicitly set
    let mut results = Vec::new();

    if let Some(home) = dirs::home_dir() {
        for subdir in &["Documents", "Downloads", "Desktop"] {
            let root = home.join(subdir);
            if !root.exists() {
                continue;
            }

            for entry in WalkDir::new(&root)
                .max_depth(FALLBACK_MAX_DEPTH)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if results.len() >= FALLBACK_INDEX_LIMIT || results.len() >= FALLBACK_SCAN_LIMIT {
                    break;
                }
                let path = entry.path();
                if should_skip_path(path) {
                    continue;
                }
                if let Some(info) = path_to_info(path.to_string_lossy().as_ref()) {
                    results.push(info);
                }
            }
        }
    }
    results
}

fn try_cached_results(query: &str) -> Option<Vec<FileInfo>> {
    let query_lower = query.to_lowercase();
    let cache = FILE_SEARCH_CACHE.lock().ok()?;
    let entry = cache.as_ref()?;

    if entry.timestamp.elapsed() > CACHE_TTL {
        return None;
    }

    if entry.query == query {
        return Some(entry.results.clone());
    }

    if query.len() > entry.query.len() && query.starts_with(&entry.query) {
        // Narrowing search: filter the existing result set instead of spawning a new mdfind
        let filtered = entry
            .results
            .iter()
            .filter(|f| {
                f.name.to_lowercase().contains(&query_lower)
                    || f.path.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect::<Vec<_>>();
        return Some(filtered);
    }

    None
}

fn update_cache(query: &str, results: &[FileInfo]) {
    if let Ok(mut cache) = FILE_SEARCH_CACHE.lock() {
        *cache = Some(FileCacheEntry {
            query: query.to_string(),
            results: results.to_vec(),
            timestamp: Instant::now(),
        });
    }
}

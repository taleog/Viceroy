use anyhow::Result;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteInfo {
    pub title: String,
    pub path: String,
    pub relative_path: String,
    pub vault_name: Option<String>,
    pub modified_unix: i64,
}

struct NotesCache {
    vault_path: String,
    notes: Vec<NoteInfo>,
    timestamp: Instant,
}

lazy_static! {
    static ref NOTES_CACHE: Mutex<Option<NotesCache>> = Mutex::new(None);
}

const CACHE_TTL: Duration = Duration::from_secs(20);
const MAX_SCAN_DEPTH: usize = 12;
const MAX_NOTES: usize = 20_000;

pub fn search_notes(query: &str, vault_path: &str, vault_name: Option<&str>) -> Result<Vec<NoteInfo>> {
    let notes = get_or_build_index(vault_path, vault_name)?;
    let query_lower = query.trim().to_lowercase();
    if query_lower.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = notes
        .into_iter()
        .filter(|note| {
            let title = note.title.to_lowercase();
            let rel = note.relative_path.to_lowercase();
            title.contains(&query_lower) || rel.contains(&query_lower)
        })
        .collect::<Vec<_>>();

    results.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));
    Ok(results)
}

pub fn open_note_in_obsidian(path: &str, vault_path: &str, vault_name: Option<&str>) -> Result<()> {
    let relative_path = relative_note_path(path, vault_path)?;
    let vault_display = vault_name
        .map(|name| name.to_string())
        .or_else(|| {
            Path::new(vault_path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .unwrap_or_else(|| "Obsidian".to_string());

    let url = format!(
        "obsidian://open?vault={}&file={}",
        urlencoding::encode(&vault_display),
        urlencoding::encode(&relative_path)
    );

    open_target(&url)
}

pub fn reveal_note_in_finder(path: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()?;
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        super::app_launcher::open_file(path)
    }
}

fn open_target(target: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(target).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()?;
        return Ok(());
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn()?;
        return Ok(());
    }
}

fn get_or_build_index(vault_path: &str, vault_name: Option<&str>) -> Result<Vec<NoteInfo>> {
    let normalized_vault = Path::new(vault_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(vault_path));
    let normalized_vault_str = normalized_vault.to_string_lossy().to_string();

    if let Ok(cache) = NOTES_CACHE.lock() {
        if let Some(cache_entry) = cache.as_ref() {
            if cache_entry.vault_path == normalized_vault_str && cache_entry.timestamp.elapsed() < CACHE_TTL {
                return Ok(cache_entry.notes.clone());
            }
        }
    }

    let notes = build_index(&normalized_vault, vault_name)?;
    if let Ok(mut cache) = NOTES_CACHE.lock() {
        *cache = Some(NotesCache {
            vault_path: normalized_vault_str,
            notes: notes.clone(),
            timestamp: Instant::now(),
        });
    }

    Ok(notes)
}

fn build_index(vault_path: &Path, vault_name: Option<&str>) -> Result<Vec<NoteInfo>> {
    let mut notes = Vec::new();
    let derived_vault_name = vault_name
        .map(|name| name.to_string())
        .or_else(|| vault_path.file_name().and_then(|name| name.to_str()).map(|name| name.to_string()));

    for entry in WalkDir::new(vault_path)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if notes.len() >= MAX_NOTES {
            break;
        }

        let path = entry.path();
        if !path.is_file() || !is_markdown_file(path) || should_skip_path(path) {
            continue;
        }

        let absolute = path.to_string_lossy().to_string();
        let relative = match path.strip_prefix(vault_path) {
            Ok(rel) => rel.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string())
            .unwrap_or_else(|| relative.clone());
        let modified_unix = file_modified_unix(path);

        notes.push(NoteInfo {
            title,
            path: absolute,
            relative_path: relative,
            vault_name: derived_vault_name.clone(),
            modified_unix,
        });
    }

    Ok(notes)
}

fn relative_note_path(path: &str, vault_path: &str) -> Result<String> {
    let note_path = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    let root = Path::new(vault_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(vault_path));
    let relative = note_path.strip_prefix(root)?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some(ext) if ext.eq_ignore_ascii_case("md"))
}

fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .map(|value| {
                let lower = value.to_ascii_lowercase();
                lower == ".obsidian"
                    || lower == ".trash"
                    || lower == ".git"
                    || (lower.starts_with('.') && lower != ".")
            })
            .unwrap_or(false)
    })
}

fn file_modified_unix(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

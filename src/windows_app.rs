use arboard::Clipboard;
use eframe::egui::{
    self, vec2, Align, ColorImage, Image, Key, Layout, ScrollArea, Slider, TextEdit,
    TextureHandle, TextureOptions,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use viceroy::search_engine::{self, SearchResult};
use viceroy::{
    app_launcher,
    clipboard::{self, ClipboardEntry},
    clipboard_count, database, dictionary, obsidian, settings, sync, system_commands, updater,
    usage, web_search,
};

use crate::windows_hotkey::{start_hotkey_listener, HotkeyEvent};
use crate::windows_icon;
use crate::windows_preview::{self, PreviewCard, PreviewPanelState, PreviewSource};
use crate::windows_style::{self, BadgeTone};
use viceroy::logo;

use raw_window_handle::HasWindowHandle;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(Debug)]
enum AppSurface {
    Search,
    Clipboard,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Behavior,
    Obsidian,
    Sync,
}

#[derive(Clone)]
enum DisplayItem {
    Search(SearchResult),
    History(ClipboardEntry),
}

#[derive(Clone)]
struct WorkRequest {
    id: u64,
    surface: AppSurface,
    query: String,
    limit: usize,
}

#[derive(Clone)]
struct WorkResponse {
    id: u64,
    surface: AppSurface,
    query: String,
    items: Vec<DisplayItem>,
    status: String,
    clipboard_total_count: Option<usize>,
}

fn start_work_thread() -> (Sender<WorkRequest>, Receiver<WorkResponse>) {
    use std::sync::mpsc;

    let (req_tx, req_rx) = mpsc::channel::<WorkRequest>();
    let (resp_tx, resp_rx) = mpsc::channel::<WorkResponse>();

    thread::spawn(move || {
        let rt = Runtime::new().expect("failed to create windows worker runtime");

        while let Ok(mut req) = req_rx.recv() {
            // Coalesce bursts from live typing: always process the newest request available.
            while let Ok(newer) = req_rx.try_recv() {
                req = newer;
            }

            let mut items: Vec<DisplayItem> = Vec::new();
            let status: String;
            let mut clipboard_total_count: Option<usize> = None;

            match req.surface {
                AppSurface::Settings => {
                    status = "Preferences".to_string();
                }
                AppSurface::Search => {
                    if req.query.trim().is_empty() {
                        status = "Start typing to search apps, files, clipboard snippets, commands, and the web.".to_string();
                    } else {
                        match rt.block_on(search_engine::search_fast(&req.query)) {
                            Ok(mut results) => {
                                results.truncate(results.len().min(req.limit));
                                items = results.into_iter().map(DisplayItem::Search).collect();
                                status = if items.is_empty() {
                                    format!("No results for \"{}\".", req.query)
                                } else {
                                    format!("{} results for \"{}\".", items.len(), req.query)
                                };
                            }
                            Err(err) => {
                                status = format!("Search failed: {err:#}");
                            }
                        }
                    }
                }
                AppSurface::Clipboard => {
                    clipboard_total_count = clipboard_count::total_history_count().ok();

                    let result = if req.query.trim().is_empty() {
                        rt.block_on(clipboard::get_history(req.limit))
                    } else {
                        rt.block_on(clipboard::search_history(&req.query))
                    };

                    match result {
                        Ok(mut entries) => {
                            entries.truncate(entries.len().min(req.limit));
                            items = entries.into_iter().map(DisplayItem::History).collect();

                            status = if items.is_empty() {
                                if req.query.trim().is_empty() {
                                    "Clipboard history is empty.".to_string()
                                } else {
                                    format!("No clipboard entries match \"{}\".", req.query)
                                }
                            } else {
                                match clipboard_total_count {
                                    Some(total) => format!(
                                        "Showing {} of {} clipboard entries.",
                                        items.len(),
                                        total
                                    ),
                                    None => format!("Showing {} clipboard entries.", items.len()),
                                }
                            };
                        }
                        Err(err) => {
                            status = format!("Clipboard load failed: {err:#}");
                        }
                    }
                }
            }

            let _ = resp_tx.send(WorkResponse {
                id: req.id,
                surface: req.surface,
                query: req.query,
                items,
                status,
                clipboard_total_count,
            });
        }
    });

    (req_tx, resp_rx)
}

impl DisplayItem {
    fn primary_text(&self) -> String {
        match self {
            Self::Search(result) => match result {
                SearchResult::Link { host, .. } => format!("Open {host}"),
                SearchResult::App { name, .. } => name.clone(),
                SearchResult::File { name, .. } => name.clone(),
                SearchResult::Clipboard {
                    custom_name,
                    preview,
                    ..
                } => custom_name.clone().unwrap_or_else(|| preview.clone()),
                SearchResult::Note { title, .. } => title.clone(),
                SearchResult::Command { name, .. } => name.clone(),
                SearchResult::Calculator {
                    expression, result, ..
                } => format!("{expression} = {result}"),
                SearchResult::Emoji { emoji, name, .. } => format!("{emoji} {name}"),
                SearchResult::Dictionary { word, .. } => format!("Define {word}"),
                SearchResult::WebSearch { engine, query, .. } => {
                    format!("Search {engine} for {query}")
                }
            },
            Self::History(entry) => history_title(entry),
        }
    }

    fn secondary_text(&self) -> String {
        match self {
            Self::Search(result) => match result {
                SearchResult::Link { url, .. } => url.clone(),
                SearchResult::App { path, .. } => path.clone(),
                SearchResult::File { path, .. } => path.clone(),
                SearchResult::Clipboard {
                    content_type,
                    app_name,
                    timestamp,
                    image_width,
                    image_height,
                    ..
                } => history_subtitle_from_fields(
                    content_type,
                    app_name.as_ref(),
                    *timestamp,
                    *image_width,
                    *image_height,
                ),
                SearchResult::Note {
                    relative_path,
                    vault_name,
                    ..
                } => vault_name
                    .as_ref()
                    .map(|vault| format!("{} | {}", vault, relative_path))
                    .unwrap_or_else(|| relative_path.clone()),
                SearchResult::Command { description, .. } => description.clone(),
                SearchResult::Calculator { formats, .. } => formats
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Calculator".to_string()),
                SearchResult::Emoji { keywords, .. } => keywords.join(", "),
                SearchResult::Dictionary { preview, .. } => preview.clone(),
                SearchResult::WebSearch { url, .. } => url.clone(),
            },
            Self::History(entry) => history_subtitle(entry),
        }
    }

    fn badge(&self) -> &'static str {
        match self {
            Self::Search(result) => match result {
                SearchResult::Link { .. } => "LINK",
                SearchResult::App { .. } => "APP",
                SearchResult::File { .. } => "FILE",
                SearchResult::Clipboard { content_type, .. } => {
                    if content_type == "image" {
                        "IMAGE"
                    } else {
                        "CLIP"
                    }
                }
                SearchResult::Note { .. } => "NOTE",
                SearchResult::Command { .. } => "CMD",
                SearchResult::Calculator { .. } => "CALC",
                SearchResult::Emoji { .. } => "EMOJI",
                SearchResult::Dictionary { .. } => "DICT",
                SearchResult::WebSearch { .. } => "WEB",
            },
            Self::History(entry) => {
                if entry.content_type == "image" {
                    "IMAGE"
                } else {
                    "CLIP"
                }
            }
        }
    }

    fn badge_tone(&self) -> BadgeTone {
        match self {
            Self::Search(result) => match result {
                SearchResult::Link { .. } => BadgeTone::Neutral,
                SearchResult::App { .. } => BadgeTone::Accent,
                SearchResult::File { .. } => BadgeTone::Neutral,
                SearchResult::Clipboard { .. } => BadgeTone::Accent,
                SearchResult::Note { .. } => BadgeTone::Accent,
                SearchResult::Command { .. } => BadgeTone::Warning,
                SearchResult::Calculator { .. } => BadgeTone::Success,
                SearchResult::Emoji { .. } => BadgeTone::Accent,
                SearchResult::Dictionary { .. } => BadgeTone::Accent,
                SearchResult::WebSearch { .. } => BadgeTone::Neutral,
            },
            Self::History(entry) => {
                if entry.content_type == "image" {
                    BadgeTone::Warning
                } else {
                    BadgeTone::Accent
                }
            }
        }
    }

    fn action_label(&self) -> &'static str {
        match self {
            Self::Search(SearchResult::Clipboard { .. }) | Self::History(_) => "Restore",
            Self::Search(SearchResult::Calculator { .. })
            | Self::Search(SearchResult::Emoji { .. }) => "Copy",
            _ => "Open",
        }
    }

    fn is_pinned(&self) -> bool {
        match self {
            Self::Search(SearchResult::Clipboard { is_pinned, .. }) => *is_pinned,
            Self::History(entry) => entry.is_pinned,
            _ => false,
        }
    }

    fn history_entry(&self) -> Option<&ClipboardEntry> {
        match self {
            Self::History(entry) => Some(entry),
            _ => None,
        }
    }
}

struct ClipboardEditor {
    entry_id: i64,
    custom_name: String,
    content: String,
    is_text: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewCacheKey {
    EmptySearch,
    EmptyClipboard,
    EmptySettings,
    SearchClipboard(i64),
    SearchLink(u64),
    HistoryEntry(i64),
    SearchApp(u64),
    SearchFile(u64),
    SearchCommand(u64),
    SearchCalculator(u64),
    SearchEmoji(u64),
    SearchDictionary(u64),
    SearchWeb(u64),
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    if let Err(err) = database::init() {
        eprintln!("Database init error: {err}");
        return;
    }
    if let Err(err) = sync::init() {
        eprintln!("Sync init error: {err:#}");
    }
    if let Err(err) = sync::start_background_worker() {
        eprintln!("Sync worker start error: {err:#}");
    }

    start_clipboard_monitor();
    maybe_check_for_updates(&args);

    let runtime = Arc::new(Runtime::new().expect("failed to create tokio runtime"));
    let initial_query = extract_query_args(&args).join(" ");
    let icon_data = eframe::icon_data::from_png_bytes(logo::APP_ICON_PNG)
        .expect("failed to decode Viceroy app icon");
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Viceroy")
            .with_inner_size([960.0, 124.0])
            .with_min_inner_size([960.0, 124.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_has_shadow(true)
            .with_always_on_top()
            .with_visible(true)
            .with_icon(icon_data),
        ..Default::default()
    };

    let app_runtime = runtime.clone();
    let app_query = initial_query.clone();
    if let Err(err) = eframe::run_native(
        "Viceroy",
        native_options,
        Box::new(move |cc| Ok(Box::new(ViceroyWindowsApp::new(cc, app_runtime, app_query)))),
    ) {
        eprintln!("Failed to launch Viceroy window: {err}");
    }
}

fn print_help() {
    println!("Viceroy v{}", env!("CARGO_PKG_VERSION"));
    println!("Windows launcher");
    println!();
    println!("Usage:");
    println!("  viceroy");
    println!("  viceroy [initial query]");
    println!("  viceroy --help");
}

fn extract_query_args(args: &[String]) -> Vec<String> {
    args.iter()
        .skip(1)
        .filter(|arg| !arg.starts_with("--"))
        .cloned()
        .collect()
}

fn start_clipboard_monitor() {
    thread::spawn(|| {
        let runtime = Runtime::new().expect("failed to create clipboard runtime");
        if let Err(err) = runtime.block_on(clipboard::start_monitor()) {
            eprintln!("Clipboard monitor error: {err}");
        }
    });
}

fn maybe_check_for_updates(args: &[String]) {
    if updater::update_check_disabled(args) {
        return;
    }

    let args = args.to_vec();
    thread::spawn(move || {
        let runtime = Runtime::new().expect("failed to create updater runtime");
        let silent = updater::silent_update_check(&args);
        if let Err(err) = runtime.block_on(updater::check_for_updates(silent)) {
            log::error!("Update check failed: {err:#}");
        }
    });
}

struct ViceroyWindowsApp {
    runtime: Arc<Runtime>,
    query: String,
    items: Vec<DisplayItem>,
    selected: usize,
    surface: AppSurface,
    last_loaded_query: String,
    last_loaded_surface: AppSurface,
    last_loaded_clipboard_revision: u64,
    status: String,
    hotkey: String,
    hotkey_events: Receiver<HotkeyEvent>,
    hotkey_message: Option<String>,

    work_tx: Sender<WorkRequest>,
    work_rx: Receiver<WorkResponse>,
    next_work_id: u64,
    inflight_work_id: Option<u64>,

    window_minimized: bool,
    window_was_focused: bool,
    focus_grace_frames: u8,
    focus_query_next_frame: bool,
    scroll_to_selected: bool,
    // (KNOWN BUG) Caret/cursor position in the search bar resets to the end when expanding from slim to full view.
    // This is a limitation of egui/eframe 0.33.x, which does not support programmatic caret control.
    hotkey_toggle_cooldown_until: Option<Instant>,
    pending_reload: bool,
    last_query_edit: Option<Instant>,
    clipboard_total_count: Option<usize>,
    max_results: usize,
    icon_cache: HashMap<u64, eframe::egui::TextureHandle>,
    icon_cache_failures: HashSet<u64>,
    logo_badge: Option<TextureHandle>,
    paste_after_restore: bool,
    dismiss_on_escape: bool,
    dismiss_on_click_away: bool,
    sync_enabled: bool,
    sync_mirror_clipboard: bool,
    sync_device_name: String,
    sync_device_id: String,
    sync_server_url: String,
    sync_auth_token: String,
    obsidian_enabled: bool,
    obsidian_vault_path: String,
    obsidian_vault_name: String,
    obsidian_open_in_obsidian: bool,
    obsidian_message: String,
    sync_status: Option<sync::SyncStatus>,
    sync_test_result: Option<sync::SyncConnectionTestResult>,
    sync_message: String,
    settings_tab: SettingsTab,
    clipboard_editor: Option<ClipboardEditor>,
    preview_state: PreviewPanelState,
    preview_cache_key: Option<PreviewCacheKey>,
    preview_cache_card: PreviewCard,
    last_window_size: [f32; 2],
}

impl ViceroyWindowsApp {
    fn new(cc: &eframe::CreationContext<'_>, runtime: Arc<Runtime>, initial_query: String) -> Self {
        windows_style::apply_launcher_theme(&cc.egui_ctx);
        let app_settings = settings::load().unwrap_or_default();
        let (work_tx, work_rx) = start_work_thread();
        let logo_badge = match logo::decode_png_rgba(logo::TRAY_ICON_PNG) {
            Ok((width, height, rgba)) => {
                let image = ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &rgba,
                );
                Some(cc.egui_ctx.load_texture(
                    "viceroy_logo_badge",
                    image,
                    TextureOptions::LINEAR,
                ))
            }
            Err(err) => {
                log::warn!("failed to decode tray logo badge: {err:#}");
                None
            }
        };

        let mut app = Self {
            runtime,
            query: initial_query,
            items: Vec::new(),
            selected: 0,
            surface: AppSurface::Search,
            last_loaded_query: String::new(),
            last_loaded_surface: AppSurface::Settings,
            last_loaded_clipboard_revision: clipboard::history_revision(),
            status: "Search apps, files, clipboard snippets, commands, and the web.".to_string(),
            hotkey: app_settings.hotkey.clone(),
            hotkey_events: start_hotkey_listener(&app_settings.hotkey),
            hotkey_message: None,

            work_tx,
            work_rx,
            next_work_id: 1,
            inflight_work_id: None,

            window_minimized: false,
            window_was_focused: false,
            focus_grace_frames: 12,
            focus_query_next_frame: true,
            scroll_to_selected: true,
            hotkey_toggle_cooldown_until: None,
            pending_reload: false,
            last_query_edit: None,
            clipboard_total_count: None,
            max_results: app_settings.max_results,
            icon_cache: HashMap::new(),
            icon_cache_failures: HashSet::new(),
            logo_badge,
            paste_after_restore: app_settings.paste_after_restore,
            dismiss_on_escape: app_settings.dismiss_on_escape,
            dismiss_on_click_away: app_settings.dismiss_on_click_away,
            sync_enabled: app_settings.sync.enabled,
            sync_mirror_clipboard: app_settings.sync.mirror_clipboard,
            sync_device_name: app_settings.sync.device_name.clone(),
            sync_device_id: app_settings.sync.device_id.clone(),
            sync_server_url: app_settings.sync.server_url.unwrap_or_default(),
            sync_auth_token: app_settings.sync.auth_token.unwrap_or_default(),
            obsidian_enabled: app_settings.obsidian.enabled,
            obsidian_vault_path: app_settings.obsidian.vault_path.unwrap_or_default(),
            obsidian_vault_name: app_settings.obsidian.vault_name.unwrap_or_default(),
            obsidian_open_in_obsidian: app_settings.obsidian.open_in_obsidian,
            obsidian_message: String::new(),
            sync_status: None,
            sync_test_result: None,
            sync_message: String::new(),
            settings_tab: SettingsTab::General,
            clipboard_editor: None,
            preview_state: PreviewPanelState::new(),
            preview_cache_key: None,
            preview_cache_card: PreviewCard::empty("Loading preview..."),
            last_window_size: [0.0, 0.0],
        };
        app.refresh_sync_status();
        app.pending_reload = true;
        app.last_query_edit = Some(Instant::now());
        app
    }

    fn set_surface(&mut self, surface: AppSurface) {
        if self.surface != surface {
            self.surface = surface;
            self.selected = 0;
            self.scroll_to_selected = true;
            if surface != AppSurface::Clipboard {
                self.clipboard_editor = None;
            }
            self.preview_cache_key = None;
        }
    }

    fn is_collapsed_search(&self) -> bool {
        self.surface == AppSurface::Search
            && self.query.trim().is_empty()
            && self.items.is_empty()
            && self.clipboard_editor.is_none()
    }

    fn target_window_size(&self) -> [f32; 2] {
        match self.surface {
            AppSurface::Settings => [1120.0, 780.0],
            AppSurface::Clipboard => [1120.0, 720.0],
            AppSurface::Search if self.is_collapsed_search() => [960.0, 124.0],
            AppSurface::Search => [1120.0, 720.0],
        }
    }

    fn sync_window_geometry(&mut self, ctx: &egui::Context) {
        let target = self.target_window_size();
        if self.last_window_size == target {
            return;
        }
        self.last_window_size = target;
        let size = egui::vec2(target[0], target[1]);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size));
        if let Some(hwnd) = crate::windows_hwnd::get() {
            crate::windows_dwm::center_now(hwnd, target[0] as i32, target[1] as i32);
        }
    }

    fn request_reload(&mut self, force: bool) {
        let clipboard_revision = clipboard::history_revision();
        if !force
            && self.last_loaded_query == self.query
            && self.last_loaded_surface == self.surface
            && (self.surface != AppSurface::Clipboard
                || self.last_loaded_clipboard_revision == clipboard_revision)
        {
            return;
        }

        let limit = self.max_results.clamp(10, 200);
        let id = self.next_work_id;
        self.next_work_id = self.next_work_id.saturating_add(1);
        self.inflight_work_id = Some(id);

        // Keep existing items on screen while searching to avoid UI "thrash".
        if self.surface == AppSurface::Search && !self.query.trim().is_empty() {
            self.status = format!("Searching for \"{}\"...", self.query);
        }

        let _ = self.work_tx.send(WorkRequest {
            id,
            surface: self.surface,
            query: self.query.clone(),
            limit,
        });

        self.last_loaded_query = self.query.clone();
        self.last_loaded_surface = self.surface;
        self.last_loaded_clipboard_revision = clipboard_revision;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.items.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
        self.preview_cache_key = None;
        self.scroll_to_selected = true;
    }

    fn selected_item(&self) -> Option<&DisplayItem> {
        self.items.get(self.selected)
    }

    fn refresh_preview_card_cache(&mut self) {
        let key = self.preview_cache_key_for_selected_item();
        if self.preview_cache_key != Some(key) {
            self.preview_cache_card = match self.selected_item() {
                Some(DisplayItem::Search(result)) => {
                    windows_preview::preview_card(PreviewSource::from_search_result(result))
                }
                Some(DisplayItem::History(entry)) => {
                    windows_preview::preview_card(PreviewSource::from_clipboard_entry(entry))
                }
                None => PreviewCard::empty(match self.surface {
                    AppSurface::Search => {
                        "Start typing to search, then use Up and Down to move through results."
                    }
                    AppSurface::Clipboard => {
                        "Select a clipboard entry to inspect its content and metadata."
                    }
                    AppSurface::Settings => "Choose a settings tab to continue.",
                }),
            };
            self.preview_cache_key = Some(key);
        }
    }

    fn preview_cache_key_for_selected_item(&self) -> PreviewCacheKey {
        match self.selected_item() {
            Some(DisplayItem::History(entry)) => PreviewCacheKey::HistoryEntry(entry.id),
            Some(DisplayItem::Search(result)) => match result {
                SearchResult::Clipboard { id, .. } => PreviewCacheKey::SearchClipboard(*id),
                SearchResult::Link { url, .. } => {
                    PreviewCacheKey::SearchLink(stable_preview_hash(url))
                }
                SearchResult::App { path, .. } => {
                    PreviewCacheKey::SearchApp(stable_preview_hash(path))
                }
                SearchResult::File { path, .. } => {
                    PreviewCacheKey::SearchFile(stable_preview_hash(path))
                }
                SearchResult::Note { path, .. } => {
                    PreviewCacheKey::SearchFile(stable_preview_hash(path))
                }
                SearchResult::Command { command, .. } => {
                    PreviewCacheKey::SearchCommand(stable_preview_hash(command))
                }
                SearchResult::Calculator {
                    expression, result, ..
                } => PreviewCacheKey::SearchCalculator(stable_preview_hash(&format!(
                    "{expression}={result}"
                ))),
                SearchResult::Emoji { emoji, name, .. } => {
                    PreviewCacheKey::SearchEmoji(stable_preview_hash(&format!("{emoji}:{name}")))
                }
                SearchResult::Dictionary { word, .. } => {
                    PreviewCacheKey::SearchDictionary(stable_preview_hash(word))
                }
                SearchResult::WebSearch { url, .. } => {
                    PreviewCacheKey::SearchWeb(stable_preview_hash(url))
                }
            },
            None => match self.surface {
                AppSurface::Search => PreviewCacheKey::EmptySearch,
                AppSurface::Clipboard => PreviewCacheKey::EmptyClipboard,
                AppSurface::Settings => PreviewCacheKey::EmptySettings,
            },
        }
    }

    fn activate_selected(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            return;
        };
        self.status = match execute_item(&self.runtime, &item) {
            Ok(message) => message,
            Err(err) => format!("Action failed: {err:#}"),
        };
    }

    fn copy_selected(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            return;
        };
        self.status = match copy_item(&self.runtime, &item) {
            Ok(message) => message,
            Err(err) => format!("Copy failed: {err:#}"),
        };
    }

    fn begin_clipboard_edit(&mut self) {
        let Some(entry) = self
            .selected_item()
            .and_then(DisplayItem::history_entry)
            .cloned()
        else {
            return;
        };
        self.clipboard_editor = Some(ClipboardEditor {
            entry_id: entry.id,
            custom_name: entry.custom_name.unwrap_or_default(),
            content: if entry.content_type == "text" {
                entry.content
            } else {
                String::new()
            },
            is_text: entry.content_type == "text",
        });
        self.status = "Editing clipboard entry.".to_string();
    }

    fn cancel_clipboard_edit(&mut self) {
        self.clipboard_editor = None;
        self.status = "Clipboard edit canceled.".to_string();
    }

    fn save_clipboard_edit(&mut self) {
        let Some(editor) = &self.clipboard_editor else {
            return;
        };
        let entry_id = editor.entry_id;
        let name = non_empty(editor.custom_name.trim());
        let result = if editor.is_text {
            self.runtime.block_on(clipboard::update_entry(
                entry_id,
                editor.content.clone(),
                name.clone(),
            ))
        } else {
            self.runtime
                .block_on(clipboard::update_custom_name(entry_id, name.clone()))
        };
        match result {
            Ok(()) => {
                self.clipboard_editor = None;
                self.request_reload(true);
                if let Some(index) = self.items.iter().position(|item| match item {
                    DisplayItem::History(entry) => entry.id == entry_id,
                    _ => false,
                }) {
                    self.selected = index;
                }
                self.status = "Clipboard entry saved.".to_string();
            }
            Err(err) => self.status = format!("Failed to save clipboard entry: {err:#}"),
        }
    }

    fn remove_selected_history_entry(&mut self) {
        let Some(entry) = self
            .selected_item()
            .and_then(DisplayItem::history_entry)
            .cloned()
        else {
            return;
        };
        match self.runtime.block_on(clipboard::delete_entry(entry.id)) {
            Ok(()) => {
                self.clipboard_editor = None;
                self.request_reload(true);
                self.status = "Clipboard entry removed.".to_string();
            }
            Err(err) => self.status = format!("Failed to remove clipboard entry: {err:#}"),
        }
    }

    fn refresh_sync_status(&mut self) {
        match self.runtime.block_on(sync::refresh_remote_status()) {
            Ok(status) => {
                self.sync_device_id = status.device.device_id.clone();
                if self.sync_device_name.trim().is_empty() {
                    self.sync_device_name = status.device.device_name.clone();
                }
                if self.sync_server_url.trim().is_empty() {
                    self.sync_server_url = status.server_url.clone().unwrap_or_default();
                }
                self.sync_status = Some(status);
                if self.sync_message.is_empty() {
                    self.sync_message =
                        "Sync status loaded. Save settings after changing server details."
                            .to_string();
                }
            }
            Err(err) => {
                self.sync_status = None;
                self.sync_message = format!("Failed to load sync status: {err:#}");
            }
        }
    }

    fn test_sync_connection(&mut self) {
        let auth_token = non_empty(self.sync_auth_token.trim());
        let result = self.runtime.block_on(sync::test_connection(
            &self.sync_server_url,
            auth_token.as_deref(),
        ));

        if let Some(url) = result.normalized_server_url.clone() {
            self.sync_server_url = url;
        }

        self.sync_message = result.message.clone();
        let ok = result.ok;
        self.sync_test_result = Some(result);
        if ok {
            if self.sync_enabled {
                self.refresh_sync_status();
            } else if let Ok(status) = sync::status() {
                self.sync_status = Some(status);
            }
        }
    }

    fn save_settings(&mut self) {
        match settings::load() {
            Ok(mut app_settings) => {
                if self.hotkey.trim().is_empty() {
                    self.sync_message = "Hotkey cannot be empty.".to_string();
                    return;
                }
                let old_enabled = app_settings.sync.enabled;
                let old_server_url = app_settings.sync.server_url.clone().unwrap_or_default();
                let old_auth_token = app_settings.sync.auth_token.clone().unwrap_or_default();

                let prepared_sync = match settings::prepare_sync_settings(
                    self.sync_enabled,
                    &self.sync_device_name,
                    &self.sync_server_url,
                    &self.sync_auth_token,
                ) {
                    Ok(prepared) => prepared,
                    Err(err) => {
                        self.sync_message = format!("{err:#}");
                        return;
                    }
                };
                let prepared_obsidian = match settings::prepare_obsidian_settings(
                    self.obsidian_enabled,
                    &self.obsidian_vault_path,
                    &self.obsidian_vault_name,
                ) {
                    Ok(prepared) => prepared,
                    Err(err) => {
                        self.obsidian_message = format!("{err:#}");
                        return;
                    }
                };

                app_settings.hotkey = self.hotkey.trim().to_string();
                app_settings.max_results = self.max_results.clamp(10, 200);
                app_settings.paste_after_restore = self.paste_after_restore;
                app_settings.dismiss_on_escape = self.dismiss_on_escape;
                app_settings.dismiss_on_click_away = self.dismiss_on_click_away;
                app_settings.sync.enabled = self.sync_enabled;
                app_settings.sync.mirror_clipboard = self.sync_mirror_clipboard;
                app_settings.sync.device_name = prepared_sync.device_name.clone();
                app_settings.sync.server_url = prepared_sync.server_url.clone();
                app_settings.sync.auth_token = prepared_sync.auth_token.clone();
                app_settings.obsidian.enabled = self.obsidian_enabled;
                app_settings.obsidian.vault_path = prepared_obsidian.vault_path.clone();
                app_settings.obsidian.vault_name = prepared_obsidian.vault_name.clone();
                app_settings.obsidian.open_in_obsidian = self.obsidian_open_in_obsidian;
                self.sync_device_name = prepared_sync.device_name;
                self.sync_server_url = prepared_sync.server_url.unwrap_or_default();
                self.sync_auth_token = prepared_sync.auth_token.unwrap_or_default();
                self.obsidian_vault_path = prepared_obsidian.vault_path.unwrap_or_default();
                self.obsidian_vault_name = prepared_obsidian.vault_name.unwrap_or_default();

                if let Err(err) = settings::save(&app_settings) {
                    self.sync_message = format!("Failed to save settings: {err:#}");
                    return;
                }
                app_settings = match settings::load() {
                    Ok(settings) => settings,
                    Err(err) => {
                        self.sync_message =
                            format!("Settings were written, but reloading them failed: {err:#}");
                        return;
                    }
                };
                self.hotkey = app_settings.hotkey.clone();
                self.max_results = app_settings.max_results;
                self.paste_after_restore = app_settings.paste_after_restore;
                self.dismiss_on_escape = app_settings.dismiss_on_escape;
                self.dismiss_on_click_away = app_settings.dismiss_on_click_away;

                if self.hotkey != app_settings.hotkey {
                    self.hotkey_message = Some(
                        "Hotkey saved. Restart Viceroy once to apply the new global shortcut."
                            .to_string(),
                    );
                    self.hotkey = app_settings.hotkey.clone();
                } else {
                    self.hotkey_message = None;
                }
                self.sync_enabled = app_settings.sync.enabled;
                self.sync_mirror_clipboard = app_settings.sync.mirror_clipboard;
                self.sync_device_name = app_settings.sync.device_name.clone();
                self.sync_server_url = app_settings.sync.server_url.clone().unwrap_or_default();
                self.sync_auth_token = app_settings.sync.auth_token.clone().unwrap_or_default();
                self.obsidian_enabled = app_settings.obsidian.enabled;
                self.obsidian_vault_path =
                    app_settings.obsidian.vault_path.clone().unwrap_or_default();
                self.obsidian_vault_name =
                    app_settings.obsidian.vault_name.clone().unwrap_or_default();
                self.obsidian_open_in_obsidian = app_settings.obsidian.open_in_obsidian;
                match sync::init() {
                    Ok(status) => {
                        self.sync_status = Some(status.clone());
                        self.sync_device_id = status.device.device_id.clone();
                        self.sync_device_name = status.device.device_name.clone();
                    }
                    Err(err) => {
                        self.sync_message =
                            format!("Settings saved, but sync init failed: {err:#}");
                        return;
                    }
                }
                if self.sync_enabled {
                    if let Err(err) = sync::start_background_worker() {
                        self.sync_message =
                            format!("Settings saved, but sync worker failed to start: {err:#}");
                        return;
                    }
                }

                let connection_changed = old_enabled
                    && self.sync_enabled
                    && (old_server_url != self.sync_server_url.trim()
                        || old_auth_token != self.sync_auth_token.trim());
                self.sync_message = if connection_changed {
                    "Sync settings saved. The background worker is reconnecting with the updated server details."
                        .to_string()
                } else if self.sync_enabled && !old_enabled {
                    "Sync enabled. The background worker will use this server for new uploads."
                        .to_string()
                } else if !self.sync_enabled {
                    "Sync settings saved. Sync is disabled until you re-enable it.".to_string()
                } else {
                    "Sync settings saved.".to_string()
                };
                if self.sync_enabled {
                    let auth_token = non_empty(self.sync_auth_token.trim());
                    let result = self.runtime.block_on(sync::test_connection(
                        &self.sync_server_url,
                        auth_token.as_deref(),
                    ));
                    if let Some(url) = result.normalized_server_url.clone() {
                        self.sync_server_url = url;
                    }
                    self.sync_message = if result.ok {
                        format!("Sync settings saved. {}", result.message)
                    } else {
                        format!("Sync settings saved, but {}", result.message)
                    };
                    self.sync_test_result = Some(result);
                } else {
                    self.sync_test_result = None;
                }
                let obsidian_label = if self.obsidian_vault_name.trim().is_empty() {
                    Path::new(&self.obsidian_vault_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("the selected vault")
                        .to_string()
                } else {
                    self.obsidian_vault_name.trim().to_string()
                };
                self.obsidian_message = if self.obsidian_enabled {
                    format!("Obsidian note search is ready for {obsidian_label}.")
                } else {
                    "Obsidian note search is off.".to_string()
                };
                self.status = "Settings saved.".to_string();
                self.request_reload(true);
            }
            Err(err) => {
                self.sync_message = format!("Failed to load current settings: {err:#}");
            }
        }
    }

    fn handle_escape(&mut self, ctx: &egui::Context) {
        if self.clipboard_editor.is_some() {
            self.cancel_clipboard_edit();
            return;
        }
        if self.surface == AppSurface::Settings {
            self.set_surface(AppSurface::Search);
            return;
        }
        if self.surface == AppSurface::Clipboard {
            self.set_surface(AppSurface::Search);
            self.query.clear();
            self.request_reload(true);
            return;
        }
        if self.dismiss_on_escape {
            self.query.clear();
            self.window_minimized = true;
            self.window_was_focused = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn hide_launcher(&mut self, ctx: &egui::Context) {
        eprintln!("[hide_launcher] called, surface={:?}", self.surface);
        self.window_minimized = true;
        self.window_was_focused = false;
        self.focus_grace_frames = 0;
        // If hiding from clipboard view, always reset to Search and clear clipboard editor
        if self.surface == AppSurface::Clipboard {
            eprintln!("[hide_launcher] Resetting surface from Clipboard to Search and clearing clipboard_editor");
            self.set_surface(AppSurface::Search);
            self.clipboard_editor = None;
            self.query.clear();
            self.request_reload(true);
        }
        // Avoid fully hiding the window: some event loops stop updating when invisible.
        // Minimize instead, so the app can still process global hotkey events.
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    fn icon_texture_for_item(
        &mut self,
        ctx: &egui::Context,
        item: &DisplayItem,
    ) -> Option<eframe::egui::TextureHandle> {
        match item {
            DisplayItem::Search(SearchResult::App { path, .. })
            | DisplayItem::Search(SearchResult::File { path, .. }) => {
                self.icon_texture_for_path(ctx, path)
            }
            _ => None,
        }
    }

    fn icon_texture_for_path(
        &mut self,
        ctx: &egui::Context,
        path: &str,
    ) -> Option<eframe::egui::TextureHandle> {
        // Icon extraction on Windows is expensive (Win32 + GDI). Avoid doing it while the user
        // is actively typing, or we'll tank search UX.
        if self.pending_reload {
            return None;
        }
        if let Some(last_edit) = self.last_query_edit {
            if last_edit.elapsed() < Duration::from_millis(120) {
                return None;
            }
        }
        let key = stable_preview_hash(path);
        if let Some(texture) = self.icon_cache.get(&key) {
            return Some(texture.clone());
        }
        if self.icon_cache_failures.contains(&key) {
            return None;
        }

        let icon = match windows_icon::load_file_icon_rgba(path) {
            Ok(icon) => icon,
            Err(err) => {
                log::debug!("icon load failed for {path}: {err:#}");
                self.icon_cache_failures.insert(key);
                return None;
            }
        };

        let image = ColorImage::from_rgba_unmultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        );
        let texture = ctx.load_texture(format!("win_icon_{key}"), image, TextureOptions::LINEAR);
        self.icon_cache.insert(key, texture.clone());
        Some(texture)
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mode_label = match self.surface {
                AppSurface::Search => "Search",
                AppSurface::Clipboard => "Clipboard",
                AppSurface::Settings => "Settings",
            };
            windows_style::badge_frame(BadgeTone::Neutral).show(ui, |ui| {
                ui.label(windows_style::badge_text(
                    mode_label.to_uppercase(),
                    BadgeTone::Neutral,
                ));
            });
            ui.label(windows_style::muted_text(match self.surface {
                AppSurface::Search => "Search-first launcher shell",
                AppSurface::Clipboard => "Clipboard history mode",
                AppSurface::Settings => "Preferences",
            }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(windows_style::ghost_button("Settings")).clicked() {
                    self.set_surface(AppSurface::Settings);
                }
                if ui
                    .add(windows_style::tab_button(
                        "Clipboard",
                        self.surface == AppSurface::Clipboard,
                    ))
                    .clicked()
                {
                    self.set_surface(AppSurface::Clipboard);
                }
                if ui
                    .add(windows_style::tab_button(
                        "Search",
                        self.surface == AppSurface::Search,
                    ))
                    .clicked()
                {
                    self.set_surface(AppSurface::Search);
                }
            });
        });
    }

    /// Renders the search bar and handles input.
    ///
    /// (KNOWN BUG) Caret/cursor position resets to the end when the search bar expands from slim to full view.
    /// This is due to limitations in egui/eframe 0.33.x, which does not allow programmatic caret control.
    /// Upgrading egui may allow a fix in the future.
    fn render_search_box(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let slim = self.surface == AppSurface::Search
            && self.query.trim().is_empty()
            && self.items.is_empty();

        windows_style::search_shell_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                windows_style::icon_badge_frame().show(ui, |ui| {
                    if let Some(logo_badge) = &self.logo_badge {
                        ui.add(Image::new((logo_badge.id(), vec2(20.0, 20.0))));
                    } else {
                        ui.label(windows_style::search_icon_text());
                    }
                });

                let right_hint = if self.surface == AppSurface::Clipboard {
                    "Tab Search   Enter Restore   Esc Hide"
                } else {
                    "Tab Clipboard   Ctrl+, Settings   Esc Hide"
                };
                let reserved_hint_width = if slim { 0.0 } else { 255.0 };
                let input_width = (ui.available_width() - reserved_hint_width).max(260.0);

                let text_edit = TextEdit::singleline(&mut self.query)
                    .id_salt("launcher_query")
                    .font(if slim {
                        egui::TextStyle::Heading
                    } else {
                        egui::TextStyle::Body
                    })
                    .frame(false)
                    .hint_text(if self.surface == AppSurface::Clipboard {
                        "Filter clipboard history"
                    } else {
                        "Search apps, files, clipboard history, and more"
                    });

                let response = ui.add_sized([
                    input_width,
                    if slim { 54.0 } else { 42.0 }
                ], text_edit);

                if self.focus_query_next_frame {
                    response.request_focus();
                    if self.focus_grace_frames == 0 {
                        self.focus_query_next_frame = false;
                    }
                }
                if response.changed() {
                    changed = true;
                    self.pending_reload = true;
                    self.last_query_edit = Some(Instant::now());
                    if self.surface == AppSurface::Search && self.query.trim().is_empty() {
                        self.items.clear();
                        self.selected = 0;
                        self.scroll_to_selected = true;
                        self.preview_cache_key = None;
                        self.status =
                            "Search apps, files, clipboard snippets, commands, and the web."
                                .to_string();
                    }
                }
                if !slim {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(windows_style::premium_hint_text(right_hint));
                    });
                }
            });
        });
        changed
    }

    fn render_results(&mut self, ui: &mut egui::Ui) {
        ui.push_id("results_panel", |ui| {
            windows_style::panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(windows_style::section_text(
                        if self.surface == AppSurface::Clipboard {
                            "History"
                        } else {
                            "Results"
                        },
                    ));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let item_label = if self.surface == AppSurface::Clipboard {
                            match self.clipboard_total_count {
                                Some(total) => {
                                    format!("{} shown / {} total", self.items.len(), total)
                                }
                                None => format!("{} shown", self.items.len()),
                            }
                        } else {
                            format!("{} items", self.items.len())
                        };
                        ui.label(windows_style::muted_text(item_label));
                    });
                });
                ui.add_space(8.0);
                if self.items.is_empty() {
                    ui.add_space(18.0);
                    ui.vertical_centered(|ui| {
                        ui.label(windows_style::section_text(
                            if self.surface == AppSurface::Clipboard {
                                "No clipboard entries"
                            } else {
                                "No results yet"
                            },
                        ));
                        ui.label(windows_style::muted_text(
                            if self.surface == AppSurface::Clipboard {
                                "Copy something first, or clear the filter."
                            } else {
                                "Try a broader query or switch to clipboard history."
                            },
                        ));
                    });
                    return;
                }

                let mut activate = None;
                ScrollArea::vertical()
                    .id_salt("results_scroll")
                    .show(ui, |ui| {
                        for index in 0..self.items.len() {
                            let item = self.items[index].clone();
                            let response = windows_style::card_frame(index == self.selected)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let icon_texture =
                                            self.icon_texture_for_item(ui.ctx(), &item);
                                        if let Some(texture) = icon_texture {
                                            windows_style::badge_frame(BadgeTone::Neutral).show(
                                                ui,
                                                |ui| {
                                                    ui.add(
                                                        Image::from_texture(&texture)
                                                            .fit_to_exact_size(vec2(20.0, 20.0)),
                                                    );
                                                },
                                            );
                                        } else {
                                            windows_style::badge_frame(item.badge_tone()).show(
                                                ui,
                                                |ui| {
                                                    ui.label(windows_style::badge_text(
                                                        item.badge(),
                                                        item.badge_tone(),
                                                    ));
                                                },
                                            );
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(windows_style::body_text(item.primary_text()));
                                            ui.label(windows_style::muted_text(
                                                item.secondary_text(),
                                            ));
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if item.is_pinned() {
                                                    windows_style::badge_frame(BadgeTone::Warning)
                                                        .show(ui, |ui| {
                                                            ui.label(windows_style::badge_text(
                                                                "PIN",
                                                                BadgeTone::Warning,
                                                            ));
                                                        });
                                                }
                                            },
                                        );
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if index == self.selected && self.scroll_to_selected {
                                response.scroll_to_me(Some(egui::Align::Center));
                                self.scroll_to_selected = false;
                            }
                            if response.clicked() {
                                self.selected = index;
                                self.clipboard_editor = None;
                                self.preview_cache_key = None;
                            }
                            if response.double_clicked() {
                                activate = Some(index);
                            }
                            ui.add_space(8.0);
                        }
                    });
                if let Some(index) = activate {
                    self.selected = index;
                    self.preview_cache_key = None;
                    self.activate_selected();
                }
            })
        });
    }

    fn render_detail(&mut self, ui: &mut egui::Ui) {
        ui.push_id("detail_panel", |ui| {
            let action_height = 58.0;
            let gap = 10.0;
            let preview_height = (ui.available_height() - action_height - gap).max(260.0);

            if let Some(editor) = &mut self.clipboard_editor {
                ScrollArea::vertical()
                    .id_salt("clipboard_editor_scroll")
                    .max_height(preview_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        windows_style::panel_frame().show(ui, |ui| {
                            ui.label(windows_style::section_text("Edit Clipboard Entry"));
                            ui.label(windows_style::muted_text(
                                "Rename any entry. Text entries can also be edited inline.",
                            ));
                            ui.add_space(12.0);
                            ui.label(windows_style::muted_text("Display name"));
                            ui.add_sized(
                                [ui.available_width(), 34.0],
                                TextEdit::singleline(&mut editor.custom_name)
                                    .id_salt("clipboard_custom_name"),
                            );
                            if editor.is_text {
                                ui.add_space(10.0);
                                ui.label(windows_style::muted_text("Text content"));
                                ui.add_sized(
                                    [ui.available_width(), 260.0],
                                    TextEdit::multiline(&mut editor.content)
                                        .id_salt("clipboard_editor_content")
                                        .desired_rows(12),
                                );
                            }
                        });
                    });
            } else {
                self.refresh_preview_card_cache();
                let ctx = ui.ctx().clone();
                let preview = &self.preview_cache_card;
                let preview_state = &mut self.preview_state;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), preview_height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ScrollArea::vertical()
                            .id_salt("detail_preview_scroll")
                            .max_height(preview_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                windows_preview::render_preview_panel(
                                    ui,
                                    &ctx,
                                    preview_state,
                                    Some(preview),
                                );
                            });
                    },
                );
            }

            ui.add_space(gap);
            windows_style::panel_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    let enabled = self.selected_item().is_some();
                    match self.surface {
                        AppSurface::Search => {
                            if ui
                                .add_enabled(
                                    enabled,
                                    windows_style::action_button(
                                        self.selected_item()
                                            .map(DisplayItem::action_label)
                                            .unwrap_or("Open"),
                                    ),
                                )
                                .clicked()
                            {
                                self.activate_selected();
                            }
                            if ui
                                .add_enabled(enabled, windows_style::ghost_button("Copy"))
                                .clicked()
                            {
                                self.copy_selected();
                            }
                        }
                        AppSurface::Clipboard => {
                            if self.clipboard_editor.is_some() {
                                if ui.add(windows_style::action_button("Save")).clicked() {
                                    self.save_clipboard_edit();
                                }
                                if ui.add(windows_style::ghost_button("Cancel")).clicked() {
                                    self.cancel_clipboard_edit();
                                }
                            } else {
                                if ui
                                    .add_enabled(enabled, windows_style::action_button("Restore"))
                                    .clicked()
                                {
                                    self.activate_selected();
                                }
                                if ui
                                    .add_enabled(enabled, windows_style::ghost_button("Copy"))
                                    .clicked()
                                {
                                    self.copy_selected();
                                }
                                if ui
                                    .add_enabled(enabled, windows_style::ghost_button("Edit"))
                                    .clicked()
                                {
                                    self.begin_clipboard_edit();
                                }
                                if ui
                                    .add_enabled(enabled, windows_style::ghost_button("Remove"))
                                    .clicked()
                                {
                                    self.remove_selected_history_entry();
                                }
                            }
                        }
                        AppSurface::Settings => {}
                    }
                    if ui.add(windows_style::ghost_button("Refresh")).clicked() {
                        self.request_reload(true);
                    }
                });
            });
        });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (SettingsTab::General, "General"),
                (SettingsTab::Behavior, "Behavior"),
                (SettingsTab::Obsidian, "Obsidian"),
                (SettingsTab::Sync, "Sync"),
            ] {
                if ui
                    .add(windows_style::tab_button(label, self.settings_tab == tab))
                    .clicked()
                {
                    self.settings_tab = tab;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(windows_style::ghost_button("Back to search"))
                    .clicked()
                {
                    self.set_surface(AppSurface::Search);
                }
            });
        });
        ui.add_space(12.0);

        let content_height = (ui.available_height() - 76.0).max(160.0);
        ScrollArea::vertical()
            .id_salt("settings_scroll")
            .max_height(content_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                windows_style::panel_frame().show(ui, |ui| match self.settings_tab {
                    SettingsTab::General => {
                        ui.label(windows_style::section_text("Global Hotkey"));
                        ui.label(windows_style::muted_text(
                            "Keep parity with the macOS launcher shell. The value is stored here for the Windows frontend.",
                        ));
                        ui.add_space(12.0);
                        ui.add_sized(
                            [ui.available_width(), 36.0],
                            TextEdit::singleline(&mut self.hotkey),
                        );
                    }
                    SettingsTab::Behavior => {
                        ui.label(windows_style::section_text("Results and Dismissal"));
                        ui.label(windows_style::muted_text(
                            "Tune how much content the launcher loads, how aggressively it collapses, and how clipboard restore behaves on macOS.",
                        ));
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.label(windows_style::body_text(format!(
                                "Max results: {}",
                                self.max_results
                            )));
                            ui.add(Slider::new(&mut self.max_results, 10..=200));
                        });
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.dismiss_on_escape, "Dismiss on Escape");
                        ui.checkbox(&mut self.dismiss_on_click_away, "Dismiss on click away");
                        ui.checkbox(
                            &mut self.paste_after_restore,
                            "On macOS, paste immediately after restoring a clipboard item",
                        );
                    }
                    SettingsTab::Obsidian => {
                        ui.label(windows_style::section_text("Obsidian Vault"));
                        ui.label(windows_style::muted_text(
                            "Point Viceroy at an Obsidian vault so notes show up as first-class results with note-aware open behavior.",
                        ));
                        ui.add_space(12.0);
                        ui.checkbox(
                            &mut self.obsidian_enabled,
                            "Enable Obsidian note search",
                        );
                        ui.checkbox(
                            &mut self.obsidian_open_in_obsidian,
                            "Open note results in Obsidian when possible",
                        );
                        ui.add_space(8.0);
                        ui.label(windows_style::muted_text("Vault folder"));
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [ui.available_width() - 104.0, 34.0],
                                TextEdit::singleline(&mut self.obsidian_vault_path),
                            );
                            if ui.add(windows_style::ghost_button("Browse")).clicked() {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.obsidian_vault_path = folder.display().to_string();
                                    self.obsidian_message =
                                        "Selected an Obsidian vault folder. Save to apply it."
                                            .to_string();
                                }
                            }
                        });
                        ui.add_space(8.0);
                        settings_field(
                            ui,
                            "Vault name (optional)",
                            &mut self.obsidian_vault_name,
                            false,
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let has_vault = !self.obsidian_vault_path.trim().is_empty();
                            if ui
                                .add_enabled(
                                    has_vault,
                                    windows_style::ghost_button("Open folder"),
                                )
                                .clicked()
                            {
                                match app_launcher::open_file(self.obsidian_vault_path.trim()) {
                                    Ok(()) => {
                                        self.obsidian_message =
                                            "Opened the configured vault folder.".to_string();
                                    }
                                    Err(err) => {
                                        self.obsidian_message =
                                            format!("Failed to open the vault folder: {err:#}");
                                    }
                                }
                            }
                            if ui
                                .add_enabled(has_vault, windows_style::ghost_button("Clear"))
                                .clicked()
                            {
                                self.obsidian_vault_path.clear();
                                self.obsidian_vault_name.clear();
                                self.obsidian_enabled = false;
                                self.obsidian_message =
                                    "Cleared the configured Obsidian vault.".to_string();
                            }
                        });
                        if !self.obsidian_message.is_empty() {
                            ui.add_space(10.0);
                            ui.label(windows_style::muted_text(&self.obsidian_message));
                        }
                    }
                    SettingsTab::Sync => {
                        ui.label(windows_style::section_text("Cross-Device Sync"));
                        ui.label(windows_style::muted_text(
                            "Point Viceroy at your self-hosted sync server, test the connection, and keep an eye on every device tied to it.",
                        ));
                        ui.add_space(12.0);
                        ui.columns(2, |columns| {
                            columns[0].checkbox(&mut self.sync_enabled, "Enable sync");
                            columns[0].checkbox(
                                &mut self.sync_mirror_clipboard,
                                "Mirror latest synced item to this clipboard",
                            );
                            columns[0].add_space(8.0);
                            settings_field(
                                &mut columns[0],
                                "Device name",
                                &mut self.sync_device_name,
                                false,
                            );
                            settings_field(
                                &mut columns[0],
                                "Device id",
                                &mut self.sync_device_id,
                                true,
                            );
                            settings_field(
                                &mut columns[0],
                                "Server URL",
                                &mut self.sync_server_url,
                                false,
                            );
                            columns[0].label(windows_style::muted_text("Auth token"));
                            columns[0].add_sized(
                                [columns[0].available_width(), 34.0],
                                TextEdit::singleline(&mut self.sync_auth_token).password(true),
                            );

                            windows_style::card_frame(false).show(&mut columns[1], |ui| {
                                ui.set_min_width(ui.available_width());
                                let tone = sync_indicator_tone(
                                    self.sync_status.as_ref(),
                                    self.sync_test_result.as_ref(),
                                );
                                let heading = sync_indicator_heading(
                                    self.sync_status.as_ref(),
                                    self.sync_test_result.as_ref(),
                                );
                                ui.horizontal(|ui| {
                                    sync_status_dot(ui, tone);
                                    ui.label(windows_style::section_text(heading));
                                });
                                ui.add_space(6.0);
                                if let Some(status) = &self.sync_status {
                                    for line in [
                                        format!(
                                            "Current device: {} ({})",
                                            status.device.device_name, status.device.platform
                                        ),
                                        format!(
                                            "Server: {}",
                                            status.server_url.as_deref().unwrap_or("Not configured")
                                        ),
                                        format!(
                                            "Last successful sync: {}",
                                            sync::format_timestamp(status.last_successful_sync_at)
                                        ),
                                        format!("Pending operations: {}", status.pending_operations),
                                    ] {
                                        ui.label(windows_style::muted_text(line));
                                    }
                                } else {
                                    ui.label(windows_style::muted_text(
                                        "Sync status is not available yet.",
                                    ));
                                }
                                if let Some(status) = &self.sync_status {
                                    if let Some(error) = &status.last_error {
                                        ui.add_space(8.0);
                                        ui.label(windows_style::status_text(error, BadgeTone::Danger));
                                    }
                                }
                                if !self.sync_message.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(windows_style::status_text(
                                        &self.sync_message,
                                        sync_message_tone(
                                            self.sync_test_result.as_ref(),
                                            &self.sync_message,
                                        ),
                                    ));
                                }
                            });
                        });

                        ui.add_space(12.0);
                        windows_style::search_shell_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(windows_style::section_text("Devices"));
                                if let Some(status) = &self.sync_status {
                                    ui.label(windows_style::muted_text(format!(
                                        "{} known",
                                        status.known_devices.len()
                                    )));
                                }
                            });
                            ui.add_space(8.0);
                            let known_devices = self
                                .sync_status
                                .as_ref()
                                .map(|status| status.known_devices.as_slice())
                                .unwrap_or(&[]);
                            if known_devices.is_empty() {
                                ui.label(windows_style::muted_text(
                                    "No device roster is cached yet. Run Test connection or Refresh status to load it.",
                                ));
                            } else {
                                ScrollArea::vertical()
                                    .id_salt("sync_devices_scroll")
                                    .max_height(140.0)
                                    .show(ui, |ui| {
                                        for device in known_devices {
                                            windows_style::card_frame(device.is_current).show(
                                                ui,
                                                |ui| {
                                                    ui.set_min_width(ui.available_width());
                                                    ui.horizontal(|ui| {
                                                        let badge_tone = if device.is_current {
                                                            BadgeTone::Accent
                                                        } else {
                                                            BadgeTone::Neutral
                                                        };
                                                        windows_style::badge_frame(badge_tone).show(
                                                            ui,
                                                            |ui| {
                                                                ui.label(windows_style::badge_text(
                                                                    if device.is_current {
                                                                        "Current"
                                                                    } else {
                                                                        "Device"
                                                                    },
                                                                    badge_tone,
                                                                ));
                                                            },
                                                        );
                                                        ui.label(windows_style::body_text(
                                                            &device.device_name,
                                                        ));
                                                        ui.label(windows_style::muted_text(format!(
                                                            "({})",
                                                            device.platform
                                                        )));
                                                    });
                                                    ui.add_space(4.0);
                                                    ui.label(windows_style::muted_text(format!(
                                                        "Last seen: {}",
                                                        sync::format_timestamp(Some(
                                                            device.last_seen_at,
                                                        ))
                                                    )));
                                                },
                                            );
                                            ui.add_space(6.0);
                                        }
                                    });
                            }
                        });
                    }
                });
            });

        ui.add_space(12.0);
        windows_style::panel_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.settings_tab == SettingsTab::Sync {
                    if ui
                        .add(windows_style::ghost_button("Refresh status"))
                        .clicked()
                    {
                        self.refresh_sync_status();
                    }
                    if ui
                        .add(windows_style::ghost_button("Test connection"))
                        .clicked()
                    {
                        self.test_sync_connection();
                    }
                }
                if ui
                    .add(windows_style::action_button("Save settings"))
                    .clicked()
                {
                    self.save_settings();
                }
            });
        });
    }

    fn render_footer(&self, ui: &mut egui::Ui) {
        windows_style::panel_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for hint in footer_hints(self.surface) {
                    windows_style::badge_frame(BadgeTone::Neutral).show(ui, |ui| {
                        ui.label(windows_style::badge_text(*hint, BadgeTone::Neutral));
                    });
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let status_line = if let Some(msg) = &self.hotkey_message {
                        format!("Hotkey error: {msg}")
                    } else {
                        self.status.clone()
                    };
                    ui.label(windows_style::status_text(
                        status_line,
                        status_tone(&self.status),
                    ));
                });
            });
        });
    }
}

impl eframe::App for ViceroyWindowsApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0).to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Only send viewport focus when window is restored from minimized, not every frame.
        if self.focus_query_next_frame && !self.window_minimized && self.window_was_focused == false && self.focus_grace_frames > 0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Capture HWND for Win32 hotkey thread to be able to restore/focus the window.
        if let Ok(handle) = frame.window_handle() {
            let raw = handle.as_raw();
            if let raw_window_handle::RawWindowHandle::Win32(win) = raw {
                let hwnd = win.hwnd.get();
                crate::windows_hwnd::set(hwnd);
                crate::windows_dwm::apply_once(hwnd);
            }
        }

        // Apply any completed background work results.
        loop {
            match self.work_rx.try_recv() {
                Ok(resp) => {
                    // Only apply the latest inflight response, and only if it still matches the
                    // current UI surface/query.
                    if Some(resp.id) != self.inflight_work_id {
                        continue;
                    }
                    if resp.surface != self.surface || resp.query != self.query {
                        continue;
                    }

                    self.items = resp.items;
                    self.status = resp.status;
                    if resp.surface == AppSurface::Clipboard {
                        self.clipboard_total_count = resp.clipboard_total_count;
                    }

                    if self.selected >= self.items.len() {
                        self.selected = self.items.len().saturating_sub(1);
                        self.scroll_to_selected = true;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        // Drain global hotkey events.
        loop {
            match self.hotkey_events.try_recv() {
                Ok(HotkeyEvent::Pressed) => {
                    let now = Instant::now();
                    eprintln!("[hotkey] Pressed at {:?}", now);
                    eprintln!("[hotkey] State: minimized={}, was_focused={}, grace_frames={}, cooldown_until={:?}", self.window_minimized, self.window_was_focused, self.focus_grace_frames, self.hotkey_toggle_cooldown_until);
                    if self.hotkey_toggle_cooldown_until.is_some_and(|until| now < until) {
                        eprintln!("[hotkey] Ignored: cooldown");
                        continue;
                    }

                    let has_focus_now = ctx.input(|input| input.focused);
                    let is_minimized = self.window_minimized;
                    let can_hide = has_focus_now && !is_minimized && self.window_was_focused && self.focus_grace_frames == 0;

                    if can_hide {
                        eprintln!("[hotkey] Hiding launcher");
                        self.hide_launcher(ctx);
                        self.hotkey_toggle_cooldown_until = Some(now + Duration::from_millis(800));
                        continue;
                    }

                    // If already visible and not minimized, do nothing (prevents double-show crash)
                    if !is_minimized && has_focus_now {
                        eprintln!("[hotkey] Ignored: already showing and focused");
                        continue;
                    }

                    // Defensive: Only restore if not already restoring
                    if !is_minimized && !has_focus_now {
                        eprintln!("[hotkey] Warning: inconsistent state (not minimized, not focused)");
                    }

                    // Restore and focus the window
                    eprintln!("[hotkey] Restoring and focusing window");
                    self.window_minimized = false;
                    self.window_was_focused = false;
                    self.focus_grace_frames = 12;

                    // Extra guard: wrap in catch_unwind to log panics
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }));
                    if let Err(e) = result {
                        eprintln!("[hotkey] ERROR: panic during window restore: {:?}", e);
                        continue;
                    }

                    self.set_surface(AppSurface::Search);
                    self.query.clear();
                    self.items.clear();
                    self.selected = 0;
                    self.scroll_to_selected = true;
                    self.preview_cache_key = None;
                    self.status = "Search apps, files, clipboard snippets, commands, and the web.".to_string();
                    self.focus_query_next_frame = true;
                    self.pending_reload = true;
                    self.last_query_edit = Some(Instant::now());
                    self.hotkey_toggle_cooldown_until = Some(now + Duration::from_millis(800));
                }
                Ok(HotkeyEvent::Error(msg)) => {
                    // Treat this as a warning/status: keep listening so fallback hotkeys can still work.
                    self.hotkey_message = Some(msg);
                    continue;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        let mut reload = false;
        let editor_active = self.clipboard_editor.is_some();
        let settings_active = self.surface == AppSurface::Settings;
        let mut surface_request = None;
        let mut move_delta = 0isize;
        let mut activate = false;
        let mut handle_escape = false;
        let mut delete_history = false;
        let has_focus = ctx.input(|input| input.focused);

        // Suppress scroll-to-selected snapping while the user is actively scrolling
        // with the mouse wheel, so free scrolling through the list is not interrupted.
        let user_scrolling = ctx.input(|i| i.smooth_scroll_delta.y.abs() > 0.5);
        if user_scrolling {
            self.scroll_to_selected = false;
        }

        ctx.input(|input| {
            if input.modifiers.ctrl && input.key_pressed(Key::Comma) {
                surface_request = Some(AppSurface::Settings);
            }
            if !editor_active && !settings_active && input.key_pressed(Key::Tab) {
                surface_request = Some(if self.surface == AppSurface::Clipboard {
                    AppSurface::Search
                } else {
                    AppSurface::Clipboard
                });
            }
            if input.key_pressed(Key::Escape) {
                handle_escape = true;
            }
            if !editor_active && !settings_active {
                if input.key_pressed(Key::ArrowDown) {
                    move_delta += 1;
                }
                if input.key_pressed(Key::ArrowUp) {
                    move_delta -= 1;
                }
                if input.key_pressed(Key::Enter) {
                    activate = true;
                }
            }
            if !editor_active
                && self.surface == AppSurface::Clipboard
                && input.key_pressed(Key::Delete)
            {
                delete_history = true;
            }
        });

        if self.focus_grace_frames > 0 {
            self.focus_grace_frames = self.focus_grace_frames.saturating_sub(1);
        }

        if self.dismiss_on_click_away
            && self.focus_grace_frames == 0
            && self.window_was_focused
            && !has_focus
            && !self.window_minimized
        {
            self.hide_launcher(ctx);
        }

        // Only consider the window "was focused" after it actually receives focus.
        // This prevents hotkey-summon flicker where Windows reports !focused briefly.
        self.window_was_focused = self.window_was_focused || (has_focus && !self.window_minimized);

        if let Some(surface) = surface_request {
            self.set_surface(surface);
            self.focus_query_next_frame = self.surface != AppSurface::Settings;
            reload = true;
        }
        if handle_escape {
            self.handle_escape(ctx);
            reload = true;
        }
        if move_delta != 0 {
            self.move_selection(move_delta);
        }
        if activate {
            self.activate_selected();
        }
        if delete_history {
            self.remove_selected_history_entry();
        }

        // Capture the previous window size before syncing, so we can detect the
        // collapsed → full layout transition below.
        let prev_window_size = self.last_window_size;
        self.sync_window_geometry(ctx);
        let collapsed_search = self.is_collapsed_search();

        // When the layout transitions from collapsed (Spotlight-only, 960×124) to the
        // full dashboard, the TextEdit is rendered inside a different parent Frame,
        // giving it a new egui widget Id.  The old Id that had focus no longer exists,
        // so focus is silently dropped.  Re-arm the focus request here so the new Id
        // picks up focus on the very next frame without requiring a click.
        let collapsed_size = [960.0_f32, 124.0_f32];
        if prev_window_size == collapsed_size && !collapsed_search {
            self.focus_query_next_frame = true;
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                if collapsed_search {
                    // Slim Spotlight-style mode: just the search field, no outer dashboard shell.
                    self.render_search_box(ui);
                } else {
                    windows_style::launcher_shell_frame().show(ui, |ui| {
                        if self.surface == AppSurface::Settings {
                            self.render_header(ui);
                            ui.add_space(12.0);
                            let settings_height = (ui.available_height() - 48.0).max(360.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), settings_height),
                                Layout::top_down(Align::Min),
                                |ui| self.render_settings(ui),
                            );
                            ui.add_space(10.0);
                            self.render_footer(ui);
                        } else {
                            self.render_search_box(ui);
                            ui.add_space(12.0);
                            if self.surface == AppSurface::Clipboard {
                                self.render_header(ui);
                                ui.add_space(10.0);
                            }

                            // Reserve room for the footer before allocating the results/detail columns.
                            // Previously the columns consumed all available height, which made the
                            // footer and lower/right edges look cropped in Clipboard/Search modes.
                            let column_height = (ui.available_height() - 54.0).max(360.0);
                            let total_width = ui.available_width();
                            let gap = 14.0;
                            let left_width = (total_width * 0.54).clamp(430.0, total_width - 420.0);
                            let right_width = (total_width - left_width - gap).max(400.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(total_width, column_height),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(left_width, column_height),
                                            Layout::top_down(Align::Min),
                                            |ui| self.render_results(ui),
                                        );
                                        ui.add_space(gap);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(right_width, column_height),
                                            Layout::top_down(Align::Min),
                                            |ui| self.render_detail(ui),
                                        );
                                    });
                                },
                            );
                            ui.add_space(10.0);
                            self.render_footer(ui);
                        }
                    });
                }
            });

        // Reload policy:
        // - Force reloads for explicit actions/surface changes.
        // - Debounce query-driven reloads so typing stays snappy.
        // - Clipboard surface still polls for updates.
        // NOTE: request_reload is async (work thread). Avoid firing it every frame.
        // Only request work when:
        // - an explicit reload happened
        // - clipboard wants a periodic refresh
        // - the user stopped typing (debounce)
        if reload {
            self.pending_reload = false;
            self.request_reload(true);
        } else if self.surface == AppSurface::Clipboard {
            // Clipboard needs periodic refresh (new copies coming in).
            self.request_reload(false);
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if self.pending_reload {
            let ready = self
                .last_query_edit
                .map(|t| t.elapsed() >= Duration::from_millis(40))
                .unwrap_or(true);
            if ready {
                self.pending_reload = false;
                self.request_reload(false);
            }
        }
    }
}

fn history_title(entry: &ClipboardEntry) -> String {
    if entry.content_type == "image" {
        entry
            .custom_name
            .clone()
            .unwrap_or_else(|| match (entry.image_width, entry.image_height) {
                (Some(width), Some(height)) => format!("Image {}x{}", width, height),
                _ => "Clipboard image".to_string(),
            })
    } else {
        entry.custom_name.clone().unwrap_or_else(|| {
            let preview: String = entry.content.chars().take(60).collect();
            if preview.is_empty() {
                "Clipboard entry".to_string()
            } else {
                preview
            }
        })
    }
}

fn history_subtitle(entry: &ClipboardEntry) -> String {
    history_subtitle_from_fields(
        &entry.content_type,
        entry.app_name.as_ref(),
        entry.timestamp,
        entry.image_width,
        entry.image_height,
    )
}

fn history_subtitle_from_fields(
    content_type: &str,
    app_name: Option<&String>,
    timestamp: i64,
    image_width: Option<i64>,
    image_height: Option<i64>,
) -> String {
    let now = chrono::Utc::now().timestamp();
    let app = app_name
        .cloned()
        .unwrap_or_else(|| "Unknown app".to_string());
    let time = windows_preview::format_relative_time(timestamp, now);
    let detail = if content_type == "image" {
        match (image_width, image_height) {
            (Some(width), Some(height)) => format!("{}x{} px", width, height),
            _ => "Image".to_string(),
        }
    } else {
        "Text".to_string()
    };
    format!("{app} | {time} | {detail}")
}

fn settings_field(ui: &mut egui::Ui, label: &str, value: &mut String, readonly: bool) {
    ui.label(windows_style::muted_text(label));
    ui.add_sized(
        [ui.available_width(), 34.0],
        TextEdit::singleline(value).interactive(!readonly),
    );
    ui.add_space(8.0);
}

fn footer_hints(surface: AppSurface) -> &'static [&'static str] {
    match surface {
        AppSurface::Search => &[
            "Up/Down Navigate",
            "Tab Clipboard",
            "Ctrl+, Settings",
            "Enter Launch",
            "Esc Hide",
        ],
        AppSurface::Clipboard => &[
            "Up/Down Navigate",
            "Tab Search",
            "Delete Remove",
            "Enter Restore",
            "Esc Hide",
        ],
        AppSurface::Settings => &["Ctrl+, Settings", "Tab Clipboard", "Esc Back"],
    }
}

fn status_tone(message: &str) -> BadgeTone {
    let lower = message.to_lowercase();
    if lower.contains("failed") || lower.contains("invalid") || lower.contains("error") {
        BadgeTone::Danger
    } else if lower.contains("saved") || lower.contains("loaded") || lower.contains("enabled") {
        BadgeTone::Success
    } else {
        BadgeTone::Accent
    }
}

fn sync_indicator_tone(
    status: Option<&sync::SyncStatus>,
    test_result: Option<&sync::SyncConnectionTestResult>,
) -> BadgeTone {
    if let Some(result) = test_result {
        return if result.ok {
            BadgeTone::Success
        } else {
            match result.issue {
                sync::SyncConnectionTestIssue::None => BadgeTone::Success,
                sync::SyncConnectionTestIssue::ServerUnreachable => BadgeTone::Danger,
                sync::SyncConnectionTestIssue::AuthenticationFailed => BadgeTone::Danger,
                sync::SyncConnectionTestIssue::InvalidConfiguration => BadgeTone::Danger,
                sync::SyncConnectionTestIssue::UnexpectedResponse => BadgeTone::Warning,
            }
        };
    }

    match status.map(|status| &status.connection_state) {
        Some(sync::SyncConnectionState::Connected) => BadgeTone::Success,
        Some(sync::SyncConnectionState::Reconnecting) => BadgeTone::Warning,
        Some(sync::SyncConnectionState::Disabled) => BadgeTone::Neutral,
        Some(sync::SyncConnectionState::Disconnected) => BadgeTone::Danger,
        None => BadgeTone::Neutral,
    }
}

fn sync_indicator_heading(
    status: Option<&sync::SyncStatus>,
    test_result: Option<&sync::SyncConnectionTestResult>,
) -> &'static str {
    if let Some(result) = test_result {
        return if result.ok {
            "Connection healthy"
        } else {
            match result.issue {
                sync::SyncConnectionTestIssue::None => "Connection healthy",
                sync::SyncConnectionTestIssue::InvalidConfiguration => "Server URL needs attention",
                sync::SyncConnectionTestIssue::AuthenticationFailed => "Authentication failed",
                sync::SyncConnectionTestIssue::ServerUnreachable => "Server unreachable",
                sync::SyncConnectionTestIssue::UnexpectedResponse => "Server response needs review",
            }
        };
    }

    match status.map(|status| &status.connection_state) {
        Some(sync::SyncConnectionState::Connected) => "Connected",
        Some(sync::SyncConnectionState::Reconnecting) => "Reconnecting",
        Some(sync::SyncConnectionState::Disabled) => "Disabled",
        Some(sync::SyncConnectionState::Disconnected) => "Disconnected",
        None => "Status unavailable",
    }
}

fn sync_message_tone(
    test_result: Option<&sync::SyncConnectionTestResult>,
    message: &str,
) -> BadgeTone {
    if let Some(result) = test_result {
        return if result.ok {
            BadgeTone::Success
        } else {
            sync_indicator_tone(None, Some(result))
        };
    }
    status_tone(message)
}

fn sync_status_dot(ui: &mut egui::Ui, tone: BadgeTone) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), 4.0, tone.stroke());
}

fn execute_item(runtime: &Runtime, item: &DisplayItem) -> anyhow::Result<String> {
    match item {
        DisplayItem::Search(result) => execute_search_result(runtime, result),
        DisplayItem::History(entry) => {
            runtime.block_on(clipboard::restore_saved_history_entry_to_clipboard(
                entry.id,
                &entry.content,
                &entry.content_type,
                entry.image_width,
                entry.image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
    }
}

fn execute_search_result(runtime: &Runtime, result: &SearchResult) -> anyhow::Result<String> {
    match result {
        SearchResult::Link { url, host, .. } => {
            web_search::open_web_search(url)?;
            Ok(format!("Opened {host}"))
        }
        SearchResult::App { name, path, .. } => {
            usage::record_app_launch(path);
            app_launcher::launch(path)?;
            Ok(format!("Launched {name}"))
        }
        SearchResult::File { name, path, .. } => {
            app_launcher::open_file(path)?;
            Ok(format!("Opened {name}"))
        }
        SearchResult::Note {
            title,
            path,
            vault_name,
            ..
        } => {
            let obsidian_settings = settings::load().unwrap_or_default().obsidian;
            if obsidian_settings.open_in_obsidian {
                if let Some(vault_path) = obsidian_settings.vault_path {
                    obsidian::open_note_in_obsidian(
                        path,
                        &vault_path,
                        vault_name
                            .as_deref()
                            .or(obsidian_settings.vault_name.as_deref()),
                    )?;
                } else {
                    app_launcher::open_file(path)?;
                }
            } else {
                app_launcher::open_file(path)?;
            }
            Ok(format!("Opened note {title}"))
        }
        SearchResult::Clipboard {
            id,
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_saved_history_entry_to_clipboard(
                *id,
                content,
                content_type,
                *image_width,
                *image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
        SearchResult::Command { command, .. } => {
            runtime.block_on(system_commands::execute(command))
        }
        SearchResult::Calculator { result, .. } => {
            copy_text(result)?;
            Ok("Calculator result copied to the clipboard".to_string())
        }
        SearchResult::Emoji { emoji, .. } => {
            copy_text(emoji)?;
            Ok("Emoji copied to the clipboard".to_string())
        }
        SearchResult::Dictionary { word, .. } => {
            dictionary::open_dictionary(word)?;
            Ok(format!("Opened a definition for {word}"))
        }
        SearchResult::WebSearch { url, .. } => {
            web_search::open_web_search(url)?;
            Ok("Opened web search".to_string())
        }
    }
}

fn copy_item(runtime: &Runtime, item: &DisplayItem) -> anyhow::Result<String> {
    match item {
        DisplayItem::Search(result) => copy_search_result(runtime, result),
        DisplayItem::History(entry) => {
            runtime.block_on(clipboard::restore_saved_history_entry_to_clipboard(
                entry.id,
                &entry.content,
                &entry.content_type,
                entry.image_width,
                entry.image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
    }
}

fn copy_search_result(runtime: &Runtime, result: &SearchResult) -> anyhow::Result<String> {
    match result {
        SearchResult::Link { url, .. } => {
            copy_text(url)?;
            Ok("Link copied to the clipboard".to_string())
        }
        SearchResult::App { path, .. } | SearchResult::File { path, .. } => {
            copy_text(path)?;
            Ok("Path copied to the clipboard".to_string())
        }
        SearchResult::Note { path, .. } => {
            copy_text(path)?;
            Ok("Note path copied to the clipboard".to_string())
        }
        SearchResult::Clipboard {
            id,
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_saved_history_entry_to_clipboard(
                *id,
                content,
                content_type,
                *image_width,
                *image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
        SearchResult::Command { command, .. } => {
            copy_text(command)?;
            Ok("Command identifier copied to the clipboard".to_string())
        }
        SearchResult::Calculator { result, .. } => {
            copy_text(result)?;
            Ok("Calculator result copied to the clipboard".to_string())
        }
        SearchResult::Emoji { emoji, .. } => {
            copy_text(emoji)?;
            Ok("Emoji copied to the clipboard".to_string())
        }
        SearchResult::Dictionary { word, .. } => {
            copy_text(word)?;
            Ok("Dictionary term copied to the clipboard".to_string())
        }
        SearchResult::WebSearch { url, .. } => {
            copy_text(url)?;
            Ok("Search URL copied to the clipboard".to_string())
        }
    }
}

fn copy_text(value: &str) -> anyhow::Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(value.to_string())?;
    Ok(())
}

fn stable_preview_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

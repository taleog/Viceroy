use arboard::Clipboard;
use eframe::egui::{self, Align, Key, Layout, RichText, ScrollArea, Slider, TextEdit};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;
use viceroy::search_engine::{self, SearchResult};
use viceroy::{
    app_launcher,
    clipboard::{self, ClipboardEntry},
    database, dictionary, settings, sync, system_commands, updater, usage, web_search,
};

use crate::windows_preview::{self, PreviewCard, PreviewPanelState, PreviewSource};
use crate::windows_style::{self, BadgeTone};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppSurface {
    Search,
    Clipboard,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Behavior,
    Sync,
}

#[derive(Clone)]
enum DisplayItem {
    Search(SearchResult),
    History(ClipboardEntry),
}

impl DisplayItem {
    fn primary_text(&self) -> String {
        match self {
            Self::Search(result) => match result {
                SearchResult::App { name, .. } => name.clone(),
                SearchResult::File { name, .. } => name.clone(),
                SearchResult::Clipboard {
                    custom_name,
                    preview,
                    ..
                } => custom_name.clone().unwrap_or_else(|| preview.clone()),
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
                SearchResult::App { .. } => "APP",
                SearchResult::File { .. } => "FILE",
                SearchResult::Clipboard { content_type, .. } => {
                    if content_type == "image" {
                        "IMAGE"
                    } else {
                        "CLIP"
                    }
                }
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
                SearchResult::App { .. } => BadgeTone::Accent,
                SearchResult::File { .. } => BadgeTone::Neutral,
                SearchResult::Clipboard { .. } => BadgeTone::Accent,
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
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([940.0, 640.0])
            .with_title("Viceroy"),
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
        runtime.block_on(async {
            if let Err(err) = clipboard::start_monitor().await {
                eprintln!("Clipboard monitor error: {err}");
            }
        });
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
    status: String,
    hotkey: String,
    max_results: usize,
    dismiss_on_escape: bool,
    dismiss_on_click_away: bool,
    sync_enabled: bool,
    sync_device_name: String,
    sync_device_id: String,
    sync_server_url: String,
    sync_auth_token: String,
    sync_status: Option<sync::SyncStatus>,
    sync_test_result: Option<sync::SyncConnectionTestResult>,
    sync_message: String,
    settings_tab: SettingsTab,
    clipboard_editor: Option<ClipboardEditor>,
    preview_state: PreviewPanelState,
}

impl ViceroyWindowsApp {
    fn new(cc: &eframe::CreationContext<'_>, runtime: Arc<Runtime>, initial_query: String) -> Self {
        windows_style::apply_launcher_theme(&cc.egui_ctx);
        let app_settings = settings::load().unwrap_or_default();

        let mut app = Self {
            runtime,
            query: initial_query,
            items: Vec::new(),
            selected: 0,
            surface: AppSurface::Search,
            last_loaded_query: String::new(),
            last_loaded_surface: AppSurface::Settings,
            status: "Search apps, files, clipboard snippets, commands, and the web.".to_string(),
            hotkey: app_settings.hotkey.clone(),
            max_results: app_settings.max_results,
            dismiss_on_escape: app_settings.dismiss_on_escape,
            dismiss_on_click_away: app_settings.dismiss_on_click_away,
            sync_enabled: app_settings.sync.enabled,
            sync_device_name: app_settings.sync.device_name.clone(),
            sync_device_id: app_settings.sync.device_id.clone(),
            sync_server_url: app_settings.sync.server_url.unwrap_or_default(),
            sync_auth_token: app_settings.sync.auth_token.unwrap_or_default(),
            sync_status: None,
            sync_test_result: None,
            sync_message: String::new(),
            settings_tab: SettingsTab::General,
            clipboard_editor: None,
            preview_state: PreviewPanelState::new(),
        };
        app.refresh_sync_status();
        app.reload_items(true);
        app
    }

    fn set_surface(&mut self, surface: AppSurface) {
        if self.surface != surface {
            self.surface = surface;
            self.selected = 0;
            if surface != AppSurface::Clipboard {
                self.clipboard_editor = None;
            }
        }
    }

    fn reload_items(&mut self, force: bool) {
        if self.surface == AppSurface::Settings {
            self.items.clear();
            self.last_loaded_query = self.query.clone();
            self.last_loaded_surface = self.surface;
            return;
        }
        if !force
            && self.last_loaded_query == self.query
            && self.last_loaded_surface == self.surface
        {
            return;
        }

        self.items.clear();
        self.clipboard_editor = None;
        let limit = self.max_results.clamp(10, 200);

        match self.surface {
            AppSurface::Search => {
                if self.query.trim().is_empty() {
                    self.status =
                        "Start typing to search apps, files, clipboard snippets, commands, and the web."
                            .to_string();
                } else {
                    match self.runtime.block_on(search_engine::search(&self.query)) {
                        Ok(mut results) => {
                            results.truncate(results.len().min(limit));
                            self.items = results.into_iter().map(DisplayItem::Search).collect();
                            self.status = if self.items.is_empty() {
                                format!("No results for \"{}\".", self.query)
                            } else {
                                format!("{} results for \"{}\".", self.items.len(), self.query)
                            };
                        }
                        Err(err) => self.status = format!("Search failed: {err:#}"),
                    }
                }
            }
            AppSurface::Clipboard => {
                let result = if self.query.trim().is_empty() {
                    self.runtime.block_on(clipboard::get_history(limit))
                } else {
                    self.runtime
                        .block_on(clipboard::search_history(&self.query))
                };
                match result {
                    Ok(mut entries) => {
                        entries.truncate(entries.len().min(limit));
                        self.items = entries.into_iter().map(DisplayItem::History).collect();
                        self.status = if self.items.is_empty() {
                            if self.query.trim().is_empty() {
                                "Clipboard history is empty.".to_string()
                            } else {
                                format!("No clipboard entries match \"{}\".", self.query)
                            }
                        } else {
                            format!("Showing {} clipboard entries.", self.items.len())
                        };
                    }
                    Err(err) => self.status = format!("Clipboard load failed: {err:#}"),
                }
            }
            AppSurface::Settings => {}
        }

        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.last_loaded_query = self.query.clone();
        self.last_loaded_surface = self.surface;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.items.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
    }

    fn selected_item(&self) -> Option<&DisplayItem> {
        self.items.get(self.selected)
    }

    fn preview_card(&self) -> PreviewCard {
        match self.selected_item() {
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
                self.reload_items(true);
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
                self.reload_items(true);
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
                let old_enabled = app_settings.sync.enabled;
                let old_server_url = app_settings.sync.server_url.clone().unwrap_or_default();
                let old_auth_token = app_settings.sync.auth_token.clone().unwrap_or_default();

                let normalized_server_url = if self.sync_enabled {
                    let input = self.sync_server_url.trim();
                    if input.is_empty() {
                        self.sync_message =
                            "Enter a sync server URL before enabling sync.".to_string();
                        return;
                    }
                    match sync::normalize_server_url(input) {
                        Ok(url) => {
                            if let Err(err) = sync::validate_server_url_for_local_device(&url) {
                                self.sync_message = format!("Invalid sync server URL: {err:#}");
                                return;
                            }
                            url
                        }
                        Err(err) => {
                            self.sync_message = format!("Invalid sync server URL: {err:#}");
                            return;
                        }
                    }
                } else {
                    self.sync_server_url.trim().to_string()
                };

                app_settings.hotkey = self.hotkey.trim().to_string();
                app_settings.max_results = self.max_results.clamp(10, 200);
                app_settings.dismiss_on_escape = self.dismiss_on_escape;
                app_settings.dismiss_on_click_away = self.dismiss_on_click_away;
                app_settings.sync.enabled = self.sync_enabled;
                app_settings.sync.device_name = self.sync_device_name.trim().to_string();
                app_settings.sync.server_url = non_empty(normalized_server_url.trim());
                app_settings.sync.auth_token = non_empty(self.sync_auth_token.trim());
                self.sync_server_url = normalized_server_url;

                if let Err(err) = settings::save(&app_settings) {
                    self.sync_message = format!("Failed to save settings: {err:#}");
                    return;
                }
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
                    "Sync settings saved. Restart Viceroy to apply server URL or token changes."
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
                self.status = "Settings saved.".to_string();
                self.reload_items(true);
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
        if !self.query.is_empty() {
            self.query.clear();
            self.reload_items(true);
            self.status = "Cleared the current query.".to_string();
            return;
        }
        if self.surface == AppSurface::Clipboard {
            self.set_surface(AppSurface::Search);
            self.reload_items(true);
            return;
        }
        if self.dismiss_on_escape {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(windows_style::title_text("Viceroy"));
                ui.label(windows_style::muted_text(match self.surface {
                    AppSurface::Search => {
                        "Launcher-style search for apps, files, clipboard, commands, and the web."
                    }
                    AppSurface::Clipboard => {
                        "Clipboard history with preview, rename, and restore actions."
                    }
                    AppSurface::Settings => {
                        "Mac-style settings tabs for general behavior and sync."
                    }
                }));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(windows_style::pill_button(RichText::new(format!(
                        "Hotkey {}",
                        if self.hotkey.trim().is_empty() {
                            "Alt+Space"
                        } else {
                            self.hotkey.trim()
                        }
                    ))))
                    .clicked()
                {
                    self.set_surface(AppSurface::Settings);
                }
                for (surface, label) in [
                    (AppSurface::Settings, "Settings"),
                    (AppSurface::Clipboard, "Clipboard"),
                    (AppSurface::Search, "Search"),
                ] {
                    if ui
                        .add(windows_style::tab_button(label, self.surface == surface))
                        .clicked()
                    {
                        self.set_surface(surface);
                    }
                }
            });
        });
    }

    fn render_search_box(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        windows_style::card_frame(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                windows_style::badge_frame(BadgeTone::Accent).show(ui, |ui| {
                    ui.label(windows_style::badge_text(
                        if self.surface == AppSurface::Clipboard {
                            "FILTER"
                        } else {
                            "SEARCH"
                        },
                        BadgeTone::Accent,
                    ));
                });
                let response = ui.add_sized(
                    [ui.available_width() - 80.0, 38.0],
                    TextEdit::singleline(&mut self.query)
                        .id_salt("launcher_query")
                        .frame(false)
                        .hint_text(if self.surface == AppSurface::Clipboard {
                            "Filter clipboard history"
                        } else {
                            "Search apps, files, clipboard snippets, commands, and the web"
                        }),
                );
                changed |= response.changed();
                if !self.query.is_empty() && ui.add(windows_style::ghost_button("Clear")).clicked()
                {
                    self.query.clear();
                    changed = true;
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
                        ui.label(windows_style::muted_text(format!(
                            "{} items",
                            self.items.len()
                        )));
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
                                        windows_style::badge_frame(item.badge_tone()).show(
                                            ui,
                                            |ui| {
                                                ui.label(windows_style::badge_text(
                                                    item.badge(),
                                                    item.badge_tone(),
                                                ));
                                            },
                                        );
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
                            if response.clicked() {
                                self.selected = index;
                                self.clipboard_editor = None;
                            }
                            if response.double_clicked() {
                                activate = Some(index);
                            }
                            ui.add_space(8.0);
                        }
                    });
                if let Some(index) = activate {
                    self.selected = index;
                    self.activate_selected();
                }
            })
        });
    }

    fn render_detail(&mut self, ui: &mut egui::Ui) {
        ui.push_id("detail_panel", |ui| {
            if let Some(editor) = &mut self.clipboard_editor {
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
                            [ui.available_width(), 300.0],
                            TextEdit::multiline(&mut editor.content)
                                .id_salt("clipboard_editor_content")
                                .desired_rows(14),
                        );
                    }
                });
            } else {
                let preview = self.preview_card();
                let ctx = ui.ctx().clone();
                windows_preview::render_preview_panel(
                    ui,
                    &ctx,
                    &mut self.preview_state,
                    Some(&preview),
                );
            }

            ui.add_space(12.0);
            windows_style::panel_frame().show(ui, |ui| {
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
                        self.reload_items(true);
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
                    "Tune how much content the launcher loads and how aggressively it collapses back out of the way.",
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
                ui.checkbox(
                    &mut self.dismiss_on_click_away,
                    "Dismiss on click away",
                );
            }
            SettingsTab::Sync => {
                ui.label(windows_style::section_text("Cross-Device Sync"));
                ui.label(windows_style::muted_text(
                    "Point Viceroy at your self-hosted sync server, test the connection, and keep an eye on every device tied to it.",
                ));
                ui.add_space(12.0);
                ui.columns(2, |columns| {
                    columns[0].checkbox(&mut self.sync_enabled, "Enable sync");
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
                                sync_message_tone(self.sync_test_result.as_ref(), &self.sync_message),
                            ));
                        }
                    });
                });

                ui.add_space(12.0);
                windows_style::card_frame(false).show(ui, |ui| {
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
                            .max_height(180.0)
                            .show(ui, |ui| {
                                for device in known_devices {
                                    windows_style::card_frame(device.is_current).show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            let badge_tone = if device.is_current {
                                                BadgeTone::Accent
                                            } else {
                                                BadgeTone::Neutral
                                            };
                                            windows_style::badge_frame(badge_tone).show(ui, |ui| {
                                                ui.label(windows_style::badge_text(
                                                    if device.is_current {
                                                        "Current"
                                                    } else {
                                                        "Device"
                                                    },
                                                    badge_tone,
                                                ));
                                            });
                                            ui.label(windows_style::body_text(&device.device_name));
                                            ui.label(windows_style::muted_text(format!(
                                                "({})",
                                                device.platform
                                            )));
                                        });
                                        ui.add_space(4.0);
                                        ui.label(windows_style::muted_text(format!(
                                            "Last seen: {}",
                                            sync::format_timestamp(Some(device.last_seen_at))
                                        )));
                                    });
                                    ui.add_space(6.0);
                                }
                            });
                    }
                });
            }
        });

        ui.add_space(12.0);
        windows_style::panel_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
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
                    ui.label(windows_style::status_text(
                        &self.status,
                        status_tone(&self.status),
                    ));
                });
            });
        });
    }
}

impl eframe::App for ViceroyWindowsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut reload = false;
        let editor_active = self.clipboard_editor.is_some();
        let settings_active = self.surface == AppSurface::Settings;
        let mut surface_request = None;
        let mut move_delta = 0isize;
        let mut activate = false;
        let mut handle_escape = false;
        let mut delete_history = false;

        ctx.input(|input| {
            if input.modifiers.ctrl && input.key_pressed(Key::Num1) {
                surface_request = Some(AppSurface::Search);
            }
            if input.modifiers.ctrl && input.key_pressed(Key::Num2) {
                surface_request = Some(AppSurface::Clipboard);
            }
            if input.modifiers.ctrl && input.key_pressed(Key::Comma) {
                surface_request = Some(AppSurface::Settings);
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

        if let Some(surface) = surface_request {
            self.set_surface(surface);
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

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(windows_style::WINDOW_BG_ELEVATED)
                    .inner_margin(windows_style::WINDOW_PADDING),
            )
            .show(ctx, |ui| {
                windows_style::panel_frame().show(ui, |ui| {
                    self.render_header(ui);
                    ui.add_space(16.0);
                    if self.surface != AppSurface::Settings {
                        reload |= self.render_search_box(ui);
                        ui.add_space(16.0);
                        ui.columns(2, |columns| {
                            self.render_results(&mut columns[0]);
                            self.render_detail(&mut columns[1]);
                        });
                    } else {
                        self.render_settings(ui);
                    }
                    ui.add_space(16.0);
                    self.render_footer(ui);
                });
            });

        if reload {
            self.reload_items(true);
        } else {
            self.reload_items(false);
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
            "Up/Down move",
            "Enter open",
            "Ctrl+2 clipboard",
            "Esc dismiss",
        ],
        AppSurface::Clipboard => &[
            "Up/Down move",
            "Enter restore",
            "Delete remove",
            "Ctrl+, settings",
        ],
        AppSurface::Settings => &["Ctrl+1 search", "Ctrl+2 clipboard", "Esc back"],
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
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
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
        SearchResult::App { name, path, .. } => {
            usage::record_app_launch(path);
            app_launcher::launch(path)?;
            Ok(format!("Launched {name}"))
        }
        SearchResult::File { name, path, .. } => {
            app_launcher::open_file(path)?;
            Ok(format!("Opened {name}"))
        }
        SearchResult::Clipboard {
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
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
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
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
        SearchResult::App { path, .. } | SearchResult::File { path, .. } => {
            copy_text(path)?;
            Ok("Path copied to the clipboard".to_string())
        }
        SearchResult::Clipboard {
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
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

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

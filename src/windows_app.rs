use arboard::Clipboard;
use eframe::egui::{
    self, Align, Color32, Key, Layout, RichText, ScrollArea, Sense, TextEdit, Vec2,
};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;
use viceroy::search_engine::{self, SearchResult};
use viceroy::{
    app_launcher, clipboard, database, dictionary, settings, sync, system_commands, updater, usage,
    web_search,
};

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
            .with_inner_size([920.0, 680.0])
            .with_min_inner_size([720.0, 520.0])
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

#[derive(Clone)]
enum DisplayItem {
    Search(SearchResult),
    History(clipboard::ClipboardEntry),
}

impl DisplayItem {
    fn primary_text(&self) -> String {
        match self {
            DisplayItem::Search(result) => match result {
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
            DisplayItem::History(entry) => {
                if entry.content_type == "image" {
                    entry
                        .custom_name
                        .clone()
                        .unwrap_or_else(|| "Clipboard image".to_string())
                } else {
                    entry.custom_name.clone().unwrap_or_else(|| {
                        let preview: String = entry.content.chars().take(100).collect();
                        if preview.is_empty() {
                            "Clipboard entry".to_string()
                        } else {
                            preview
                        }
                    })
                }
            }
        }
    }

    fn secondary_text(&self) -> String {
        match self {
            DisplayItem::Search(result) => match result {
                SearchResult::App { path, .. } => path.clone(),
                SearchResult::File { path, .. } => path.clone(),
                SearchResult::Clipboard {
                    content_type,
                    app_name,
                    ..
                } => match app_name {
                    Some(app) => format!("Clipboard {content_type} from {app}"),
                    None => format!("Clipboard {content_type}"),
                },
                SearchResult::Command { description, .. } => description.clone(),
                SearchResult::Calculator { .. } => "Calculator".to_string(),
                SearchResult::Emoji { keywords, .. } => keywords.join(", "),
                SearchResult::Dictionary { preview, .. } => preview.clone(),
                SearchResult::WebSearch { url, .. } => url.clone(),
            },
            DisplayItem::History(entry) => match &entry.app_name {
                Some(app) => format!("{} from {}", content_label(&entry.content_type), app),
                None => content_label(&entry.content_type).to_string(),
            },
        }
    }

    fn badge(&self) -> &'static str {
        match self {
            DisplayItem::Search(result) => match result {
                SearchResult::App { .. } => "APP",
                SearchResult::File { .. } => "FILE",
                SearchResult::Clipboard { .. } => "CLIP",
                SearchResult::Command { .. } => "CMD",
                SearchResult::Calculator { .. } => "CALC",
                SearchResult::Emoji { .. } => "EMOJI",
                SearchResult::Dictionary { .. } => "DICT",
                SearchResult::WebSearch { .. } => "WEB",
            },
            DisplayItem::History(entry) => {
                if entry.content_type == "image" {
                    "IMAGE"
                } else {
                    "CLIP"
                }
            }
        }
    }

    fn action_label(&self) -> &'static str {
        match self {
            DisplayItem::Search(SearchResult::Clipboard { .. }) | DisplayItem::History(_) => "Copy",
            DisplayItem::Search(SearchResult::Calculator { .. })
            | DisplayItem::Search(SearchResult::Emoji { .. }) => "Copy",
            _ => "Open",
        }
    }
}

fn content_label(content_type: &str) -> &'static str {
    if content_type == "image" {
        "Clipboard image"
    } else {
        "Clipboard text"
    }
}

struct ViceroyWindowsApp {
    runtime: Arc<Runtime>,
    query: String,
    items: Vec<DisplayItem>,
    selected: usize,
    show_history: bool,
    last_loaded_query: String,
    last_loaded_history: bool,
    status: String,
    sync_enabled: bool,
    sync_device_name: String,
    sync_device_id: String,
    sync_server_url: String,
    sync_auth_token: String,
    sync_status: Option<sync::SyncStatus>,
    sync_message: String,
}

impl ViceroyWindowsApp {
    fn new(cc: &eframe::CreationContext<'_>, runtime: Arc<Runtime>, initial_query: String) -> Self {
        apply_theme(&cc.egui_ctx);

        let (sync_enabled, sync_device_name, sync_device_id, sync_server_url, sync_auth_token) =
            match settings::load() {
                Ok(app_settings) => (
                    app_settings.sync.enabled,
                    app_settings.sync.device_name,
                    app_settings.sync.device_id,
                    app_settings.sync.server_url.unwrap_or_default(),
                    app_settings.sync.auth_token.unwrap_or_default(),
                ),
                Err(_) => (
                    false,
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            };

        let mut app = Self {
            runtime,
            query: initial_query,
            items: Vec::new(),
            selected: 0,
            show_history: false,
            last_loaded_query: String::new(),
            last_loaded_history: false,
            status: "Type to search apps, files, clipboard history, commands, and the web."
                .to_string(),
            sync_enabled,
            sync_device_name,
            sync_device_id,
            sync_server_url,
            sync_auth_token,
            sync_status: None,
            sync_message: String::new(),
        };
        app.reload_items(true);
        app.refresh_sync_status();
        app
    }

    fn reload_items(&mut self, force: bool) {
        if !force
            && self.last_loaded_query == self.query
            && self.last_loaded_history == self.show_history
        {
            return;
        }

        self.items.clear();

        if self.show_history {
            match self.runtime.block_on(clipboard::get_history(50)) {
                Ok(entries) => {
                    self.items = entries.into_iter().map(DisplayItem::History).collect();
                    self.status = if self.items.is_empty() {
                        "Clipboard history is empty.".to_string()
                    } else {
                        format!("Showing {} clipboard entries.", self.items.len())
                    };
                }
                Err(err) => {
                    self.status = format!("Failed to load clipboard history: {err:#}");
                }
            }
        } else if self.query.trim().is_empty() {
            self.status =
                "Type to search. Use Clipboard History to browse saved clips.".to_string();
        } else {
            match self.runtime.block_on(search_engine::search(&self.query)) {
                Ok(results) => {
                    self.items = results.into_iter().map(DisplayItem::Search).collect();
                    self.status = if self.items.is_empty() {
                        format!("No results for \"{}\".", self.query)
                    } else {
                        format!("{} results for \"{}\".", self.items.len(), self.query)
                    };
                }
                Err(err) => {
                    self.status = format!("Search failed: {err:#}");
                }
            }
        }

        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
        self.last_loaded_query = self.query.clone();
        self.last_loaded_history = self.show_history;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }

        let max_index = self.items.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index);
        self.selected = next as usize;
    }

    fn selected_item(&self) -> Option<&DisplayItem> {
        self.items.get(self.selected)
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

    fn refresh_sync_status(&mut self) {
        match sync::status() {
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

    fn save_sync_settings(&mut self) {
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

                app_settings.sync.enabled = self.sync_enabled;
                app_settings.sync.device_name = self.sync_device_name.trim().to_string();
                app_settings.sync.server_url = non_empty(normalized_server_url.trim());
                app_settings.sync.auth_token = non_empty(self.sync_auth_token.trim());
                self.sync_server_url = normalized_server_url;

                if let Err(err) = settings::save(&app_settings) {
                    self.sync_message = format!("Failed to save sync settings: {err:#}");
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
            }
            Err(err) => {
                self.sync_message = format!("Failed to load current settings: {err:#}");
            }
        }
    }
}

impl eframe::App for ViceroyWindowsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut query_changed = false;
        let mut history_toggled = false;

        ctx.input(|input| {
            if input.key_pressed(Key::ArrowDown) {
                self.move_selection(1);
            }
            if input.key_pressed(Key::ArrowUp) {
                self.move_selection(-1);
            }
            if input.key_pressed(Key::Enter) {
                self.activate_selected();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Viceroy").size(30.0).strong());
                ui.add_space(10.0);
                ui.label(RichText::new("Windows").color(Color32::from_rgb(120, 190, 255)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [150.0, 32.0],
                            egui::Button::new(if self.show_history {
                                "Back To Search"
                            } else {
                                "Clipboard History"
                            }),
                        )
                        .clicked()
                    {
                        self.show_history = !self.show_history;
                        history_toggled = true;
                    }
                });
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new("A simple Windows GUI for the shared Viceroy backend.")
                    .color(Color32::from_gray(180)),
            );
            ui.add_space(14.0);

            egui::CollapsingHeader::new("Sync Settings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.sync_enabled, "Enable sync");
                        if ui.button("Refresh Status").clicked() {
                            self.refresh_sync_status();
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Device name");
                        ui.add_sized(
                            [260.0, 28.0],
                            TextEdit::singleline(&mut self.sync_device_name),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Device id");
                        ui.add_sized(
                            [460.0, 28.0],
                            TextEdit::singleline(&mut self.sync_device_id).interactive(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Server URL");
                        ui.add_sized(
                            [460.0, 28.0],
                            TextEdit::singleline(&mut self.sync_server_url)
                                .hint_text("https://sync.example.com"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Auth token");
                        ui.add_sized(
                            [460.0, 28.0],
                            TextEdit::singleline(&mut self.sync_auth_token).password(true),
                        );
                    });

                    ui.add_space(8.0);
                    if ui.button("Save Sync Settings").clicked() {
                        self.save_sync_settings();
                    }

                    ui.add_space(8.0);
                    if let Some(status) = &self.sync_status {
                        ui.label(
                            RichText::new(format!(
                                "Current device: {} ({})",
                                status.device.device_name, status.device.platform
                            ))
                            .color(Color32::from_gray(190)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Connection: {}",
                                status.connection_state.display_label()
                            ))
                            .color(Color32::from_gray(190)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Server: {}",
                                status.server_url.as_deref().unwrap_or("Not configured")
                            ))
                            .color(Color32::from_gray(190)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Last successful sync: {}",
                                sync::format_timestamp(status.last_successful_sync_at)
                            ))
                            .color(Color32::from_gray(190)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Pending outbox operations: {}",
                                status.pending_operations
                            ))
                            .color(Color32::from_gray(190)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Last error: {}",
                                status.last_error.as_deref().unwrap_or("None")
                            ))
                            .color(if status.last_error.is_some() {
                                Color32::from_rgb(255, 145, 145)
                            } else {
                                Color32::from_gray(190)
                            }),
                        );
                    } else {
                        ui.label(
                            RichText::new("Sync status is not available yet.")
                                .color(Color32::from_gray(190)),
                        );
                    }
                    if !self.sync_message.is_empty() {
                        ui.label(
                            RichText::new(&self.sync_message)
                                .color(Color32::from_rgb(130, 195, 255)),
                        );
                    }
                });

            ui.add_space(14.0);

            let search = ui.add_sized(
                [ui.available_width(), 42.0],
                TextEdit::singleline(&mut self.query)
                    .hint_text("Search apps, files, clipboard history, commands, and more"),
            );
            if search.changed() {
                if self.show_history {
                    self.show_history = false;
                    history_toggled = true;
                }
                query_changed = true;
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.selected_item().is_some(),
                        egui::Button::new(
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
                    .add_enabled(self.selected_item().is_some(), egui::Button::new("Copy"))
                    .clicked()
                {
                    self.copy_selected();
                }

                if ui.button("Refresh").clicked() {
                    self.reload_items(true);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(&self.status).color(Color32::from_gray(170)));
                });
            });

            ui.add_space(14.0);

            if query_changed || history_toggled {
                self.reload_items(true);
            } else {
                self.reload_items(false);
            }

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.items.is_empty() {
                        ui.add_space(30.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if self.show_history {
                                    "No clipboard entries yet"
                                } else {
                                    "No results yet"
                                })
                                .size(22.0)
                                .color(Color32::from_gray(180)),
                            );
                        });
                        return;
                    }

                    let mut activate_index = None;

                    for index in 0..self.items.len() {
                        let item = self.items[index].clone();
                        let is_selected = index == self.selected;
                        let fill = if is_selected {
                            Color32::from_rgb(33, 48, 78)
                        } else {
                            Color32::from_rgb(20, 24, 32)
                        };

                        egui::Frame::new()
                            .fill(fill)
                            .stroke(egui::Stroke::new(
                                1.0,
                                if is_selected {
                                    Color32::from_rgb(92, 162, 255)
                                } else {
                                    Color32::from_rgb(42, 47, 60)
                                },
                            ))
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                let response = ui
                                    .allocate_ui_with_layout(
                                        Vec2::new(ui.available_width(), 54.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(item.badge())
                                                    .size(11.0)
                                                    .color(Color32::from_rgb(120, 190, 255)),
                                            );
                                            ui.add_space(14.0);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(item.primary_text())
                                                        .size(16.0)
                                                        .strong(),
                                                );
                                                ui.label(
                                                    RichText::new(item.secondary_text())
                                                        .size(12.0)
                                                        .color(Color32::from_gray(170)),
                                                );
                                            });
                                        },
                                    )
                                    .response
                                    .interact(Sense::click());

                                if response.clicked() {
                                    self.selected = index;
                                }
                                if response.double_clicked() {
                                    self.selected = index;
                                    activate_index = Some(index);
                                }
                            });

                        ui.add_space(8.0);
                    }

                    if let Some(index) = activate_index {
                        self.selected = index;
                        self.activate_selected();
                    }
                });
        });
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(12, 14, 20);
    visuals.extreme_bg_color = Color32::from_rgb(17, 20, 28);
    visuals.widgets.active.bg_fill = Color32::from_rgb(39, 68, 120);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(28, 36, 52);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(23, 27, 35);
    visuals.selection.bg_fill = Color32::from_rgb(37, 76, 145);
    visuals.override_text_color = Some(Color32::from_rgb(236, 240, 248));
    ctx.set_visuals(visuals);
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

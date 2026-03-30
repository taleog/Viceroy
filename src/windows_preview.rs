use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Local, LocalResult, TimeZone, Utc};
use eframe::egui::{
    self, Color32, ColorImage, Frame, RichText, ScrollArea, Stroke, TextureHandle, TextureOptions,
    Ui, Vec2,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use viceroy::clipboard;
use viceroy::search_engine::SearchResult;

const PANEL_FILL: Color32 = Color32::from_rgb(16, 20, 28);
const PANEL_STROKE: Color32 = Color32::from_rgb(42, 48, 62);
const TITLE_COLOR: Color32 = Color32::from_rgb(242, 245, 250);
const SUBTITLE_COLOR: Color32 = Color32::from_rgb(174, 182, 196);
const ACCENT_COLOR: Color32 = Color32::from_rgb(120, 190, 255);
const CHIP_FILL: Color32 = Color32::from_rgb(24, 30, 41);
const CHIP_STROKE: Color32 = Color32::from_rgb(53, 60, 76);
const EMPTY_HINT_COLOR: Color32 = Color32::from_rgb(160, 168, 180);

const MAX_TEXT_PREVIEW_CHARS: usize = 5_000;
const MAX_BODY_CHARS: usize = 10_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewMetadata {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct PreviewImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl PreviewImage {
    pub fn from_base64_png(encoded: &str) -> Option<Self> {
        let png_bytes = STANDARD.decode(encoded).ok()?;
        let cursor = Cursor::new(png_bytes);
        let decoder = png::Decoder::new(cursor);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).ok()?;
        buf.truncate(info.buffer_size());
        Some(Self {
            width: info.width as usize,
            height: info.height as usize,
            rgba: buf,
        })
    }

    fn cache_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.width.hash(&mut hasher);
        self.height.hash(&mut hasher);
        self.rgba.hash(&mut hasher);
        hasher.finish()
    }

    fn to_color_image(&self) -> Option<ColorImage> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        Some(ColorImage::from_rgba_unmultiplied(
            [self.width, self.height],
            &self.rgba,
        ))
    }
}

#[derive(Clone, Debug)]
pub enum PreviewBody {
    Empty {
        message: String,
    },
    Text {
        text: String,
        monospace: bool,
    },
    Image {
        image: PreviewImage,
        caption: String,
    },
}

impl PreviewBody {
    fn is_empty(&self) -> bool {
        matches!(self, Self::Empty { .. })
    }
}

#[derive(Clone, Debug)]
pub struct PreviewCard {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub metadata: Vec<PreviewMetadata>,
    pub body: PreviewBody,
    pub footer: Option<String>,
}

impl PreviewCard {
    pub fn empty(message: impl Into<String>) -> Self {
        Self {
            badge: "PREVIEW".to_string(),
            title: "Preview".to_string(),
            subtitle: String::new(),
            metadata: Vec::new(),
            body: PreviewBody::Empty {
                message: message.into(),
            },
            footer: None,
        }
    }

    pub fn from_clipboard_entry(entry: &clipboard::ClipboardEntry) -> Self {
        let now = Utc::now().timestamp();
        let badge = if entry.content_type == "image" {
            "IMAGE"
        } else {
            "CLIP"
        };
        let time_label = format_relative_time(entry.timestamp, now);
        let subtitle = match entry.app_name.as_deref() {
            Some(app) => format!("{app} | {time_label}"),
            None => time_label.clone(),
        };

        let mut metadata = Vec::new();
        metadata.push(PreviewMetadata {
            label: "Time".to_string(),
            value: time_label.clone(),
        });
        metadata.push(PreviewMetadata {
            label: "Source".to_string(),
            value: entry
                .app_name
                .clone()
                .unwrap_or_else(|| "Unknown app".to_string()),
        });
        metadata.extend(clipboard_metadata(entry));

        let title = entry.custom_name.clone().unwrap_or_else(|| {
            if entry.content_type == "image" {
                clipboard_image_title(entry.image_width, entry.image_height)
            } else {
                clip_text(&entry.content, 60)
            }
        });

        if entry.content_type == "image" {
            let body = PreviewImage::from_base64_png(&entry.content).map_or_else(
                || PreviewBody::Empty {
                    message: "Image preview could not be decoded.".to_string(),
                },
                |image| PreviewBody::Image {
                    caption: clipboard_image_caption(entry.image_width, entry.image_height),
                    image,
                },
            );

            return Self {
                badge: badge.to_string(),
                title,
                subtitle,
                metadata,
                body,
                footer: Some("Stored as PNG in the clipboard history".to_string()),
            };
        }

        Self {
            badge: badge.to_string(),
            title,
            subtitle,
            metadata,
            body: PreviewBody::Text {
                text: clip_text(&entry.content, MAX_TEXT_PREVIEW_CHARS),
                monospace: false,
            },
            footer: Some(format!("{} characters", entry.content.chars().count())),
        }
    }

    pub fn from_search_result(result: &SearchResult) -> Self {
        match result {
            SearchResult::App { name, path, .. } => Self {
                badge: "APP".to_string(),
                title: name.clone(),
                subtitle: path.clone(),
                metadata: vec![PreviewMetadata {
                    label: "Path".to_string(),
                    value: path.clone(),
                }],
                body: PreviewBody::Empty {
                    message: "Application results do not have a content preview.".to_string(),
                },
                footer: Some("Press Enter to launch".to_string()),
            },
            SearchResult::File { name, path, .. } => Self {
                badge: "FILE".to_string(),
                title: name.clone(),
                subtitle: path.clone(),
                metadata: vec![PreviewMetadata {
                    label: "Path".to_string(),
                    value: path.clone(),
                }],
                body: PreviewBody::Empty {
                    message: "File results do not have a content preview.".to_string(),
                },
                footer: Some("Press Enter to open".to_string()),
            },
            SearchResult::Clipboard {
                content,
                preview,
                content_type,
                app_name,
                timestamp,
                custom_name,
                is_pinned,
                image_width,
                image_height,
                ..
            } => {
                let entry = clipboard::ClipboardEntry {
                    id: 0,
                    content: content.clone(),
                    content_type: content_type.clone(),
                    app_name: app_name.clone(),
                    timestamp: *timestamp,
                    custom_name: custom_name.clone(),
                    is_favorite: false,
                    is_pinned: *is_pinned,
                    image_width: *image_width,
                    image_height: *image_height,
                };
                let mut card = Self::from_clipboard_entry(&entry);
                if entry.content_type != "image" && !preview.is_empty() {
                    card.body = PreviewBody::Text {
                        text: clip_text(preview, MAX_TEXT_PREVIEW_CHARS),
                        monospace: false,
                    };
                    card.footer = Some(format!("{} characters", content.chars().count()));
                }
                card
            }
            SearchResult::Command {
                name,
                description,
                command,
                ..
            } => Self {
                badge: "CMD".to_string(),
                title: name.clone(),
                subtitle: description.clone(),
                metadata: vec![
                    PreviewMetadata {
                        label: "Command".to_string(),
                        value: command.clone(),
                    },
                    PreviewMetadata {
                        label: "Description".to_string(),
                        value: description.clone(),
                    },
                ],
                body: PreviewBody::Text {
                    text: command.clone(),
                    monospace: true,
                },
                footer: Some("Press Enter to run".to_string()),
            },
            SearchResult::Calculator {
                expression,
                result,
                formats,
            } => Self {
                badge: "CALC".to_string(),
                title: format!("{expression} = {result}"),
                subtitle: "Calculator result".to_string(),
                metadata: vec![
                    PreviewMetadata {
                        label: "Expression".to_string(),
                        value: expression.clone(),
                    },
                    PreviewMetadata {
                        label: "Result".to_string(),
                        value: result.clone(),
                    },
                ],
                body: PreviewBody::Text {
                    text: formats.join("\n"),
                    monospace: true,
                },
                footer: Some("Press Enter to copy".to_string()),
            },
            SearchResult::Emoji {
                emoji,
                name,
                keywords,
            } => Self {
                badge: "EMOJI".to_string(),
                title: format!("{emoji} {name}"),
                subtitle: keywords.join(", "),
                metadata: vec![PreviewMetadata {
                    label: "Keywords".to_string(),
                    value: keywords.join(", "),
                }],
                body: PreviewBody::Text {
                    text: emoji.clone(),
                    monospace: false,
                },
                footer: Some("Press Enter to copy".to_string()),
            },
            SearchResult::Dictionary { word, preview } => Self {
                badge: "DICT".to_string(),
                title: word.clone(),
                subtitle: preview.clone(),
                metadata: vec![PreviewMetadata {
                    label: "Preview".to_string(),
                    value: preview.clone(),
                }],
                body: PreviewBody::Text {
                    text: preview.clone(),
                    monospace: false,
                },
                footer: Some("Dictionary lookup".to_string()),
            },
            SearchResult::WebSearch { query, engine, url } => Self {
                badge: "WEB".to_string(),
                title: query.clone(),
                subtitle: engine.clone(),
                metadata: vec![
                    PreviewMetadata {
                        label: "Engine".to_string(),
                        value: engine.clone(),
                    },
                    PreviewMetadata {
                        label: "URL".to_string(),
                        value: url.clone(),
                    },
                ],
                body: PreviewBody::Text {
                    text: url.clone(),
                    monospace: true,
                },
                footer: Some("Press Enter to open".to_string()),
            },
        }
    }
}

#[derive(Default)]
pub struct PreviewPanelState {
    texture_key: Option<u64>,
    texture: Option<TextureHandle>,
}

impl PreviewPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    fn texture_for_image<'a>(
        &'a mut self,
        ctx: &egui::Context,
        image: &PreviewImage,
    ) -> Option<&'a TextureHandle> {
        let key = image.cache_key();
        if self.texture_key != Some(key) {
            let color_image = image.to_color_image()?;
            self.texture = Some(ctx.load_texture(
                format!("clipboard-preview-{key}"),
                color_image,
                TextureOptions::LINEAR,
            ));
            self.texture_key = Some(key);
        }
        self.texture.as_ref()
    }
}

pub enum PreviewSource<'a> {
    SearchResult(&'a SearchResult),
    ClipboardEntry(&'a clipboard::ClipboardEntry),
}

impl<'a> PreviewSource<'a> {
    pub fn from_search_result(result: &'a SearchResult) -> Self {
        Self::SearchResult(result)
    }

    pub fn from_clipboard_entry(entry: &'a clipboard::ClipboardEntry) -> Self {
        Self::ClipboardEntry(entry)
    }
}

pub fn format_relative_time(timestamp: i64, now: i64) -> String {
    let delta = (now - timestamp).max(0);
    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        let local_time = match Local.timestamp_opt(timestamp, 0) {
            LocalResult::Single(dt) => dt,
            _ => Local::now(),
        };
        local_time.format("%b %d").to_string()
    }
}

pub fn preview_card(source: PreviewSource<'_>) -> PreviewCard {
    match source {
        PreviewSource::SearchResult(result) => PreviewCard::from_search_result(result),
        PreviewSource::ClipboardEntry(entry) => PreviewCard::from_clipboard_entry(entry),
    }
}

pub fn render_preview_panel(
    ui: &mut Ui,
    ctx: &egui::Context,
    state: &mut PreviewPanelState,
    preview: Option<&PreviewCard>,
) {
    ui.push_id("preview_panel", |ui| {
        Frame::new()
            .fill(PANEL_FILL)
            .stroke(Stroke::new(1.0, PANEL_STROKE))
            .corner_radius(20.0)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if let Some(preview) = preview {
                    render_preview_card(ui, ctx, state, preview);
                } else {
                    render_empty_preview(ui);
                }
            });
    });
}

pub fn render_preview_card(
    ui: &mut Ui,
    ctx: &egui::Context,
    state: &mut PreviewPanelState,
    preview: &PreviewCard,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(&preview.badge)
                .size(11.0)
                .strong()
                .color(ACCENT_COLOR),
        );
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(&preview.title)
            .size(22.0)
            .strong()
            .color(TITLE_COLOR),
    );
    if !preview.subtitle.is_empty() {
        ui.add_space(2.0);
        ui.label(
            RichText::new(&preview.subtitle)
                .size(12.5)
                .color(SUBTITLE_COLOR),
        );
    }

    if !preview.metadata.is_empty() {
        ui.add_space(14.0);
        render_metadata_row(ui, &preview.metadata);
    }

    if !preview.body.is_empty() {
        ui.add_space(16.0);
        match &preview.body {
            PreviewBody::Empty { message } => {
                ui.label(RichText::new(message).color(EMPTY_HINT_COLOR));
            }
            PreviewBody::Text { text, monospace } => {
                render_text_preview(ui, text, *monospace);
            }
            PreviewBody::Image { image, caption } => {
                render_image_preview(ui, ctx, state, image, caption);
            }
        }
    }

    if let Some(footer) = &preview.footer {
        ui.add_space(16.0);
        ui.label(
            RichText::new(footer)
                .size(11.5)
                .color(Color32::from_rgb(130, 196, 255)),
        );
    }
}

pub fn clipboard_metadata(entry: &clipboard::ClipboardEntry) -> Vec<PreviewMetadata> {
    let mut metadata = Vec::new();
    if entry.content_type == "image" {
        if let (Some(width), Some(height)) = (entry.image_width, entry.image_height) {
            metadata.push(PreviewMetadata {
                label: "Dimensions".to_string(),
                value: format!("{width} x {height} px"),
            });
        }
        metadata.push(PreviewMetadata {
            label: "Type".to_string(),
            value: "Image".to_string(),
        });
    } else {
        metadata.push(PreviewMetadata {
            label: "Type".to_string(),
            value: "Text".to_string(),
        });
        metadata.push(PreviewMetadata {
            label: "Length".to_string(),
            value: format!("{} chars", entry.content.chars().count()),
        });
    }

    if entry.is_pinned {
        metadata.push(PreviewMetadata {
            label: "Pinned".to_string(),
            value: "Yes".to_string(),
        });
    }
    if entry.is_favorite {
        metadata.push(PreviewMetadata {
            label: "Favorite".to_string(),
            value: "Yes".to_string(),
        });
    }

    metadata
}

fn render_metadata_row(ui: &mut Ui, metadata: &[PreviewMetadata]) {
    ui.horizontal_wrapped(|ui| {
        for item in metadata {
            render_metadata_chip(ui, item);
            ui.add_space(6.0);
        }
    });
}

fn render_metadata_chip(ui: &mut Ui, item: &PreviewMetadata) {
    Frame::new()
        .fill(CHIP_FILL)
        .stroke(Stroke::new(1.0, CHIP_STROKE))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&item.label).size(10.0).color(ACCENT_COLOR));
                ui.label(RichText::new(&item.value).size(10.5).color(TITLE_COLOR));
            });
        });
}

fn render_text_preview(ui: &mut Ui, text: &str, monospace: bool) {
    ScrollArea::vertical()
        .id_salt("preview_text_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let text = clip_text(text, MAX_BODY_CHARS);
            let rich_text = if monospace {
                RichText::new(text).monospace().size(13.0)
            } else {
                RichText::new(text).size(13.0)
            };
            ui.label(rich_text.color(TITLE_COLOR));
        });
}

fn render_image_preview(
    ui: &mut Ui,
    ctx: &egui::Context,
    state: &mut PreviewPanelState,
    image: &PreviewImage,
    caption: &str,
) {
    if let Some(texture) = state.texture_for_image(ctx, image) {
        let available_width = ui.available_width().max(1.0);
        let native_size = Vec2::new(image.width as f32, image.height as f32);
        let scale = (available_width / native_size.x.max(1.0)).min(1.0);
        let display_size = native_size * scale;

        ui.centered_and_justified(|ui| {
            ui.add(egui::Image::new(texture).fit_to_exact_size(display_size));
        });

        if !caption.is_empty() {
            ui.add_space(10.0);
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(caption).size(11.5).color(SUBTITLE_COLOR));
            });
        }
    } else {
        ui.label(RichText::new("Image preview unavailable").color(EMPTY_HINT_COLOR));
        if !caption.is_empty() {
            ui.label(RichText::new(caption).color(SUBTITLE_COLOR));
        }
    }
}

fn render_empty_preview(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        ui.label(
            RichText::new("Clipboard Preview")
                .size(22.0)
                .strong()
                .color(TITLE_COLOR),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("Select an item to inspect its contents and metadata.")
                .size(13.0)
                .color(EMPTY_HINT_COLOR),
        );
    });
}

fn clip_text(value: &str, limit: usize) -> String {
    let mut clipped = String::new();
    let mut count = 0usize;
    for ch in value.chars() {
        if count >= limit {
            clipped.push_str("...");
            break;
        }
        clipped.push(ch);
        count += 1;
    }
    clipped
}

fn clipboard_image_title(width: Option<i64>, height: Option<i64>) -> String {
    match (width, height) {
        (Some(w), Some(h)) => format!("Image | {w} x {h} px"),
        _ => "Image".to_string(),
    }
}

fn clipboard_image_caption(width: Option<i64>, height: Option<i64>) -> String {
    match (width, height) {
        (Some(w), Some(h)) => format!("{w} x {h} px"),
        _ => "Clipboard image".to_string(),
    }
}

#[allow(dead_code)]
pub fn preview_card_from_clipboard_entry(entry: &clipboard::ClipboardEntry) -> PreviewCard {
    PreviewCard::from_clipboard_entry(entry)
}

#[allow(dead_code)]
pub fn preview_card_from_search_result(result: &SearchResult) -> PreviewCard {
    PreviewCard::from_search_result(result)
}

#[allow(dead_code)]
pub fn preview_card_from_source(source: PreviewSource<'_>) -> PreviewCard {
    preview_card(source)
}

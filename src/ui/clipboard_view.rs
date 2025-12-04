use crate::app_launcher;
use crate::clipboard;
use crate::search_engine;
use crate::ui::helpers::run_on_main;
use crate::ui::helpers::style;
use crate::ui::state::{
    ClipboardPreviewRefs, TableMode, CLIPBOARD_PREVIEW, ICON_CACHE, SEARCH_RT, TABLE_DATA,
    TABLE_MODE, TABLE_RESULTS,
};
use crate::ui::table::{reload_table, schedule_table_update_next_tick};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Local, LocalResult, TimeZone, Utc};
use cocoa::base::{id, nil, NO, YES, BOOL};
use cocoa::foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::fmt::Write;

const MAX_PREVIEW_CHARS: usize = 5000;
const PREVIEW_HEADER_HEIGHT: f64 = 86.0;

pub fn format_clipboard_relative_time(timestamp: i64, now: i64) -> String {
    let delta = (now - timestamp).max(0);
    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        let local_time = match Local.timestamp_opt(timestamp, 0) {
            LocalResult::Single(dt) => dt,
            _ => Local::now(),
        };
        local_time.format("%b %d").to_string()
    }
}

fn truncate_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

pub fn build_clipboard_history_payload(
    entries: Vec<clipboard::ClipboardEntry>,
) -> (Vec<(String, String)>, Vec<search_engine::SearchResult>) {
    let now = Utc::now().timestamp();
    let mut rows = Vec::with_capacity(entries.len());
    let mut results = Vec::with_capacity(entries.len());

    for entry in entries.into_iter() {
        let app_label = entry
            .app_name
            .clone()
            .unwrap_or_else(|| "Unknown App".to_string());
        let time_label = format_clipboard_relative_time(entry.timestamp, now);
        let detail_label = if entry.content_type == "image" {
            if let (Some(width), Some(height)) = (entry.image_width, entry.image_height) {
                format!("{}×{} px", width, height)
            } else {
                "Image".to_string()
            }
        } else {
            format!("{} chars", entry.content.chars().count())
        };
        let subtitle = format!("{} · {} · {}", app_label, time_label, detail_label);

        let image_title = if entry.content_type == "image" {
            if detail_label == "Image" {
                "Image".to_string()
            } else {
                format!("Image · {}", detail_label)
            }
        } else {
            String::new()
        };

        let preview = if entry.content_type == "image" {
            entry
                .custom_name
                .clone()
                .unwrap_or_else(|| image_title.clone())
        } else {
            truncate_text(&entry.content, 100)
        };

        let title = entry.custom_name.clone().unwrap_or_else(|| {
            if entry.content_type == "image" {
                image_title.clone()
            } else {
                truncate_text(&entry.content, 60)
            }
        });

        rows.push((title, subtitle));
        results.push(search_engine::SearchResult::Clipboard {
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
            score: 0,
        });
    }

    (rows, results)
}

pub fn apply_clipboard_history_state(
    rows: Vec<(String, String)>,
    results: Vec<search_engine::SearchResult>,
) {
    let select_first = !rows.is_empty();
    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::ClipboardHistory;
    }
    if let Ok(mut tr) = TABLE_RESULTS.lock() {
        *tr = results;
    }
    if let Ok(mut td) = TABLE_DATA.lock() {
        *td = rows;
    }
    unsafe {
        reload_table();
        schedule_table_update_next_tick();
    }
    crate::ui::table::update_preview_layout(true);
    let selection = if select_first { Some(0) } else { None };
    update_clipboard_preview_selection(selection);
}

pub fn show_clipboard_history_view() {
    SEARCH_RT.spawn(async move {
        match clipboard::get_history(200).await {
            Ok(entries) => {
                let (rows, results) = build_clipboard_history_payload(entries);
                run_on_main(move || {
                    apply_clipboard_history_state(rows, results);
                });
            }
            Err(err) => {
                eprintln!("Failed to load clipboard history: {}", err);
            }
        }
    });
}

fn preview_refs() -> Option<ClipboardPreviewRefs> {
    CLIPBOARD_PREVIEW.get().cloned()
}

unsafe fn id_from(ptr: usize) -> id {
    ptr as id
}

unsafe fn set_hidden(view: id, hidden: bool) {
    let _: () = msg_send![view, setHidden: if hidden { YES } else { NO }];
}

unsafe fn set_string(view: id, value: &str) {
    let ns_string = NSString::alloc(nil).init_str(value);
    let _: () = msg_send![view, setStringValue: ns_string];
}

fn placeholder_text_for_mode(mode: TableMode) -> &'static str {
    match mode {
        TableMode::ClipboardHistory => "Select a clipboard entry to preview",
        TableMode::Search | TableMode::Settings => "Open clipboard history (Tab) to see previews",
    }
}

fn preview_data_for_row(row: usize) -> Option<search_engine::SearchResult> {
    let results = TABLE_RESULTS.lock().ok()?;
    results.get(row).cloned()
}

fn detail_label_for_entry(
    entry: &search_engine::SearchResult,
) -> Option<(String, String, Option<String>)> {
    if let search_engine::SearchResult::Clipboard {
        content,
        content_type,
        app_name,
        timestamp,
        custom_name,
        ..
    } = entry
    {
        let now = Utc::now().timestamp();
        let time_label = format_clipboard_relative_time(*timestamp, now);
        let app_label = app_name
            .clone()
            .unwrap_or_else(|| "Unknown App".to_string());
        let detail = if content_type == "image" {
            "Image content".to_string()
        } else {
            format!("{} chars", content.chars().count())
        };
        let title = custom_name.clone().unwrap_or_else(|| {
            if content_type == "image" {
                "Clipboard Image".to_string()
            } else {
                truncate_text(content, 80)
            }
        });
        let subtitle = format!("{} · {} · {}", app_label, time_label, detail);
        let text_body = if content_type == "image" {
            None
        } else {
            let (mut body_text, total_chars, truncated) = truncated_preview_body(content);
            if truncated {
                let _ = write!(
                    body_text,
                    "\n… (showing first {} of {} characters)",
                    MAX_PREVIEW_CHARS, total_chars
                );
            }
            Some(body_text)
        };
        return Some((title, subtitle, text_body));
    }
    None
}

fn truncated_preview_body(content: &str) -> (String, usize, bool) {
    let mut chars = content.chars();
    let mut preview = String::new();
    let mut taken = 0;
    while taken < MAX_PREVIEW_CHARS {
        match chars.next() {
            Some(ch) => {
                preview.push(ch);
                taken += 1;
            }
            None => return (preview, taken, false),
        }
    }
    let mut total = taken;
    for _ in chars {
        total += 1;
    }
    (preview, total, true)
}

fn set_placeholder_for_mode(mode: TableMode) {
    if let Some(refs) = preview_refs() {
        unsafe {
            let root = id_from(refs.root);
            if mode != TableMode::ClipboardHistory {
                set_hidden(root, true);
                return;
            }
            set_hidden(root, false);
            let title = id_from(refs.title_field);
            let detail = id_from(refs.detail_field);
            let placeholder = id_from(refs.placeholder_field);
            let text_scroll = id_from(refs.text_scroll);
            let image_view = id_from(refs.image_view);
            let text_background = id_from(refs.text_background);
            set_hidden(title, true);
            set_hidden(detail, true);
            set_hidden(text_scroll, true);
            set_hidden(image_view, true);
            set_hidden(text_background, true);
            set_hidden(placeholder, false);
            set_string(placeholder, placeholder_text_for_mode(mode));
        }
    }
}

fn show_text_preview(title: &str, subtitle: &str, body: &str) {
    if let Some(refs) = preview_refs() {
        unsafe {
            let title_field = id_from(refs.title_field);
            let detail_field = id_from(refs.detail_field);
            let placeholder = id_from(refs.placeholder_field);
            let text_scroll = id_from(refs.text_scroll);
            let text_view = id_from(refs.text_view);
            let image_view = id_from(refs.image_view);
            let text_background = id_from(refs.text_background);

            set_hidden(placeholder, true);
            set_hidden(image_view, true);
            set_hidden(text_scroll, false);
            set_hidden(title_field, false);
            set_hidden(detail_field, false);
            set_hidden(text_background, false);

            set_string(title_field, title);
            set_string(detail_field, subtitle);
            let ns_body = NSString::alloc(nil).init_str(body);
            let _: () = msg_send![text_view, setString: ns_body];
            reset_text_scroll_position(text_scroll, text_view);
        }
    }
}

fn show_image_preview(title: &str, subtitle: &str, image: id) {
    if let Some(refs) = preview_refs() {
        unsafe {
            let preview_root = id_from(refs.root);
            let title_field = id_from(refs.title_field);
            let detail_field = id_from(refs.detail_field);
            let placeholder = id_from(refs.placeholder_field);
            let text_scroll = id_from(refs.text_scroll);
            let image_view = id_from(refs.image_view);
            let text_background = id_from(refs.text_background);

            set_hidden(placeholder, true);
            set_hidden(text_scroll, true);
            set_hidden(image_view, false);
            set_hidden(title_field, false);
            set_hidden(detail_field, false);
            set_hidden(text_background, true);

            set_string(title_field, title);
            set_string(detail_field, subtitle);
            layout_image_preview(preview_root, image_view, image);
            let _: () = msg_send![image_view, setImage: image];
        }
    }
}

pub fn update_clipboard_preview_selection(row: Option<usize>) {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    if row.is_none() {
        set_placeholder_for_mode(mode);
        return;
    }
    if mode != TableMode::ClipboardHistory {
        set_placeholder_for_mode(mode);
        return;
    }
    let selected_row = row.unwrap();
    if let Some(entry) = preview_data_for_row(selected_row) {
        if let Some((title, subtitle, maybe_text)) = detail_label_for_entry(&entry) {
            if let search_engine::SearchResult::Clipboard {
                content,
                content_type,
                ..
            } = entry
            {
                if content_type == "image" {
                    if let Some(image) = image_from_clipboard_content(&content) {
                        show_image_preview(&title, &subtitle, image);
                        return;
                    }
                } else if let Some(text_body) = maybe_text {
                    show_text_preview(&title, &subtitle, &text_body);
                    return;
                }
            }
        }
    }
    set_placeholder_for_mode(mode);
}

fn reset_text_scroll_position(text_scroll: id, text_view: id) {
    unsafe {
        let start = NSRange::new(0, 0);
        let _: () = msg_send![text_view, setSelectedRange: start];
        let _: () = msg_send![text_view, scrollRangeToVisible: start];
        let clip_view: id = msg_send![text_scroll, contentView];
        if clip_view == nil {
            return;
        }
        let doc_bounds: NSRect = msg_send![text_view, bounds];
        let clip_bounds: NSRect = msg_send![clip_view, bounds];
        let is_flipped: BOOL = msg_send![text_view, isFlipped];
        let target_y = if is_flipped == YES {
            0.0
        } else {
            (doc_bounds.size.height - clip_bounds.size.height).max(0.0)
        };
        let _: () = msg_send![clip_view, scrollToPoint: NSPoint::new(0.0, target_y)];
        let _: () = msg_send![text_scroll, reflectScrolledClipView: clip_view];
    }
}

fn preview_content_frame(bounds: NSRect) -> NSRect {
    let content_inset = style::PREVIEW_CONTENT_INSET;
    let width = (bounds.size.width - content_inset * 2.0).max(120.0);
    let height = (bounds.size.height - PREVIEW_HEADER_HEIGHT - content_inset).max(140.0);
    NSRect::new(
        NSPoint::new(content_inset, content_inset),
        NSSize::new(width, height),
    )
}

unsafe fn apply_preview_subview_layout(bounds: NSRect, refs: &ClipboardPreviewRefs) {
    let text_area_frame = preview_content_frame(bounds);
    let title_field = id_from(refs.title_field);
    let detail_field = id_from(refs.detail_field);
    let placeholder = id_from(refs.placeholder_field);
    let text_scroll = id_from(refs.text_scroll);
    let text_view = id_from(refs.text_view);
    let image_view = id_from(refs.image_view);
    let text_background = id_from(refs.text_background);

    let text_width = text_area_frame.size.width.max(40.0);
    let text_size = NSSize::new((text_width - 20.0).max(40.0), text_area_frame.size.height);
    let placeholder_height = 80.0;
    let content_inset = style::PREVIEW_CONTENT_INSET;
    let label_width = (bounds.size.width - content_inset * 2.0).max(40.0);
    let title_height = 26.0;
    let detail_height = 20.0;
    let title_origin_y = bounds.size.height - content_inset - title_height;
    let detail_origin_y = (title_origin_y - detail_height - 4.0).max(content_inset);
    let placeholder_origin_y =
        text_area_frame.origin.y + (text_area_frame.size.height - placeholder_height) / 2.0;

    let title_frame = NSRect::new(
        NSPoint::new(content_inset, title_origin_y),
        NSSize::new(label_width, title_height),
    );
    let detail_frame = NSRect::new(
        NSPoint::new(content_inset, detail_origin_y),
        NSSize::new(label_width, detail_height),
    );
    let placeholder_frame = NSRect::new(
        NSPoint::new(text_area_frame.origin.x, placeholder_origin_y),
        NSSize::new(text_area_frame.size.width, placeholder_height),
    );

    let _: () = msg_send![text_background, setFrame: text_area_frame];
    let _: () = msg_send![text_scroll, setFrame: text_area_frame];
    let _: () = msg_send![text_view, setFrame:NSRect::new(NSPoint::new(0.0, 0.0), text_size)];
    let _: () = msg_send![image_view, setFrame: text_area_frame];
    let _: () = msg_send![placeholder, setFrame: placeholder_frame];
    let _: () = msg_send![title_field, setFrame: title_frame];
    let _: () = msg_send![detail_field, setFrame: detail_frame];
}

pub fn refresh_clipboard_preview_layout() {
    if let Some(refs) = preview_refs() {
        unsafe {
            let preview = id_from(refs.root);
            if preview == nil {
                return;
            }
            let bounds: NSRect = msg_send![preview, bounds];
            apply_preview_subview_layout(bounds, &refs);
        }
    }
}

fn layout_image_preview(preview: id, image_view: id, image: id) {
    unsafe {
        let bounds: NSRect = msg_send![preview, bounds];
        let area_frame = preview_content_frame(bounds);
        let area_width = area_frame.size.width.max(60.0);
        let area_height = area_frame.size.height.max(60.0);

        let raw_size: NSSize = msg_send![image, size];
        let mut draw_width = area_width;
        let mut draw_height = area_height;
        if raw_size.width > 0.0 && raw_size.height > 0.0 {
            let scale =
                f64::min(area_width / raw_size.width, area_height / raw_size.height).max(0.01);
            draw_width = (raw_size.width * scale).min(area_width);
            draw_height = (raw_size.height * scale).min(area_height);
        }

        let origin_x = area_frame.origin.x + (area_width - draw_width) / 2.0;
        let origin_y = area_frame.origin.y + (area_height - draw_height) / 2.0;
        let frame = NSRect::new(
            NSPoint::new(origin_x, origin_y),
            NSSize::new(draw_width, draw_height),
        );
        let _: () = msg_send![image_view, setFrame: frame];
    }
}

fn register_preview_text_view_class() {
    unsafe {
        if objc::runtime::Class::get("MKPreviewTextView").is_some() {
            return;
        }
        let superclass = class!(NSTextView);
        let mut decl = ClassDecl::new("MKPreviewTextView", superclass).unwrap();

        extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> BOOL {
            YES
        }

        decl.add_method(
            sel!(isFlipped),
            is_flipped as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.register();
    }
}

pub unsafe fn create_clipboard_preview_view(content_view: id, frame: NSRect) {
    let preview: id = msg_send![class!(NSView), alloc];
    let preview: id = msg_send![preview, initWithFrame: frame];
    let _: () = msg_send![preview, setWantsLayer: YES];
    let layer: id = msg_send![preview, layer];
    let bg_color: id = msg_send![class!(NSColor), clearColor];
    let bg_color_cg: id = msg_send![bg_color, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_color_cg];
    let _: () = msg_send![layer, setCornerRadius: style::PREVIEW_CORNER_RADIUS];
    let _: () = msg_send![layer, setBorderWidth: 0.0f64];
    let _: () = msg_send![preview, setAutoresizingMask: 16]; // height only
    let content_inset = style::PREVIEW_CONTENT_INSET;
    let text_area_height = (frame.size.height - content_inset - PREVIEW_HEADER_HEIGHT).max(140.0);
    let text_area_frame = NSRect::new(
        NSPoint::new(content_inset, content_inset),
        NSSize::new(frame.size.width - content_inset * 2.0, text_area_height),
    );
    let text_bg: id = msg_send![class!(NSVisualEffectView), alloc];
    let text_bg: id = msg_send![text_bg, initWithFrame: text_area_frame];
    let _: () = msg_send![text_bg, setMaterial: 12];
    let _: () = msg_send![text_bg, setBlendingMode: 0];
    let _: () = msg_send![text_bg, setState: 1];
    let _: () = msg_send![text_bg, setWantsLayer: YES];
    let text_bg_layer: id = msg_send![text_bg, layer];
    let _: () = msg_send![text_bg_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![text_bg_layer, setBorderWidth: 1.0f64];
    let text_border: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let text_border_cg: id = msg_send![text_border, CGColor];
    let _: () = msg_send![text_bg_layer, setBorderColor: text_border_cg];
    let _: () = msg_send![text_bg_layer, setMasksToBounds: YES];
    let _: () = msg_send![text_bg, setHidden: YES];

    // Title label
    let title_field: id = msg_send![class!(NSTextField), alloc];
    let title_field: id = msg_send![title_field, initWithFrame: NSRect::new(NSPoint::new(content_inset, frame.size.height - content_inset - 26.0), NSSize::new(frame.size.width - content_inset * 2.0, 26.0))];
    let _: () = msg_send![title_field, setBezeled: NO];
    let _: () = msg_send![title_field, setEditable: NO];
    let _: () = msg_send![title_field, setDrawsBackground: NO];
    let _: () = msg_send![title_field, setBordered: NO];
    let _: () = msg_send![title_field, setAutoresizingMask: 2];
    let title_font: id = msg_send![class!(NSFont), systemFontOfSize:17.0 weight:0.6];
    let _: () = msg_send![title_field, setFont: title_font];
    let title_color: id = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![title_field, setTextColor: title_color];
    let _: () = msg_send![title_field, setHidden: YES];
    let _: () = msg_send![preview, addSubview: title_field];

    // Detail label
    let detail_field: id = msg_send![class!(NSTextField), alloc];
    let detail_field: id = msg_send![detail_field, initWithFrame: NSRect::new(NSPoint::new(content_inset, frame.size.height - content_inset - 50.0), NSSize::new(frame.size.width - content_inset * 2.0, 20.0))];
    let _: () = msg_send![detail_field, setBezeled: NO];
    let _: () = msg_send![detail_field, setEditable: NO];
    let _: () = msg_send![detail_field, setDrawsBackground: NO];
    let _: () = msg_send![detail_field, setBordered: NO];
    let _: () = msg_send![detail_field, setAutoresizingMask: 2];
    let detail_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.7f64];
    let _: () = msg_send![detail_field, setTextColor: detail_color];
    let detail_font: id = msg_send![class!(NSFont), systemFontOfSize:13.0];
    let _: () = msg_send![detail_field, setFont: detail_font];
    let _: () = msg_send![detail_field, setHidden: YES];
    let _: () = msg_send![preview, addSubview: detail_field];

    // Text scroll + view
    let text_scroll: id = msg_send![class!(NSScrollView), alloc];
    let text_scroll: id = msg_send![text_scroll, initWithFrame: text_area_frame];
    let _: () = msg_send![text_scroll, setBorderType: 0];
    let _: () = msg_send![text_scroll, setHasVerticalScroller: YES];
    let _: () = msg_send![text_scroll, setDrawsBackground: NO];
    let _: () = msg_send![text_scroll, setAutoresizingMask: 18];
    register_preview_text_view_class();
    let text_class = class!(MKPreviewTextView);
    let text_view: id = msg_send![text_class, alloc];
    let text_view: id = msg_send![text_view, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(text_area_frame.size.width - 20.0, text_area_frame.size.height))];
    let _: () = msg_send![text_view, setEditable: NO];
    let _: () = msg_send![text_view, setSelectable: YES];
    let _: () = msg_send![text_view, setDrawsBackground: NO];
    let text_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:13.0 weight:0.0];
    let _: () = msg_send![text_view, setFont: text_font];
    let text_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.9f64];
    let _: () = msg_send![text_view, setTextColor: text_color];
    let _: () = msg_send![text_view, setTextContainerInset:NSSize::new(8.0, 10.0)];
    let _: () = msg_send![text_scroll, setDocumentView: text_view];
    let _: () = msg_send![text_scroll, setHidden: YES];
    let _: () = msg_send![preview, addSubview: text_bg];
    let _: () = msg_send![preview, addSubview: text_scroll];

    // Image view
    let image_view: id = msg_send![class!(NSImageView), alloc];
    let image_view: id = msg_send![image_view, initWithFrame: text_area_frame];
    let _: () = msg_send![image_view, setWantsLayer: YES];
    let _: () = msg_send![image_view, setAutoresizingMask: 0];
    let image_layer: id = msg_send![image_view, layer];
    let _: () = msg_send![image_layer, setCornerRadius: 16.0f64];
    let _: () = msg_send![image_layer, setMasksToBounds: YES];
    let _: () = msg_send![image_view, setImageScaling: 1]; // proportionally fit
    let _: () = msg_send![image_view, setHidden: YES];
    let _: () = msg_send![preview, addSubview: image_view];

    // Placeholder
    let placeholder: id = msg_send![class!(NSTextField), alloc];
    let placeholder: id = msg_send![placeholder, initWithFrame: NSRect::new(NSPoint::new(text_area_frame.origin.x, text_area_frame.origin.y + text_area_frame.size.height / 2.0 - 40.0), NSSize::new(text_area_frame.size.width, 80.0))];
    let _: () = msg_send![placeholder, setBezeled: NO];
    let _: () = msg_send![placeholder, setEditable: NO];
    let _: () = msg_send![placeholder, setDrawsBackground: NO];
    let _: () = msg_send![placeholder, setBordered: NO];
    let _: () = msg_send![placeholder, setAlignment: 1];
    let placeholder_font: id = msg_send![class!(NSFont), systemFontOfSize:13.0];
    let _: () = msg_send![placeholder, setFont: placeholder_font];
    let placeholder_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.65f64];
    let _: () = msg_send![placeholder, setTextColor: placeholder_color];
    let _: () = msg_send![placeholder, setStringValue: NSString::alloc(nil).init_str("Clipboard preview\nSelect an entry to view details")];
    let _: () = msg_send![preview, addSubview: placeholder];

    let _: () = msg_send![content_view, addSubview: preview];

    let _ = CLIPBOARD_PREVIEW.set(ClipboardPreviewRefs {
        root: preview as usize,
        title_field: title_field as usize,
        detail_field: detail_field as usize,
        placeholder_field: placeholder as usize,
        text_scroll: text_scroll as usize,
        text_view: text_view as usize,
        image_view: image_view as usize,
        text_background: text_bg as usize,
    });
    update_clipboard_preview_selection(None);
    refresh_clipboard_preview_layout();
}

pub fn placeholder_clipboard_icon() -> id {
    unsafe {
        let symbol_name = NSString::alloc(nil).init_str("doc.on.clipboard");
        msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil]
    }
}

fn placeholder_image_icon() -> id {
    unsafe {
        let symbol_name = NSString::alloc(nil).init_str("photo.on.rectangle");
        msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil]
    }
}

fn image_from_clipboard_content(content: &str) -> Option<id> {
    if let Ok(bytes) = STANDARD.decode(content) {
        if bytes.is_empty() {
            return None;
        }
        unsafe {
            let data: id = msg_send![class!(NSData), alloc];
            let data: id = msg_send![data, initWithBytes:bytes.as_ptr() length:bytes.len() as u64];
            if data == nil {
                return None;
            }
            let image: id = msg_send![class!(NSImage), alloc];
            let image: id = msg_send![image, initWithData:data];
            if image == nil {
                return None;
            }
            Some(image)
        }
    } else {
        None
    }
}

fn schedule_app_icon_fetch(path: String, row: isize) {
    SEARCH_RT.spawn_blocking(move || unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let path_str = NSString::alloc(nil).init_str(&path);
        let img: id = msg_send![workspace, iconForFile: path_str];
        if img != nil {
            let img_ptr = img as usize;
            let path_clone = path.clone();
            run_on_main(move || {
                let img_for_main: id = img_ptr as id;
                let _: id = msg_send![img_for_main, retain];
                if let Ok(mut cache) = ICON_CACHE.lock() {
                    cache.insert(path_clone.clone(), img_for_main as usize);
                }
                crate::ui::table::set_icon_for_row_from_cache(&path_clone, row);
            });
        }
    });
}

fn app_icon_for_name(app_name: &str, row: isize) -> id {
    if let Some(path) = app_launcher::find_app_path_by_name(app_name) {
        if let Ok(cache) = ICON_CACHE.lock() {
            if let Some(&cached) = cache.get(&path) {
                return cached as id;
            }
        }
        schedule_app_icon_fetch(path.clone(), row);
    }
    placeholder_clipboard_icon()
}

pub fn icon_for_history_entry(entry: &search_engine::SearchResult, row: isize) -> id {
    if let search_engine::SearchResult::Clipboard {
        content_type,
        app_name,
        ..
    } = entry
    {
        if let Some(name) = app_name.as_ref() {
            return app_icon_for_name(name, row);
        }
        if content_type == "image" {
            return placeholder_image_icon();
        }
    }
    placeholder_clipboard_icon()
}

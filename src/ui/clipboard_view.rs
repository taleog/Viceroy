use crate::app_launcher;
use crate::clipboard;
use crate::search_engine;
use crate::ui::helpers::run_on_main;
use crate::ui::state::{TableMode, ICON_CACHE, SEARCH_RT, TABLE_DATA, TABLE_MODE, TABLE_RESULTS};
use crate::ui::table::{reload_table, schedule_table_update_next_tick};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{Local, LocalResult, TimeZone, Utc};
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::{class, msg_send, sel, sel_impl};

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

        let preview = if entry.content_type == "image" {
            entry
                .custom_name
                .clone()
                .unwrap_or_else(|| "Image".to_string())
        } else {
            truncate_text(&entry.content, 100)
        };

        let title = entry.custom_name.clone().unwrap_or_else(|| {
            if entry.content_type == "image" {
                "Image".to_string()
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
            score: 0,
        });
    }

    (rows, results)
}

pub fn apply_clipboard_history_state(
    rows: Vec<(String, String)>,
    results: Vec<search_engine::SearchResult>,
) {
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

pub fn placeholder_clipboard_icon() -> id {
    unsafe {
        let symbol_name = NSString::alloc(nil).init_str("doc.on.clipboard");
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
        content,
        app_name,
        ..
    } = entry
    {
        if content_type == "image" {
            if let Some(image) = image_from_clipboard_content(content) {
                return image;
            }
        }
        if let Some(name) = app_name.as_ref() {
            return app_icon_for_name(name, row);
        }
    }
    placeholder_clipboard_icon()
}

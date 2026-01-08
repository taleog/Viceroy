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
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::fmt::Write;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::OnceLock;

const MAX_PREVIEW_CHARS: usize = 5000;
const MAX_FILE_PREVIEW_BYTES: u64 = 64 * 1024;
const PREVIEW_HEADER_HEIGHT: f64 = 86.0;
const PREVIEW_ACTION_BAR_HEIGHT: f64 = 32.0;
const PREVIEW_ACTION_GAP: f64 = 10.0;
const PREVIEW_ACTION_BUTTON_HEIGHT: f64 = 26.0;
const PREVIEW_ACTION_BUTTON_SPACING: f64 = 8.0;
const PREVIEW_ACTION_BUTTON_PADDING: f64 = 10.0;
const PREVIEW_ACTION_EDIT_WIDTH: f64 = 64.0;
const PREVIEW_ACTION_REMOVE_WIDTH: f64 = 82.0;
const PREVIEW_ACTION_SAVE_WIDTH: f64 = 70.0;
const PREVIEW_ACTION_CANCEL_WIDTH: f64 = 84.0;

static PREVIEW_HOVERED: AtomicBool = AtomicBool::new(false);
static PREVIEW_SELECTED_ROW: AtomicI64 = AtomicI64::new(-1);
static PREVIEW_REQUEST_ID: AtomicU64 = AtomicU64::new(0);
static EDIT_ENTRY_ID: AtomicI64 = AtomicI64::new(-1);
static EDIT_IS_TEXT: AtomicBool = AtomicBool::new(false);
static EDIT_MODE: AtomicBool = AtomicBool::new(false);

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

unsafe fn set_button_title(button: id, title: &str, font: id, color: id) {
    let title_ns = NSString::alloc(nil).init_str(title);
    let attrs: id = msg_send![class!(NSMutableDictionary), dictionary];
    let _: () = msg_send![attrs, setObject:color forKey:NSString::alloc(nil).init_str("NSColor")];
    let _: () = msg_send![attrs, setObject:font forKey:NSString::alloc(nil).init_str("NSFont")];
    let attributed: id = msg_send![class!(NSAttributedString), alloc];
    let attributed: id = msg_send![attributed, initWithString:title_ns attributes:attrs];
    let _: () = msg_send![button, setAttributedTitle: attributed];
}

fn placeholder_text_for_mode(mode: TableMode) -> &'static str {
    match mode {
        TableMode::ClipboardHistory => "Select a clipboard entry to preview",
        TableMode::Search => "Select a file or clipboard item to preview",
        TableMode::Settings => "Preview unavailable in Settings",
    }
}

fn refresh_action_bar_visibility() {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    let hovered = PREVIEW_HOVERED.load(Ordering::SeqCst);
    let selected = PREVIEW_SELECTED_ROW.load(Ordering::SeqCst) >= 0;
    let editing = EDIT_MODE.load(Ordering::SeqCst);
    let show = mode == TableMode::ClipboardHistory && selected && (hovered || editing);
    if let Some(refs) = preview_refs() {
        unsafe {
            let bar = id_from(refs.action_bar);
            set_hidden(bar, !show);
        }
    }
}

fn preview_actions() -> id {
    static ACTIONS: OnceLock<usize> = OnceLock::new();
    if let Some(ptr) = ACTIONS.get() {
        return *ptr as id;
    }
    let target = unsafe { register_preview_action_class() };
    let _ = ACTIONS.set(target as usize);
    target
}

fn apply_edit_mode(enabled: bool, is_text: bool) {
    if let Some(refs) = preview_refs() {
        unsafe {
            let title_field = id_from(refs.title_field);
            let edit_button = id_from(refs.edit_button);
            let remove_button = id_from(refs.remove_button);
            let save_button = id_from(refs.save_button);
            let cancel_button = id_from(refs.cancel_button);
            let text_view = id_from(refs.text_view);

            let _: () = msg_send![title_field, setEditable: if enabled { YES } else { NO }];
            let _: () = msg_send![title_field, setSelectable: if enabled { YES } else { NO }];
            let _: () = msg_send![title_field, setBezeled: if enabled { YES } else { NO }];
            let _: () = msg_send![title_field, setBordered: if enabled { YES } else { NO }];
            let _: () = msg_send![title_field, setDrawsBackground: if enabled { YES } else { NO }];
            if enabled {
                let bg: id =
                    msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
                let _: () = msg_send![title_field, setBackgroundColor: bg];
            }

            let editable = enabled && is_text;
            let _: () = msg_send![text_view, setEditable: if editable { YES } else { NO }];
            let _: () = msg_send![text_view, setSelectable: YES];

            set_hidden(edit_button, enabled);
            set_hidden(remove_button, enabled);
            set_hidden(save_button, !enabled);
            set_hidden(cancel_button, !enabled);
        }
    }
}

fn exit_edit_mode() {
    let was_text = EDIT_IS_TEXT.load(Ordering::SeqCst);
    EDIT_MODE.store(false, Ordering::SeqCst);
    EDIT_ENTRY_ID.store(-1, Ordering::SeqCst);
    EDIT_IS_TEXT.store(false, Ordering::SeqCst);
    apply_edit_mode(false, was_text);
    refresh_action_bar_visibility();
}

pub fn begin_inline_edit(entry: search_engine::SearchResult) {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    if mode != TableMode::ClipboardHistory {
        return;
    }

    if let search_engine::SearchResult::Clipboard {
        id,
        content,
        content_type,
        app_name,
        timestamp,
        custom_name,
        image_width,
        image_height,
        ..
    } = entry
    {
        let is_text = content_type == "text";
        let current_id = EDIT_ENTRY_ID.load(Ordering::SeqCst);
        if EDIT_MODE.load(Ordering::SeqCst) && current_id == id {
            return;
        }
        if EDIT_MODE.load(Ordering::SeqCst) && current_id != id {
            exit_edit_mode();
        }

        EDIT_ENTRY_ID.store(id, Ordering::SeqCst);
        EDIT_IS_TEXT.store(is_text, Ordering::SeqCst);
        EDIT_MODE.store(true, Ordering::SeqCst);

        if let Some(refs) = preview_refs() {
            unsafe {
                let preview_root = id_from(refs.root);
                let title_field = id_from(refs.title_field);
                let detail_field = id_from(refs.detail_field);
                let placeholder = id_from(refs.placeholder_field);
                let text_scroll = id_from(refs.text_scroll);
                let text_view = id_from(refs.text_view);
                let image_view = id_from(refs.image_view);
                let text_background = id_from(refs.text_background);

                let (default_title, subtitle, _) = labels_for_clipboard_entry(
                    &content,
                    &content_type,
                    &app_name,
                    timestamp,
                    custom_name.as_ref(),
                    image_width,
                    image_height,
                );

                set_hidden(placeholder, true);
                set_hidden(title_field, false);
                set_hidden(detail_field, false);
                set_string(detail_field, &subtitle);

                let placeholder_value = NSString::alloc(nil).init_str(&default_title);
                let _: () = msg_send![title_field, setPlaceholderString: placeholder_value];
                if let Some(name) = custom_name.as_ref() {
                    set_string(title_field, name);
                } else {
                    set_string(title_field, "");
                }

                if is_text {
                    set_hidden(text_scroll, false);
                    set_hidden(text_background, false);
                    set_hidden(image_view, true);
                    let ns_content = NSString::alloc(nil).init_str(&content);
                    let _: () = msg_send![text_view, setString: ns_content];
                    reset_text_scroll_position(text_scroll, text_view);
                } else {
                    set_hidden(text_scroll, true);
                    set_hidden(text_background, true);
                    set_hidden(image_view, false);
                    if let Some(image) = image_from_clipboard_content(&content) {
                        layout_image_preview(preview_root, image_view, image);
                        let _: () = msg_send![image_view, setImage: image];
                    }
                }

                apply_edit_mode(true, is_text);
                refresh_action_bar_visibility();

                let window: id = msg_send![title_field, window];
                if window != nil {
                    let responder = if is_text { text_view } else { title_field };
                    let _: () = msg_send![window, makeFirstResponder: responder];
                }
            }
        }
    }
}

unsafe fn read_text_field_value(field: id) -> String {
    let value: id = msg_send![field, stringValue];
    let cstr: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if cstr.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string()
    }
}

unsafe fn read_text_view_value(view: id) -> String {
    let value: id = msg_send![view, string];
    let cstr: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if cstr.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string()
    }
}

unsafe fn save_inline_edit() {
    let entry_id = EDIT_ENTRY_ID.load(Ordering::SeqCst);
    if entry_id < 0 {
        return;
    }

    let refs = match preview_refs() {
        Some(r) => r,
        None => return,
    };
    let title_field = id_from(refs.title_field);
    let text_view = id_from(refs.text_view);

    let title = read_text_field_value(title_field);
    let title_trim = title.trim();
    let new_custom_name = if title_trim.is_empty() {
        None
    } else {
        Some(title_trim.to_string())
    };

    let is_text = EDIT_IS_TEXT.load(Ordering::SeqCst);
    let new_content = if is_text {
        read_text_view_value(text_view)
    } else {
        String::new()
    };

    exit_edit_mode();

    let updated_row = update_clipboard_entry_in_ui(
        entry_id,
        if is_text {
            Some(new_content.clone())
        } else {
            None
        },
        new_custom_name.clone(),
    );

    if let Some(row) = updated_row {
        crate::ui::clipboard_view::update_clipboard_preview_selection(Some(row));
    }

    SEARCH_RT.spawn(async move {
        if is_text {
            let _ = crate::clipboard::update_entry(entry_id, new_content, new_custom_name).await;
        } else {
            let _ = crate::clipboard::update_custom_name(entry_id, new_custom_name).await;
        }
    });
}

unsafe fn cancel_inline_edit() {
    let row = PREVIEW_SELECTED_ROW.load(Ordering::SeqCst);
    exit_edit_mode();
    if row >= 0 {
        update_clipboard_preview_selection(Some(row as usize));
    } else {
        update_clipboard_preview_selection(None);
    }
}

fn update_clipboard_entry_in_ui(
    entry_id: i64,
    new_content: Option<String>,
    new_custom_name: Option<String>,
) -> Option<usize> {
    let mut row_index = None;
    if let Ok(mut results) = TABLE_RESULTS.lock() {
        for (idx, result) in results.iter_mut().enumerate() {
            if let search_engine::SearchResult::Clipboard {
                id,
                content,
                content_type,
                app_name,
                timestamp,
                custom_name,
                image_width,
                image_height,
                preview,
                ..
            } = result
            {
                if *id != entry_id {
                    continue;
                }
                if let Some(new_value) = &new_content {
                    *content = new_value.clone();
                }
                *custom_name = new_custom_name.clone();

                let (title, subtitle, preview_value) = labels_for_clipboard_entry(
                    content,
                    content_type,
                    app_name,
                    *timestamp,
                    custom_name.as_ref(),
                    *image_width,
                    *image_height,
                );
                *preview = preview_value;
                if let Ok(mut td) = TABLE_DATA.lock() {
                    if let Some(row) = td.get_mut(idx) {
                        row.0 = title;
                        row.1 = subtitle;
                    }
                }
                row_index = Some(idx);
                break;
            }
        }
    }
    if let Some(row) = row_index {
        unsafe {
            reload_table();
            schedule_table_update_next_tick();
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let windows: id = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            if count > 0 {
                let window: id = msg_send![windows, objectAtIndex:0];
                let content: id = msg_send![window, contentView];
                let subviews: id = msg_send![content, subviews];
                let scroll: id = msg_send![subviews, objectAtIndex:2];
                let table: id = msg_send![scroll, documentView];
                let index_set: id = msg_send![class!(NSIndexSet), indexSetWithIndex:row];
                let _: () = msg_send![table, selectRowIndexes:index_set byExtendingSelection:NO];
                let _: () = msg_send![table, scrollRowToVisible:row as isize];
            }
        }
    }
    row_index
}

fn labels_for_clipboard_entry(
    content: &str,
    content_type: &str,
    app_name: &Option<String>,
    timestamp: i64,
    custom_name: Option<&String>,
    image_width: Option<i64>,
    image_height: Option<i64>,
) -> (String, String, String) {
    let now = Utc::now().timestamp();
    let app_label = app_name
        .clone()
        .unwrap_or_else(|| "Unknown App".to_string());
    let time_label = format_clipboard_relative_time(timestamp, now);
    let detail_label = if content_type == "image" {
        if let (Some(width), Some(height)) = (image_width, image_height) {
            format!("{}×{} px", width, height)
        } else {
            "Image".to_string()
        }
    } else {
        format!("{} chars", content.chars().count())
    };
    let subtitle = format!("{} · {} · {}", app_label, time_label, detail_label);

    let image_title = if content_type == "image" {
        if detail_label == "Image" {
            "Image".to_string()
        } else {
            format!("Image · {}", detail_label)
        }
    } else {
        String::new()
    };

    let preview = if content_type == "image" {
        custom_name.cloned().unwrap_or_else(|| image_title.clone())
    } else {
        truncate_text(content, 100)
    };

    let title = custom_name.cloned().unwrap_or_else(|| {
        if content_type == "image" {
            image_title.clone()
        } else {
            truncate_text(content, 60)
        }
    });

    (title, subtitle, preview)
}

unsafe fn register_preview_action_class() -> id {
    if objc::runtime::Class::get("MKClipboardPreviewActions").is_none() {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MKClipboardPreviewActions", superclass).unwrap();

        extern "C" fn edit_clipboard_entry(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                crate::ui::table::edit_selected_clipboard_entry();
            }
        }

        extern "C" fn delete_clipboard_entry(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                crate::ui::table::delete_selected_clipboard_entry();
            }
        }

        extern "C" fn save_clipboard_edit(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                save_inline_edit();
            }
        }

        extern "C" fn cancel_clipboard_edit(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                cancel_inline_edit();
            }
        }

        extern "C" fn mouse_entered(_this: &Object, _cmd: Sel, _event: id) {
            PREVIEW_HOVERED.store(true, Ordering::SeqCst);
            refresh_action_bar_visibility();
        }

        extern "C" fn mouse_exited(_this: &Object, _cmd: Sel, _event: id) {
            PREVIEW_HOVERED.store(false, Ordering::SeqCst);
            refresh_action_bar_visibility();
        }

        decl.add_method(
            sel!(editClipboardEntry:),
            edit_clipboard_entry as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(deleteClipboardEntry:),
            delete_clipboard_entry as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(saveClipboardEdit:),
            save_clipboard_edit as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(cancelClipboardEdit:),
            cancel_clipboard_edit as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseEntered:),
            mouse_entered as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseExited:),
            mouse_exited as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }

    let cls = class!(MKClipboardPreviewActions);
    let target: id = msg_send![cls, new];
    let _: id = msg_send![target, retain];
    target
}

fn preview_data_for_row(row: usize) -> Option<search_engine::SearchResult> {
    let results = TABLE_RESULTS.lock().ok()?;
    results.get(row).cloned()
}

fn table_labels_for_row(row: usize) -> Option<(String, String)> {
    let rows = TABLE_DATA.lock().ok()?;
    rows.get(row).cloned()
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
            let action_bar = id_from(refs.action_bar);
            if mode == TableMode::Settings {
                set_hidden(root, true);
                set_hidden(action_bar, true);
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
            set_hidden(action_bar, true);
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

fn show_preview_message(title: &str, subtitle: &str, message: &str) {
    if let Some(refs) = preview_refs() {
        unsafe {
            let title_field = id_from(refs.title_field);
            let detail_field = id_from(refs.detail_field);
            let placeholder = id_from(refs.placeholder_field);
            let text_scroll = id_from(refs.text_scroll);
            let image_view = id_from(refs.image_view);
            let text_background = id_from(refs.text_background);

            set_hidden(text_scroll, true);
            set_hidden(image_view, true);
            set_hidden(text_background, true);
            set_hidden(title_field, false);
            set_hidden(detail_field, false);
            set_hidden(placeholder, false);

            set_string(title_field, title);
            set_string(detail_field, subtitle);
            set_string(placeholder, message);
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < units.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, units[unit])
    } else {
        format!("{:.1} {}", size, units[unit])
    }
}

fn is_image_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            matches!(
                lower.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "heic" | "heif" | "webp"
            )
        })
        .unwrap_or(false)
}

fn load_text_preview(path: &str, size: Option<u64>) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut buf = Vec::new();
    let mut reader = file.take(MAX_FILE_PREVIEW_BYTES);
    reader.read_to_end(&mut buf).ok()?;
    if buf.contains(&0) {
        return None;
    }
    let mut text = String::from_utf8_lossy(&buf).to_string();
    if size.map(|s| s > MAX_FILE_PREVIEW_BYTES).unwrap_or(false) {
        let _ = write!(
            text,
            "\n… (showing first {} of {} bytes)",
            MAX_FILE_PREVIEW_BYTES,
            size.unwrap_or(MAX_FILE_PREVIEW_BYTES)
        );
    }
    Some(text)
}

fn file_preview_subtitle(path: &str, size: Option<u64>) -> String {
    match size {
        Some(bytes) => format!("{} · {}", path, format_bytes(bytes)),
        None => path.to_string(),
    }
}

enum FilePreviewKind {
    Image(String),
    Text(String),
    Message(String),
}

fn start_file_preview(title: String, path: String, request_id: u64) {
    show_preview_message(&title, &path, "Loading preview...");
    SEARCH_RT.spawn_blocking(move || {
        let metadata = std::fs::metadata(&path).ok();
        let size = metadata.as_ref().map(|meta| meta.len());
        let subtitle = file_preview_subtitle(&path, size);
        let is_image = is_image_extension(&path);

        let preview = if is_image {
            FilePreviewKind::Image(path.clone())
        } else if let Some(text) = load_text_preview(&path, size) {
            FilePreviewKind::Text(text)
        } else {
            FilePreviewKind::Message("No preview available".to_string())
        };

        run_on_main(move || {
            if PREVIEW_REQUEST_ID.load(Ordering::SeqCst) != request_id {
                return;
            }
            match preview {
                FilePreviewKind::Image(path) => {
                    if let Some(image) = image_from_path(&path) {
                        show_image_preview(&title, &subtitle, image);
                    } else {
                        show_preview_message(&title, &subtitle, "No preview available");
                    }
                }
                FilePreviewKind::Text(body) => {
                    show_text_preview(&title, &subtitle, &body);
                }
                FilePreviewKind::Message(message) => {
                    show_preview_message(&title, &subtitle, &message);
                }
            }
        });
    });
}

pub fn update_clipboard_preview_selection(row: Option<usize>) {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    let selected_row = row.map(|r| r as i64).unwrap_or(-1);
    PREVIEW_SELECTED_ROW.store(selected_row, Ordering::SeqCst);
    let request_id = PREVIEW_REQUEST_ID.fetch_add(1, Ordering::SeqCst) + 1;
    if EDIT_MODE.load(Ordering::SeqCst) {
        if mode == TableMode::ClipboardHistory {
            if let Some(selected_row) = row {
                if let Some(search_engine::SearchResult::Clipboard { id, .. }) =
                    preview_data_for_row(selected_row)
                {
                    if id == EDIT_ENTRY_ID.load(Ordering::SeqCst) {
                        refresh_action_bar_visibility();
                        return;
                    }
                }
            }
        }
        exit_edit_mode();
    }
    refresh_action_bar_visibility();
    if row.is_none() {
        set_placeholder_for_mode(mode);
        return;
    }
    let selected_row = row.unwrap();
    match mode {
        TableMode::ClipboardHistory => {
            if let Some(entry) = preview_data_for_row(selected_row) {
                if let Some((title, subtitle, maybe_text)) = detail_label_for_entry(&entry) {
                    if let search_engine::SearchResult::Clipboard {
                        content,
                        content_type,
                        ..
                    } = entry
                    {
                        if content_type == "image" {
                            let placeholder = placeholder_image_icon();
                            show_image_preview(&title, &subtitle, placeholder);
                            if let Some(image) = image_from_clipboard_content(&content) {
                                show_image_preview(&title, &subtitle, image);
                            }
                            return;
                        } else if let Some(text_body) = maybe_text {
                            show_text_preview(&title, &subtitle, &text_body);
                            return;
                        }
                    }
                }
            }
            set_placeholder_for_mode(mode);
        }
        TableMode::Search => {
            if let Some(entry) = preview_data_for_row(selected_row) {
                match entry {
                    search_engine::SearchResult::File { name, path, .. } => {
                        start_file_preview(name, path, request_id);
                        return;
                    }
                    search_engine::SearchResult::Clipboard { .. } => {
                        if let Some((title, subtitle, maybe_text)) = detail_label_for_entry(&entry)
                        {
                            if let search_engine::SearchResult::Clipboard {
                                content,
                                content_type,
                                ..
                            } = entry
                            {
                                if content_type == "image" {
                                    let placeholder = placeholder_image_icon();
                                    show_image_preview(&title, &subtitle, placeholder);
                                    if let Some(image) = image_from_clipboard_content(&content) {
                                        show_image_preview(&title, &subtitle, image);
                                    }
                                    return;
                                } else if let Some(text_body) = maybe_text {
                                    show_text_preview(&title, &subtitle, &text_body);
                                    return;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some((title, subtitle)) = table_labels_for_row(selected_row) {
                show_preview_message(&title, &subtitle, "No preview available");
            } else {
                set_placeholder_for_mode(mode);
            }
        }
        TableMode::Settings => {
            set_placeholder_for_mode(mode);
        }
    }
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
    let action_frame = preview_action_frame(bounds);
    let width = (bounds.size.width - content_inset * 2.0).max(120.0);
    let height = (bounds.size.height
        - PREVIEW_HEADER_HEIGHT
        - content_inset
        - action_frame.size.height
        - PREVIEW_ACTION_GAP)
        .max(140.0);
    NSRect::new(
        NSPoint::new(
            content_inset,
            content_inset + action_frame.size.height + PREVIEW_ACTION_GAP,
        ),
        NSSize::new(width, height),
    )
}

fn action_bar_width() -> f64 {
    let edit_group = PREVIEW_ACTION_BUTTON_PADDING * 2.0
        + PREVIEW_ACTION_EDIT_WIDTH
        + PREVIEW_ACTION_BUTTON_SPACING
        + PREVIEW_ACTION_REMOVE_WIDTH;
    let save_group = PREVIEW_ACTION_BUTTON_PADDING * 2.0
        + PREVIEW_ACTION_CANCEL_WIDTH
        + PREVIEW_ACTION_BUTTON_SPACING
        + PREVIEW_ACTION_SAVE_WIDTH;
    edit_group.max(save_group)
}

fn preview_action_frame(bounds: NSRect) -> NSRect {
    let content_inset = style::PREVIEW_CONTENT_INSET;
    let max_width = (bounds.size.width - content_inset * 2.0).max(120.0);
    let width = action_bar_width().min(max_width);
    NSRect::new(
        NSPoint::new(bounds.size.width - content_inset - width, content_inset),
        NSSize::new(width, PREVIEW_ACTION_BAR_HEIGHT),
    )
}

unsafe fn apply_preview_subview_layout(bounds: NSRect, refs: &ClipboardPreviewRefs) {
    let text_area_frame = preview_content_frame(bounds);
    let title_field = id_from(refs.title_field);
    let detail_field = id_from(refs.detail_field);
    let action_bar = id_from(refs.action_bar);
    let edit_button = id_from(refs.edit_button);
    let remove_button = id_from(refs.remove_button);
    let save_button = id_from(refs.save_button);
    let cancel_button = id_from(refs.cancel_button);
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
    let action_frame = preview_action_frame(bounds);
    let button_y = (action_frame.size.height - PREVIEW_ACTION_BUTTON_HEIGHT) / 2.0;
    let remove_width = PREVIEW_ACTION_REMOVE_WIDTH;
    let edit_width = PREVIEW_ACTION_EDIT_WIDTH;
    let save_width = PREVIEW_ACTION_SAVE_WIDTH;
    let cancel_width = PREVIEW_ACTION_CANCEL_WIDTH;
    let remove_x = (action_frame.size.width - PREVIEW_ACTION_BUTTON_PADDING - remove_width)
        .max(PREVIEW_ACTION_BUTTON_PADDING);
    let edit_x =
        (remove_x - PREVIEW_ACTION_BUTTON_SPACING - edit_width).max(PREVIEW_ACTION_BUTTON_PADDING);
    let save_x = (action_frame.size.width - PREVIEW_ACTION_BUTTON_PADDING - save_width)
        .max(PREVIEW_ACTION_BUTTON_PADDING);
    let cancel_x =
        (save_x - PREVIEW_ACTION_BUTTON_SPACING - cancel_width).max(PREVIEW_ACTION_BUTTON_PADDING);
    let edit_frame = NSRect::new(
        NSPoint::new(edit_x, button_y),
        NSSize::new(edit_width, PREVIEW_ACTION_BUTTON_HEIGHT),
    );
    let remove_frame = NSRect::new(
        NSPoint::new(remove_x, button_y),
        NSSize::new(remove_width, PREVIEW_ACTION_BUTTON_HEIGHT),
    );
    let save_frame = NSRect::new(
        NSPoint::new(save_x, button_y),
        NSSize::new(save_width, PREVIEW_ACTION_BUTTON_HEIGHT),
    );
    let cancel_frame = NSRect::new(
        NSPoint::new(cancel_x, button_y),
        NSSize::new(cancel_width, PREVIEW_ACTION_BUTTON_HEIGHT),
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
    let _: () = msg_send![action_bar, setFrame: action_frame];
    let _: () = msg_send![edit_button, setFrame: edit_frame];
    let _: () = msg_send![remove_button, setFrame: remove_frame];
    let _: () = msg_send![save_button, setFrame: save_frame];
    let _: () = msg_send![cancel_button, setFrame: cancel_frame];
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
    let text_area_frame = preview_content_frame(frame);
    let text_bg: id = msg_send![class!(NSVisualEffectView), alloc];
    let text_bg: id = msg_send![text_bg, initWithFrame: text_area_frame];
    let _: () = msg_send![text_bg, setMaterial: 12];
    let _: () = msg_send![text_bg, setBlendingMode: 0];
    let _: () = msg_send![text_bg, setState: 1];
    let _: () = msg_send![text_bg, setWantsLayer: YES];
    let text_bg_layer: id = msg_send![text_bg, layer];
    let _: () = msg_send![text_bg_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![text_bg_layer, setBorderWidth: 1.0f64];
    let text_border: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
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

    // Action bar (Edit / Remove / Save / Cancel), shown on hover when a row is selected
    let action_frame = preview_action_frame(frame);
    let action_bar: id = msg_send![class!(NSView), alloc];
    let action_bar: id = msg_send![action_bar, initWithFrame: action_frame];
    let _: () = msg_send![action_bar, setWantsLayer: YES];
    let action_layer: id = msg_send![action_bar, layer];
    let action_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.16f64 alpha:0.35f64];
    let action_bg_cg: id = msg_send![action_bg, CGColor];
    let _: () = msg_send![action_layer, setBackgroundColor: action_bg_cg];
    let _: () = msg_send![action_layer, setCornerRadius: 12.0f64];
    let _: () = msg_send![action_bar, setHidden: YES];

    let actions_target = preview_actions();

    let edit_button: id = msg_send![class!(NSButton), alloc];
    let edit_button: id = msg_send![edit_button, initWithFrame:NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PREVIEW_ACTION_EDIT_WIDTH, PREVIEW_ACTION_BUTTON_HEIGHT))];
    let _: () = msg_send![edit_button, setBezelStyle: 1];
    let _: () = msg_send![edit_button, setBordered: YES];
    let _: () = msg_send![edit_button, setTitle: NSString::alloc(nil).init_str("Edit")];
    let edit_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.4];
    let edit_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.85f64];
    set_button_title(edit_button, "Edit", edit_font, edit_color);
    let _: () = msg_send![edit_button, setContentTintColor: edit_color];
    let _: () = msg_send![edit_button, setTarget: actions_target];
    let _: () = msg_send![edit_button, setAction: sel!(editClipboardEntry:)];

    let remove_button: id = msg_send![class!(NSButton), alloc];
    let remove_button: id = msg_send![remove_button, initWithFrame:NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PREVIEW_ACTION_REMOVE_WIDTH, PREVIEW_ACTION_BUTTON_HEIGHT))];
    let _: () = msg_send![remove_button, setBezelStyle: 1];
    let _: () = msg_send![remove_button, setBordered: YES];
    let _: () = msg_send![remove_button, setTitle: NSString::alloc(nil).init_str("Remove")];
    let remove_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.45];
    let remove_color: id = msg_send![class!(NSColor), colorWithCalibratedRed:0.95f64 green:0.25f64 blue:0.28f64 alpha:0.95f64];
    set_button_title(remove_button, "Remove", remove_font, remove_color);
    let _: () = msg_send![remove_button, setContentTintColor: remove_color];
    let _: () = msg_send![remove_button, setTarget: actions_target];
    let _: () = msg_send![remove_button, setAction: sel!(deleteClipboardEntry:)];

    let save_button: id = msg_send![class!(NSButton), alloc];
    let save_button: id = msg_send![save_button, initWithFrame:NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PREVIEW_ACTION_SAVE_WIDTH, PREVIEW_ACTION_BUTTON_HEIGHT))];
    let _: () = msg_send![save_button, setBezelStyle: 1];
    let _: () = msg_send![save_button, setBordered: YES];
    let _: () = msg_send![save_button, setTitle: NSString::alloc(nil).init_str("Save")];
    let save_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.5];
    let save_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.9f64];
    set_button_title(save_button, "Save", save_font, save_color);
    let _: () = msg_send![save_button, setContentTintColor: save_color];
    let _: () = msg_send![save_button, setTarget: actions_target];
    let _: () = msg_send![save_button, setAction: sel!(saveClipboardEdit:)];
    let _: () = msg_send![save_button, setHidden: YES];

    let cancel_button: id = msg_send![class!(NSButton), alloc];
    let cancel_button: id = msg_send![cancel_button, initWithFrame:NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(PREVIEW_ACTION_CANCEL_WIDTH, PREVIEW_ACTION_BUTTON_HEIGHT))];
    let _: () = msg_send![cancel_button, setBezelStyle: 1];
    let _: () = msg_send![cancel_button, setBordered: YES];
    let _: () = msg_send![cancel_button, setTitle: NSString::alloc(nil).init_str("Cancel")];
    let cancel_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.4];
    let cancel_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.8f64];
    set_button_title(cancel_button, "Cancel", cancel_font, cancel_color);
    let _: () = msg_send![cancel_button, setContentTintColor: cancel_color];
    let _: () = msg_send![cancel_button, setTarget: actions_target];
    let _: () = msg_send![cancel_button, setAction: sel!(cancelClipboardEdit:)];
    let _: () = msg_send![cancel_button, setHidden: YES];

    let _: () = msg_send![action_bar, addSubview: edit_button];
    let _: () = msg_send![action_bar, addSubview: remove_button];
    let _: () = msg_send![action_bar, addSubview: cancel_button];
    let _: () = msg_send![action_bar, addSubview: save_button];
    let _: () = msg_send![preview, addSubview: action_bar];

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

    // Hover tracking for action bar
    let tracking_opts: u64 = 0x01 | 0x80 | 0x200; // entered/exited + active always + in visible rect
    let tracking: id = msg_send![class!(NSTrackingArea), alloc];
    let tracking: id = msg_send![
        tracking,
        initWithRect:NSRect::new(NSPoint::new(0.0, 0.0), frame.size)
        options:tracking_opts
        owner:actions_target
        userInfo:nil
    ];
    let _: () = msg_send![preview, addTrackingArea: tracking];

    let _: () = msg_send![content_view, addSubview: preview];

    let _ = CLIPBOARD_PREVIEW.set(ClipboardPreviewRefs {
        root: preview as usize,
        title_field: title_field as usize,
        detail_field: detail_field as usize,
        action_bar: action_bar as usize,
        edit_button: edit_button as usize,
        remove_button: remove_button as usize,
        save_button: save_button as usize,
        cancel_button: cancel_button as usize,
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

fn image_from_path(path: &str) -> Option<id> {
    unsafe {
        let ns_path = NSString::alloc(nil).init_str(path);
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithContentsOfFile: ns_path];
        if image == nil {
            None
        } else {
            Some(image)
        }
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

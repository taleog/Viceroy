#![allow(unexpected_cfgs)]

use cocoa::base::{id, nil, YES};
use cocoa::foundation::NSString;
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::Ordering;

use crate::clipboard;
use crate::search_engine;
use crate::ui::clipboard_view::{
    apply_clipboard_history_state, build_clipboard_history_payload, show_clipboard_history_view,
    update_clipboard_preview_selection,
};
use crate::ui::helpers::run_on_main;
use crate::ui::settings_view;
use crate::ui::state::{
    TableMode, CURRENT_SEARCH, SEARCH_FIELD, SEARCH_RT, SEARCH_VERSION, TABLE_DATA, TABLE_MODE,
    TABLE_RESULTS,
};
use crate::ui::table;

pub unsafe fn register_search_delegate_class() {
    if objc::runtime::Class::get("MKSearchDelegate").is_some() {
        return;
    }
    let mut decl = ClassDecl::new("MKSearchDelegate", class!(NSObject)).unwrap();

    extern "C" fn begin_editing(_this: &Object, _cmd: Sel, notification: id) {
        unsafe {
            let object: id = msg_send![notification, object];
            if object == nil {
                return;
            }
            let window: id = msg_send![object, window];
            if window != nil {
                let field_editor: id = msg_send![window, fieldEditor:YES forObject:object];
                if field_editor != nil {
                    let white: id = msg_send![class!(NSColor), whiteColor];
                    let _: () = msg_send![field_editor, setInsertionPointColor: white];
                }
            }
        }
    }

    extern "C" fn changed(_this: &Object, _cmd: Sel, notification: id) {
        unsafe {
            let object: id = msg_send![notification, object];
            if object == nil {
                return;
            }

            let window: id = msg_send![object, window];
            if window != nil {
                let field_editor: id = msg_send![window, fieldEditor:YES forObject:object];
                if field_editor != nil {
                    let white: id = msg_send![class!(NSColor), whiteColor];
                    let _: () = msg_send![field_editor, setInsertionPointColor: white];
                }
            }

            let value: id = msg_send![object, stringValue];
            let cstr: *const std::os::raw::c_char = msg_send![value, UTF8String];
            if cstr.is_null() {
                return;
            }
            let query = std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string();
            eprintln!("[viceroy] search changed: '{}'", query);
            let search_version = SEARCH_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
            let mut is_clipboard_mode = match TABLE_MODE.lock() {
                Ok(mode) => *mode == TableMode::ClipboardHistory,
                Err(_) => false,
            };
            if !is_clipboard_mode {
                if let Ok(mut mode) = TABLE_MODE.lock() {
                    *mode = TableMode::Search;
                }
                update_clipboard_preview_selection(None);
                table::update_preview_layout(false);
                is_clipboard_mode = false;
            }

            if let Ok(mut handle_guard) = CURRENT_SEARCH.lock() {
                if let Some(handle) = handle_guard.take() {
                    handle.abort();
                }
            }

            if query.is_empty() {
                if is_clipboard_mode {
                    show_clipboard_history_view();
                } else {
                    if let Ok(mut tr) = TABLE_RESULTS.lock() {
                        tr.clear();
                    }
                    if let Ok(mut td) = TABLE_DATA.lock() {
                        td.clear();
                    }
                    table::schedule_table_update_next_tick();
                }
                return;
            }

            if is_clipboard_mode {
                let query_clone = query.clone();
                let handle = SEARCH_RT.spawn(async move {
                    if let Ok(entries) = clipboard::search_history(&query_clone).await {
                        let (rows, results) = build_clipboard_history_payload(entries);
                        if SEARCH_VERSION.load(Ordering::SeqCst) != search_version {
                            return;
                        }
                        run_on_main(move || {
                            apply_clipboard_history_state(rows, results);
                        });
                    }
                });
                if let Ok(mut handle_guard) = CURRENT_SEARCH.lock() {
                    *handle_guard = Some(handle);
                }
                return;
            }

            let query_clone = query.clone();
            let handle = SEARCH_RT.spawn(async move {
                if let Ok(results) = search_engine::search_fast(&query_clone).await {
                    if SEARCH_VERSION.load(Ordering::SeqCst) != search_version {
                        return;
                    }
                    let rows = build_search_rows(&results);
                    run_on_main(move || {
                        if SEARCH_VERSION.load(Ordering::SeqCst) != search_version {
                            return;
                        }
                        dispatch_search_results(results, rows);
                    });
                }

                if should_fetch_file_results(&query_clone) {
                    if let Ok(results) = search_engine::search(&query_clone).await {
                        if SEARCH_VERSION.load(Ordering::SeqCst) != search_version {
                            return;
                        }
                        let rows = build_search_rows(&results);
                        run_on_main(move || {
                            if SEARCH_VERSION.load(Ordering::SeqCst) != search_version {
                                return;
                            }
                            dispatch_search_results(results, rows);
                        });
                    }
                }
            });

            if let Ok(mut handle_guard) = CURRENT_SEARCH.lock() {
                *handle_guard = Some(handle);
            }
        }
    }

    unsafe {
        decl.add_method(
            sel!(controlTextDidBeginEditing:),
            begin_editing as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(controlTextDidChange:),
            changed as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }
}

pub fn build_search_rows(results: &[search_engine::SearchResult]) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    for r in results {
        match r {
            search_engine::SearchResult::Link {
                host, display_url, ..
            } => {
                rows.push((format!("Open {}", host), display_url.clone()));
            }
            search_engine::SearchResult::App { name, path, .. } => {
                rows.push((name.clone(), path.clone()));
            }
            search_engine::SearchResult::File { name, path, .. } => {
                rows.push((name.clone(), path.clone()));
            }
            search_engine::SearchResult::Clipboard {
                content,
                preview,
                custom_name,
                content_type,
                image_width,
                image_height,
                ..
            } => {
                if content_type == "image" {
                    let title = custom_name.clone().unwrap_or_else(|| "Image".to_string());
                    let subtitle = match (image_width, image_height) {
                        (Some(width), Some(height)) => format!("Image · {}x{} px", width, height),
                        _ => "Image".to_string(),
                    };
                    rows.push((title, subtitle));
                } else {
                    let title = custom_name
                        .clone()
                        .unwrap_or_else(|| content.chars().take(40).collect());
                    rows.push((title, preview.chars().take(80).collect()));
                }
            }
            search_engine::SearchResult::Command {
                name, description, ..
            } => {
                rows.push((name.clone(), description.clone()));
            }
            search_engine::SearchResult::Calculator {
                expression,
                result,
                formats,
            } => {
                rows.push((
                    format!("{} = {}", expression, result),
                    formats.join("  -  "),
                ));
            }
            search_engine::SearchResult::Emoji {
                emoji,
                name,
                keywords,
            } => {
                rows.push((format!("{} {}", emoji, name), keywords.join(", ")));
            }
            search_engine::SearchResult::Dictionary { word, preview } => {
                rows.push((format!("Define: {}", word), preview.clone()));
            }
            search_engine::SearchResult::WebSearch { query, engine, .. } => {
                rows.push((format!("Search {}", query), format!("Engine: {}", engine)));
            }
        }
    }
    rows
}

pub fn dispatch_search_results(
    results: Vec<search_engine::SearchResult>,
    rows: Vec<(String, String)>,
) {
    eprintln!(
        "[viceroy] search finished - dispatching UI update ({} results)",
        results.len()
    );
    if let Ok(mode) = TABLE_MODE.lock() {
        if *mode == TableMode::ClipboardHistory {
            return;
        }
    }
    if let Ok(mut tr) = TABLE_RESULTS.lock() {
        *tr = results;
    } else {
        eprintln!("WARNING: TABLE_RESULTS lock poisoned; UI update skipped");
    }
    if let Ok(mut td) = TABLE_DATA.lock() {
        *td = rows;
    } else {
        eprintln!("WARNING: TABLE_DATA lock poisoned; UI update skipped");
    }
    unsafe {
        table::reload_table();
    }
    table::schedule_table_update_next_tick();
}

pub fn should_fetch_file_results(query: &str) -> bool {
    query.len() >= 3 && !query.starts_with(':')
}

pub fn abort_current_search() {
    if let Ok(mut handle_guard) = CURRENT_SEARCH.lock() {
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }
}

pub unsafe fn find_search_field() -> Option<id> {
    if let Some(&ptr) = SEARCH_FIELD.get() {
        let field: id = ptr as id;
        if field != nil {
            return Some(field);
        }
    }
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return None;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content_view: id = msg_send![window, contentView];
    let subviews: id = msg_send![content_view, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count <= 1 {
        return None;
    }
    let container: id = msg_send![subviews, objectAtIndex:1];
    let container_subviews: id = msg_send![container, subviews];
    let csv_count: usize = msg_send![container_subviews, count];
    if csv_count == 0 {
        return None;
    }
    let search_field: id = msg_send![container_subviews, objectAtIndex:csv_count-1];
    if search_field == nil {
        return None;
    }
    Some(search_field)
}

#[allow(dead_code)]
pub unsafe fn get_current_search_query() -> Option<String> {
    if let Some(field) = find_search_field() {
        let value: id = msg_send![field, stringValue];
        let cstr: *const std::os::raw::c_char = msg_send![value, UTF8String];
        if cstr.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string())
        }
    } else {
        None
    }
}

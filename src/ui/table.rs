use crate::dictionary;
use crate::search_engine;
use crate::system_commands;
use crate::ui::helpers::{run_on_main, wrapped_row};
use crate::ui::state::{
    TableMode, ICON_CACHE, SEARCH_RT, TABLE_DATA, TABLE_MODE, TABLE_RESULTS, TABLE_UPDATE_PENDING,
    WINDOW_SHOWING,
};
use crate::usage;
use crate::web_search;
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

use crate::app_launcher;
use crate::ui::clipboard_view::icon_for_history_entry;

pub unsafe fn create_index_set(index: usize) -> id {
    msg_send![class!(NSIndexSet), indexSetWithIndex:index]
}

pub unsafe fn move_table_selection(down: bool) {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count < 3 {
        return;
    }
    let scroll: id = msg_send![subviews, objectAtIndex:2];
    let table: id = msg_send![scroll, documentView];

    let num_rows: isize = msg_send![table, numberOfRows];
    if num_rows == 0 {
        return;
    }

    let current_row: isize = msg_send![table, selectedRow];
    let new_row = wrapped_row(current_row, num_rows, down);
    if new_row < 0 {
        return;
    }

    let _: () = msg_send![table, selectRowIndexes:create_index_set(new_row as usize) byExtendingSelection:NO];
    let _: () = msg_send![table, scrollRowToVisible:new_row];
}

pub unsafe fn activate_selected_row_or_first() {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count < 3 {
        return;
    }
    let scroll: id = msg_send![subviews, objectAtIndex:2];
    let table: id = msg_send![scroll, documentView];
    let mut row: isize = msg_send![table, selectedRow];
    if row < 0 {
        row = 0;
    }
    perform_result_action(row as usize);
}

pub unsafe fn reload_table() {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count < 4 {
        return;
    }
    let scroll: id = msg_send![subviews, objectAtIndex:2];
    let table: id = msg_send![scroll, documentView];
    let _: () = msg_send![table, reloadData];

    let num_rows: isize = msg_send![table, numberOfRows];
    if num_rows > 0 {
        let index_set = create_index_set(0);
        let _: () = msg_send![table, selectRowIndexes:index_set byExtendingSelection:NO];
    }
}

pub unsafe fn resize_window_for_results() {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count < 2 {
        return;
    }

    let num_results = match TABLE_DATA.lock() {
        Ok(g) => g.len(),
        Err(_) => 0,
    };
    let base_height = 106.0 + 22.0;
    let row_height = 60.0;
    let max_visible_rows = 8;
    let visible_rows = num_results.min(max_visible_rows);
    let new_height = if visible_rows == 0 {
        base_height
    } else {
        base_height + (visible_rows as f64 * row_height) + 10.0
    };

    let current_frame: NSRect = msg_send![window, frame];
    if (current_frame.size.height - new_height).abs() < 0.5 {
        return;
    }

    let new_frame = NSRect::new(
        NSPoint::new(
            current_frame.origin.x,
            current_frame.origin.y + (current_frame.size.height - new_height),
        ),
        NSSize::new(current_frame.size.width, new_height),
    );

    if sv_count >= 2 {
        let container: id = msg_send![subviews, objectAtIndex:1];
        let new_search_y = new_height - 80.0;
        let new_search_frame = NSRect::new(
            NSPoint::new(20.0, new_search_y),
            NSSize::new(current_frame.size.width - 40.0, 60.0),
        );
        let _: () = msg_send![container, setFrame:new_search_frame];
    }

    let _: () = msg_send![window, setFrame:new_frame display:YES animate:NO];
}

pub fn schedule_table_update_next_tick() {
    if TABLE_UPDATE_PENDING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    run_on_main(move || unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        if count == 0 {
            TABLE_UPDATE_PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
            return;
        }
        let window: id = msg_send![windows, objectAtIndex:0];
        let content: id = msg_send![window, contentView];
        let subviews: id = msg_send![content, subviews];
        let sv_count: usize = msg_send![subviews, count];
        if sv_count >= 3 {
            let scroll: id = msg_send![subviews, objectAtIndex:2];
            let table: id = msg_send![scroll, documentView];
            let _: () = msg_send![table, reloadData];
            let num_rows: isize = msg_send![table, numberOfRows];
            if num_rows > 0 {
                let index_set: id = msg_send![class!(NSIndexSet), indexSetWithIndex:0];
                let _: () = msg_send![table, selectRowIndexes:index_set byExtendingSelection:NO];
            }
        }
        resize_window_for_results();
        TABLE_UPDATE_PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

pub unsafe fn register_table_delegate_class() {
    if objc::runtime::Class::get("MKTableDelegate").is_some() {
        return;
    }
    let mut decl = ClassDecl::new("MKTableDelegate", class!(NSObject)).unwrap();

    extern "C" fn rows(_this: &Object, _cmd: Sel, _table: id) -> isize {
        match TABLE_DATA.lock() {
            Ok(g) => g.len() as isize,
            Err(_) => 0,
        }
    }

    extern "C" fn view_for_row(_this: &Object, _cmd: Sel, table: id, _col: id, row: isize) -> id {
        unsafe {
            let entries = match TABLE_DATA.lock() {
                Ok(g) => g,
                Err(_) => return nil,
            };
            let results = match TABLE_RESULTS.lock() {
                Ok(g) => g,
                Err(_) => return nil,
            };
            if row < 0 || row as usize >= entries.len() {
                return nil;
            }
            let (raw_title, raw_subtitle) = &entries[row as usize];

            let frame: NSRect = msg_send![table, frame];
            let identifier = NSString::alloc(nil).init_str("MKRowView");
            let mut container: id = msg_send![table, makeViewWithIdentifier:identifier owner:nil];

            let mut title = raw_title.clone();
            let mut subtitle = raw_subtitle.clone();
            let mut type_label_str = String::new();
            let mut icon_image: id = nil;

            let mode = match TABLE_MODE.lock() {
                Ok(m) => *m,
                Err(_) => TableMode::Search,
            };
            let mut handled_history = false;
            if mode == TableMode::ClipboardHistory && (row as usize) < results.len() {
                if let search_engine::SearchResult::Clipboard { content_type, .. } =
                    &results[row as usize]
                {
                    type_label_str = match content_type.as_str() {
                        "image" => "Image".to_string(),
                        _ => "Text".to_string(),
                    };
                    icon_image = icon_for_history_entry(&results[row as usize], row);
                    handled_history = true;
                }
            }
            if !handled_history {
                if (row as usize) < results.len() {
                    match &results[row as usize] {
                        search_engine::SearchResult::App { path, .. } => {
                            subtitle = std::panic::catch_unwind(|| format_pretty_path(path))
                                .unwrap_or_default();
                            type_label_str = "Application".to_string();
                            if let Ok(cache) = ICON_CACHE.lock() {
                                if let Some(cached) = cache.get(path) {
                                    icon_image = *cached as id;
                                } else {
                                    drop(cache);
                                    let placeholder_name = NSString::alloc(nil).init_str("app");
                                    icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:placeholder_name accessibilityDescription:nil];
                                    let path_clone = path.clone();
                                    let row_index = row;
                                    SEARCH_RT.spawn_blocking(move || {
                                        let workspace: id =
                                            msg_send![class!(NSWorkspace), sharedWorkspace];
                                        let path_str = NSString::alloc(nil).init_str(&path_clone);
                                        let img: id = msg_send![workspace, iconForFile: path_str];
                                        if img != nil {
                                            let img_ptr = img as usize;
                                            run_on_main(move || {
                                                let img_for_main: id = img_ptr as id;
                                                let _: id = msg_send![img_for_main, retain];
                                                if let Ok(mut cache) = ICON_CACHE.lock() {
                                                    cache.insert(
                                                        path_clone.clone(),
                                                        img_for_main as usize,
                                                    );
                                                }
                                                set_icon_for_row_from_cache(&path_clone, row_index);
                                            });
                                        }
                                    });
                                }
                            }
                        }
                        search_engine::SearchResult::File { path, .. } => {
                            let p = std::path::Path::new(path);
                            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                                title = name.to_string();
                            }
                            subtitle = std::panic::catch_unwind(|| format_pretty_path(path))
                                .unwrap_or_default();
                            type_label_str = "File".to_string();
                            if let Ok(cache) = ICON_CACHE.lock() {
                                if let Some(cached) = cache.get(path) {
                                    icon_image = *cached as id;
                                } else {
                                    drop(cache);
                                    let placeholder_name = NSString::alloc(nil).init_str("doc");
                                    icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:placeholder_name accessibilityDescription:nil];
                                    let path_clone = path.clone();
                                    let row_index = row;
                                    SEARCH_RT.spawn_blocking(move || {
                                        let workspace: id =
                                            msg_send![class!(NSWorkspace), sharedWorkspace];
                                        let path_str = NSString::alloc(nil).init_str(&path_clone);
                                        let img: id = msg_send![workspace, iconForFile: path_str];
                                        if img != nil {
                                            let img_ptr = img as usize;
                                            run_on_main(move || {
                                                let img_for_main: id = img_ptr as id;
                                                let _: id = msg_send![img_for_main, retain];
                                                if let Ok(mut cache) = ICON_CACHE.lock() {
                                                    cache.insert(
                                                        path_clone.clone(),
                                                        img_for_main as usize,
                                                    );
                                                }
                                                set_icon_for_row_from_cache(&path_clone, row_index);
                                            });
                                        }
                                    });
                                }
                            }
                        }
                        search_engine::SearchResult::Clipboard { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("doc.on.clipboard");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Clipboard".to_string();
                        }
                        search_engine::SearchResult::Calculator { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("function");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Calculator".to_string();
                        }
                        search_engine::SearchResult::Emoji { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("face.smiling");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Emoji".to_string();
                        }
                        search_engine::SearchResult::Command { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("terminal");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Command".to_string();
                        }
                        search_engine::SearchResult::Dictionary { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("book");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Dictionary".to_string();
                        }
                        search_engine::SearchResult::WebSearch { .. } => {
                            let symbol_name = NSString::alloc(nil).init_str("magnifyingglass");
                            icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];
                            type_label_str = "Search".to_string();
                        }
                    }
                } else {
                    let (t, _) = &entries[row as usize];
                    let (symbol_name, label) = match t.as_str() {
                        "Calculator" => ("function", "Calculator"),
                        "Open Safari" => ("safari", "App"),
                        "Clipboard" => ("doc.on.clipboard", "Clipboard"),
                        "Settings" => ("gearshape", "Settings"),
                        "Emoji Picker" => ("face.smiling", "Emoji"),
                        _ => ("app", ""),
                    };
                    let symbol_name_ns = NSString::alloc(nil).init_str(symbol_name);
                    icon_image = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name_ns accessibilityDescription:nil];
                    type_label_str = label.to_string();
                }
            }

            if container == nil {
                let new_container: id = msg_send![class!(NSView), alloc];
                let new_container: id = msg_send![new_container, initWithFrame: NSRect::new(NSPoint::new(0.0,0.0), NSSize::new(frame.size.width, 60.0))];
                let _: () = msg_send![new_container, setWantsLayer: YES];

                let icon_view: id = msg_send![class!(NSImageView), alloc];
                let icon_view: id = msg_send![icon_view, initWithFrame: NSRect::new(NSPoint::new(12.0, 10.0), NSSize::new(40.0, 40.0))];
                let _: () = msg_send![icon_view, setImageScaling: 1];
                let _: () = msg_send![new_container, addSubview: icon_view];

                let title_field: id = msg_send![class!(NSTextField), alloc];
                let title_field: id = msg_send![title_field, initWithFrame: NSRect::new(NSPoint::new(60.0, 31.0), NSSize::new(frame.size.width - 140.0, 20.0))];
                let _: () = msg_send![title_field, setBezeled: NO];
                let _: () = msg_send![title_field, setEditable: NO];
                let _: () = msg_send![title_field, setDrawsBackground: NO];
                let _: () = msg_send![title_field, setBordered: NO];
                let font_title: id = msg_send![class!(NSFont), systemFontOfSize:15.0 weight:0.4];
                let _: () = msg_send![title_field, setFont: font_title];
                let _: () = msg_send![new_container, addSubview: title_field];

                let subtitle_field: id = msg_send![class!(NSTextField), alloc];
                let subtitle_field: id = msg_send![subtitle_field, initWithFrame: NSRect::new(NSPoint::new(60.0, 11.0), NSSize::new(frame.size.width - 140.0, 16.0))];
                let _: () = msg_send![subtitle_field, setBezeled: NO];
                let _: () = msg_send![subtitle_field, setEditable: NO];
                let _: () = msg_send![subtitle_field, setDrawsBackground: NO];
                let _: () = msg_send![subtitle_field, setBordered: NO];
                let font_sub: id = msg_send![class!(NSFont), systemFontOfSize:13.0];
                let _: () = msg_send![subtitle_field, setFont: font_sub];
                let _: () = msg_send![new_container, addSubview: subtitle_field];

                let type_label_field: id = msg_send![class!(NSTextField), alloc];
                let type_label_field: id = msg_send![type_label_field, initWithFrame: NSRect::new(NSPoint::new(frame.size.width - 140.0, 22.0), NSSize::new(100.0, 16.0))];
                let _: () = msg_send![type_label_field, setBezeled: NO];
                let _: () = msg_send![type_label_field, setEditable: NO];
                let _: () = msg_send![type_label_field, setDrawsBackground: NO];
                let _: () = msg_send![type_label_field, setBordered: NO];
                let _: () = msg_send![type_label_field, setAlignment: 2];
                let type_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0];
                let _: () = msg_send![type_label_field, setFont: type_font];
                let _: () = msg_send![new_container, addSubview: type_label_field];

                let _: () = msg_send![new_container, setIdentifier: identifier];
                container = new_container;
            }

            let subviews: id = msg_send![container, subviews];
            let icon_view: id = msg_send![subviews, objectAtIndex:0];
            if icon_image != nil {
                let _: () = msg_send![icon_view, setImage: icon_image];
            }

            let title_field: id = msg_send![subviews, objectAtIndex:1];
            let _: () =
                msg_send![title_field, setStringValue: NSString::alloc(nil).init_str(&title)];

            let subtitle_field: id = msg_send![subviews, objectAtIndex:2];
            let _: () =
                msg_send![subtitle_field, setStringValue: NSString::alloc(nil).init_str(&subtitle)];

            let type_field: id = msg_send![subviews, objectAtIndex:3];
            let _: () = msg_send![type_field, setStringValue: NSString::alloc(nil).init_str(&type_label_str)];

            container
        }
    }

    extern "C" fn selection_changed(_this: &Object, _cmd: Sel, _note: id) {}

    decl.add_method(
        sel!(numberOfRowsInTableView:),
        rows as extern "C" fn(&Object, Sel, id) -> isize,
    );
    decl.add_method(
        sel!(tableView:viewForTableColumn:row:),
        view_for_row as extern "C" fn(&Object, Sel, id, id, isize) -> id,
    );
    decl.add_method(
        sel!(tableViewSelectionDidChange:),
        selection_changed as extern "C" fn(&Object, Sel, id),
    );
    decl.register();
}

pub unsafe fn set_icon_for_row_from_cache(path: &str, row: isize) {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];
    if sv_count < 3 {
        return;
    }
    let scroll: id = msg_send![subviews, objectAtIndex:2];
    let table: id = msg_send![scroll, documentView];

    if row < 0 {
        return;
    }
    let num_rows: usize = msg_send![table, numberOfRows];
    if (row as usize) >= num_rows {
        return;
    }
    let row_view: id = msg_send![table, viewAtColumn:0 row:row makeIfNecessary:NO];
    if row_view == nil {
        return;
    }

    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(&cached) = cache.get(path) {
            let img: id = cached as id;
            if img != nil {
                let subviews: id = msg_send![row_view, subviews];
                if subviews != nil {
                    let icon_view: id = msg_send![subviews, objectAtIndex:0];
                    if icon_view != nil {
                        let _: () = msg_send![icon_view, setImage: img];
                    }
                }
            }
        }
    }
}

fn format_pretty_path(path: &str) -> String {
    use std::path::Path;
    let path_obj = Path::new(path);
    let components: Vec<_> = path_obj
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    components.join(" › ")
}

unsafe fn perform_result_action(index: usize) {
    let results = match TABLE_RESULTS.lock() {
        Ok(g) => g.clone(),
        Err(_) => return,
    };
    if index >= results.len() {
        return;
    }
    let result = results[index].clone();

    match result {
        search_engine::SearchResult::App { path, .. } => {
            usage::record_app_launch(&path);
            let _ = app_launcher::launch(&path);
        }
        search_engine::SearchResult::File { path, .. } => {
            let _ = app_launcher::open_file(&path);
        }
        search_engine::SearchResult::Clipboard { content, .. } => {
            let content_clone = content.clone();
            SEARCH_RT.spawn(async move {
                let _ = crate::clipboard::paste_to_active_app(&content_clone).await;
            });
        }
        search_engine::SearchResult::Command { command, .. } => {
            SEARCH_RT.spawn(async move {
                let _ = system_commands::execute(&command).await;
            });
        }
        search_engine::SearchResult::Calculator { result, .. } => {
            let to_paste = result.clone();
            SEARCH_RT.spawn(async move {
                let _ = crate::clipboard::paste_to_active_app(&to_paste).await;
            });
        }
        search_engine::SearchResult::Emoji { emoji, .. } => {
            let to_paste = emoji.clone();
            SEARCH_RT.spawn(async move {
                let _ = crate::clipboard::paste_to_active_app(&to_paste).await;
            });
        }
        search_engine::SearchResult::Dictionary { word, .. } => {
            let _ = dictionary::open_dictionary(&word);
        }
        search_engine::SearchResult::WebSearch { url, .. } => {
            let _ = web_search::open_web_search(&url);
        }
    }

    if let Ok(mut showing) = WINDOW_SHOWING.lock() {
        *showing = false;
    }
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count > 0 {
        let window: id = msg_send![windows, objectAtIndex:0];
        let _: () = msg_send![window, orderOut: nil];
    }
}

use crate::dictionary;
use crate::search_engine;
use crate::system_commands;
use crate::ui::helpers::style;
use crate::ui::helpers::{run_on_main, wrapped_row};
use crate::ui::state::{
    TableMode, CLIPBOARD_PREVIEW, ICON_CACHE, SEARCH_RT, TABLE_DATA, TABLE_MODE, TABLE_RESULTS,
    TABLE_SCROLL_VIEW, TABLE_UPDATE_PENDING, WINDOW_IS_OPEN, WINDOW_SHOWING,
};
use crate::usage;
use crate::web_search;
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;

use crate::app_launcher;
use crate::ui::clipboard_view::{
    icon_for_history_entry, refresh_clipboard_preview_layout, update_clipboard_preview_selection,
};

pub use crate::ui::helpers::style::ROW_HEIGHT;

fn sanitize_coordinate(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn sanitize_dimension(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

fn layout_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("VICEROY_LAYOUT_DEBUG")
            .map(|v| v != "0")
            .unwrap_or(false)
    })
}

fn register_constrained_clip_view_class() {
    unsafe {
        if objc::runtime::Class::get("MKConstrainedClipView").is_some() {
            return;
        }
        let superclass = class!(NSClipView);
        let mut decl = ClassDecl::new("MKConstrainedClipView", superclass).unwrap();

        extern "C" fn constrain_bounds_rect(
            this: &Object,
            _cmd: Sel,
            mut proposed: NSRect,
        ) -> NSRect {
            proposed.origin.x = 0.0;
            proposed.origin.y = sanitize_coordinate(proposed.origin.y);
            proposed.size.width = sanitize_dimension(proposed.size.width);
            proposed.size.height = sanitize_dimension(proposed.size.height);
            unsafe { msg_send![super(this, class!(NSClipView)), constrainBoundsRect: proposed] }
        }

        extern "C" fn scroll_to_point(this: &Object, _cmd: Sel, mut point: NSPoint) {
            point.x = 0.0;
            point.y = sanitize_coordinate(point.y);
            unsafe {
                let _: () = msg_send![super(this, class!(NSClipView)), scrollToPoint: point];
            }
        }

        decl.add_method(
            sel!(constrainBoundsRect:),
            constrain_bounds_rect as extern "C" fn(&Object, Sel, NSRect) -> NSRect,
        );
        decl.add_method(
            sel!(scrollToPoint:),
            scroll_to_point as extern "C" fn(&Object, Sel, NSPoint),
        );
        decl.register();
    }
}

pub unsafe fn install_constrained_clip_view(scroll: id, initial_bounds: NSRect) {
    register_constrained_clip_view_class();
    let clip_class = class!(MKConstrainedClipView);
    let clip_view: id = msg_send![clip_class, alloc];
    let clip_view: id = msg_send![clip_view, initWithFrame: initial_bounds];
    let _: () = msg_send![clip_view, setDrawsBackground: NO];
    let _: () = msg_send![clip_view, setCopiesOnScroll: NO];
    let _: () = msg_send![clip_view, setPostsBoundsChangedNotifications: YES];
    let _: () = msg_send![scroll, setContentView: clip_view];
}

const COLLAPSED_RESULT_AREA_HEIGHT: f64 = 0.0;
const OPEN_RESULT_AREA_HEIGHT: f64 = 420.0;
const SETTINGS_WINDOW_HEIGHT: f64 = 760.0;
const WINDOW_HEIGHT_OPEN: f64 =
    style::TABLE_TOP_OFFSET + style::TABLE_FOOTER_HEIGHT + OPEN_RESULT_AREA_HEIGHT;
const WINDOW_HEIGHT_COLLAPSED: f64 =
    style::TABLE_TOP_OFFSET + style::TABLE_FOOTER_HEIGHT + COLLAPSED_RESULT_AREA_HEIGHT;

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

pub unsafe fn delete_selected_clipboard_entry() {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    if mode != TableMode::ClipboardHistory {
        return;
    }

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

    let row: isize = msg_send![table, selectedRow];
    if row < 0 {
        return;
    }

    let entry_id = {
        let mut results = match TABLE_RESULTS.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if row as usize >= results.len() {
            return;
        }
        match &results[row as usize] {
            crate::search_engine::SearchResult::Clipboard { id, .. } => {
                let entry_id = *id;
                results.remove(row as usize);
                if let Ok(mut td) = TABLE_DATA.lock() {
                    if row as usize <= td.len().saturating_sub(1) {
                        td.remove(row as usize);
                    }
                }
                entry_id
            }
            _ => return,
        }
    };

    SEARCH_RT.spawn(async move {
        let _ = crate::clipboard::delete_entry(entry_id).await;
    });

    let _: () = msg_send![table, reloadData];
    let num_rows: isize = msg_send![table, numberOfRows];
    if num_rows > 0 {
        let new_row = if row < num_rows { row } else { num_rows - 1 };
        let index_set: id = msg_send![class!(NSIndexSet), indexSetWithIndex:new_row as usize];
        let _: () = msg_send![table, selectRowIndexes:index_set byExtendingSelection:NO];
        let _: () = msg_send![table, scrollRowToVisible:new_row];
        crate::ui::clipboard_view::update_clipboard_preview_selection(Some(new_row as usize));
    } else {
        crate::ui::clipboard_view::update_clipboard_preview_selection(None);
    }
    sync_window_height_with_state();
}

pub unsafe fn edit_selected_clipboard_entry() {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    if mode != TableMode::ClipboardHistory {
        return;
    }

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

    let row: isize = msg_send![table, selectedRow];
    if row < 0 {
        return;
    }

    let entry = {
        let results = match TABLE_RESULTS.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if row as usize >= results.len() {
            return;
        }
        results[row as usize].clone()
    };

    crate::ui::clipboard_view::begin_inline_edit(entry);
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

fn should_window_open_for_mode(mode: TableMode) -> bool {
    match mode {
        TableMode::ClipboardHistory | TableMode::Settings => true,
        TableMode::Search => match TABLE_DATA.lock() {
            Ok(data) => !data.is_empty(),
            Err(_) => true,
        },
    }
}

pub unsafe fn sync_window_height_with_state() {
    let mode = match TABLE_MODE.lock() {
        Ok(m) => *m,
        Err(_) => TableMode::Search,
    };
    let open = should_window_open_for_mode(mode);
    let target_height = match mode {
        TableMode::Settings => SETTINGS_WINDOW_HEIGHT,
        _ if open => WINDOW_HEIGHT_OPEN,
        _ => WINDOW_HEIGHT_COLLAPSED,
    };
    let preview_visible = mode == TableMode::ClipboardHistory;

    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex:0];
    let current_frame: NSRect = msg_send![window, frame];
    let previously_open = WINDOW_IS_OPEN.load(Ordering::SeqCst);
    if previously_open == open && (current_frame.size.height - target_height).abs() < 0.5 {
        update_preview_layout(preview_visible);
        return;
    }
    WINDOW_IS_OPEN.store(open, Ordering::SeqCst);
    let new_origin_y = current_frame.origin.y + (current_frame.size.height - target_height);
    let new_frame = NSRect::new(
        NSPoint::new(current_frame.origin.x, new_origin_y),
        NSSize::new(current_frame.size.width, target_height),
    );

    let content: id = msg_send![window, contentView];
    let subviews: id = msg_send![content, subviews];
    let sv_count: usize = msg_send![subviews, count];

    if sv_count >= 2 {
        let container: id = msg_send![subviews, objectAtIndex:1];
        let new_search_y = target_height - style::TABLE_TOP_OFFSET;
        let new_search_frame = NSRect::new(
            NSPoint::new(20.0, new_search_y),
            NSSize::new(current_frame.size.width - 40.0, style::SEARCH_BAR_HEIGHT),
        );
        let _: () = msg_send![container, setFrame:new_search_frame];
    }

    let _: () = msg_send![window, setFrame:new_frame display:YES animate:NO];
    update_preview_layout(preview_visible);
}

pub fn update_preview_layout(preview_visible: bool) {
    let scroll_ptr = match TABLE_SCROLL_VIEW.get() {
        Some(ptr) => *ptr,
        None => return,
    };
    unsafe {
        let scroll: id = scroll_ptr as id;
        if scroll == nil {
            return;
        }
        let parent: id = msg_send![scroll, superview];
        if parent == nil {
            return;
        }
        let bounds: NSRect = msg_send![parent, bounds];
        let table_height =
            (bounds.size.height - style::TABLE_TOP_OFFSET - style::TABLE_FOOTER_HEIGHT).max(0.0);
        let horizontal_margin = style::CONTENT_SIDE_INSET + style::LIST_EXTRA_MARGIN;
        let available_width =
            (bounds.size.width - horizontal_margin * 2.0).max(style::LIST_MIN_WIDTH);
        let list_width = if preview_visible {
            (available_width * style::LIST_WIDTH_RATIO).max(style::LIST_MIN_WIDTH)
        } else {
            available_width
        };
        let preview_width = if preview_visible {
            (available_width - list_width - style::PREVIEW_GAP).max(style::PREVIEW_MIN_WIDTH)
        } else {
            0.0
        };
        let list_origin = NSPoint::new(horizontal_margin, style::TABLE_FOOTER_HEIGHT);
        let scroll_frame = NSRect::new(list_origin, NSSize::new(list_width, table_height));
        let _: () = msg_send![scroll, setFrame: scroll_frame];
        let _: () = msg_send![scroll, tile];
        let clip_view: id = msg_send![scroll, contentView];
        if clip_view != nil {
            let clip_frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(list_width, table_height),
            );
            let _: () = msg_send![clip_view, setFrame: clip_frame];
            let current_bounds: NSRect = msg_send![clip_view, bounds];
            let new_bounds = NSRect::new(
                NSPoint::new(0.0, sanitize_coordinate(current_bounds.origin.y)),
                clip_frame.size,
            );
            let _: () = msg_send![clip_view, setBounds: new_bounds];
        }
        let table_view: id = msg_send![scroll, documentView];
        let column_width = (list_width - style::LIST_SCROLL_GUTTER).max(200.0);
        if table_view != nil {
            // Make document height reflect total rows so vertical scrolling works.
            let num_rows: isize = msg_send![table_view, numberOfRows];
            // Include inter-row spacing so the last rows remain reachable.
            let row_stride = style::ROW_HEIGHT + style::ROW_STACK_SPACING;
            let content_height = ((num_rows as f64) * row_stride).max(table_height);
            let doc_frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(column_width, content_height),
            );
            let _: () = msg_send![table_view, setFrame: doc_frame];
            let columns: id = msg_send![table_view, tableColumns];
            let col_count: usize = msg_send![columns, count];
            if col_count > 0 {
                let column: id = msg_send![columns, objectAtIndex:0];
                let _: () = msg_send![column, setWidth:column_width];
            }
        }
        if let Some(refs) = CLIPBOARD_PREVIEW.get() {
            let preview: id = refs.root as id;
            if preview != nil {
                if preview_visible {
                    let preview_origin_x = horizontal_margin + list_width + style::PREVIEW_GAP;
                    let preview_frame = NSRect::new(
                        NSPoint::new(preview_origin_x, style::TABLE_FOOTER_HEIGHT),
                        NSSize::new(preview_width, table_height),
                    );
                    let _: () = msg_send![preview, setHidden: NO];
                    let _: () = msg_send![preview, setFrame: preview_frame];
                } else {
                    let _: () = msg_send![preview, setHidden: YES];
                }
            }
        }
        refresh_clipboard_preview_layout();
        if layout_debug_enabled() {
            let clip_bounds: NSRect = if clip_view != nil {
                msg_send![clip_view, bounds]
            } else {
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
            };
            println!(
                "[layout] update_preview preview_visible={} list_width={:.1} preview_width={:.1} scroll_frame=origin=({:.1},{:.1}) size=({:.1}×{:.1}) clip_bounds=origin=({:.1},{:.1}) size=({:.1}×{:.1}) column_width={:.1}",
                preview_visible,
                list_width,
                preview_width,
                scroll_frame.origin.x,
                scroll_frame.origin.y,
                scroll_frame.size.width,
                scroll_frame.size.height,
                clip_bounds.origin.x,
                clip_bounds.origin.y,
                clip_bounds.size.width,
                clip_bounds.size.height,
                column_width
            );
        }
    }
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
                let mode = match TABLE_MODE.lock() {
                    Ok(m) => *m,
                    Err(_) => TableMode::Search,
                };
                if mode == TableMode::ClipboardHistory {
                    update_clipboard_preview_selection(Some(0));
                } else {
                    update_clipboard_preview_selection(None);
                }
            } else {
                update_clipboard_preview_selection(None);
            }
        }
        sync_window_height_with_state();
        TABLE_UPDATE_PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

fn register_row_highlight_view_class() {
    unsafe {
        if objc::runtime::Class::get("MKRowHighlightView").is_some() {
            return;
        }
        let superclass = class!(NSTableRowView);
        let mut decl = ClassDecl::new("MKRowHighlightView", superclass).unwrap();

        extern "C" fn draw_selection(_this: &Object, _cmd: Sel, _rect: NSRect) {}
        decl.add_method(
            sel!(drawSelectionInRect:),
            draw_selection as extern "C" fn(&Object, Sel, NSRect),
        );

        extern "C" fn draw_background(_this: &Object, _cmd: Sel, _rect: NSRect) {}
        decl.add_method(
            sel!(drawBackgroundInRect:),
            draw_background as extern "C" fn(&Object, Sel, NSRect),
        );

        decl.register();
    }
}

pub unsafe fn register_table_delegate_class() {
    if objc::runtime::Class::get("MKTableDelegate").is_some() {
        return;
    }
    register_row_highlight_view_class();
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
            let row_width = frame.size.width.max(40.0);
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
            let clipboard_mode = mode == TableMode::ClipboardHistory;
            let mut handled_history = false;
            if clipboard_mode && (row as usize) < results.len() {
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

            let inset = style::ROW_HORIZONTAL_INSET;
            let container_height = style::ROW_HEIGHT - style::ROW_STACK_SPACING;
            let container_frame = NSRect::new(
                NSPoint::new(inset, style::ROW_VERTICAL_PADDING),
                NSSize::new(row_width - inset * 2.0, container_height),
            );
            let container_width = container_frame.size.width;
            let selected_flag: BOOL = msg_send![table, isRowSelected:row];
            let is_selected = selected_flag == YES;

            if container == nil {
                let new_container: id = msg_send![class!(NSView), alloc];
                let new_container: id = msg_send![new_container, initWithFrame: container_frame];
                let _: () = msg_send![new_container, setWantsLayer: YES];

                let icon_view: id = msg_send![class!(NSImageView), alloc];
                let icon_view: id = msg_send![icon_view, initWithFrame: NSRect::new(NSPoint::new(style::ROW_INTERNAL_PADDING, style::ROW_VERTICAL_PADDING), NSSize::new(style::ROW_ICON_SIZE, style::ROW_ICON_SIZE))];
                let _: () = msg_send![icon_view, setImageScaling: 1];
                let _: () = msg_send![new_container, addSubview: icon_view];

                let title_field: id = msg_send![class!(NSTextField), alloc];
                let title_initial_width = (container_width
                    - (style::ROW_INTERNAL_PADDING
                        + style::ROW_ICON_SIZE
                        + style::ROW_ICON_TEXT_PADDING)
                    - style::ROW_TYPE_LABEL_WIDTH
                    - style::ROW_TRAILING_PADDING)
                    .max(120.0);
                let title_field: id = msg_send![title_field, initWithFrame: NSRect::new(NSPoint::new(style::ROW_INTERNAL_PADDING + style::ROW_ICON_SIZE + style::ROW_ICON_TEXT_PADDING, container_height - style::ROW_TITLE_HEIGHT - style::ROW_TEXT_SPACING), NSSize::new(title_initial_width, style::ROW_TITLE_HEIGHT))];
                let _: () = msg_send![title_field, setBezeled: NO];
                let _: () = msg_send![title_field, setEditable: NO];
                let _: () = msg_send![title_field, setDrawsBackground: NO];
                let _: () = msg_send![title_field, setBordered: NO];
                let font_title: id = msg_send![class!(NSFont), systemFontOfSize:16.0 weight:0.6];
                let _: () = msg_send![title_field, setFont: font_title];
                let _: () = msg_send![new_container, addSubview: title_field];

                let subtitle_field: id = msg_send![class!(NSTextField), alloc];
                let subtitle_field: id = msg_send![subtitle_field, initWithFrame: NSRect::new(NSPoint::new(style::ROW_INTERNAL_PADDING + style::ROW_ICON_SIZE + style::ROW_ICON_TEXT_PADDING, style::ROW_VERTICAL_PADDING), NSSize::new(title_initial_width, style::ROW_SUBTITLE_HEIGHT))];
                let _: () = msg_send![subtitle_field, setBezeled: NO];
                let _: () = msg_send![subtitle_field, setEditable: NO];
                let _: () = msg_send![subtitle_field, setDrawsBackground: NO];
                let _: () = msg_send![subtitle_field, setBordered: NO];
                let font_sub: id = msg_send![class!(NSFont), systemFontOfSize:13.0 weight:0.3];
                let _: () = msg_send![subtitle_field, setFont: font_sub];
                let _: () = msg_send![new_container, addSubview: subtitle_field];

                let type_label_field: id = msg_send![class!(NSTextField), alloc];
                let type_initial_x =
                    container_width - style::ROW_TYPE_LABEL_WIDTH - style::ROW_TRAILING_PADDING;
                let type_label_field: id = msg_send![type_label_field, initWithFrame: NSRect::new(NSPoint::new(type_initial_x, (container_height - style::ROW_SUBTITLE_HEIGHT) / 2.0), NSSize::new(style::ROW_TYPE_LABEL_WIDTH, style::ROW_SUBTITLE_HEIGHT))];
                let _: () = msg_send![type_label_field, setBezeled: NO];
                let _: () = msg_send![type_label_field, setEditable: NO];
                let _: () = msg_send![type_label_field, setDrawsBackground: NO];
                let _: () = msg_send![type_label_field, setBordered: NO];
                let _: () = msg_send![type_label_field, setAlignment: 2];
                let type_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.6];
                let _: () = msg_send![type_label_field, setFont: type_font];
                let _: () = msg_send![new_container, addSubview: type_label_field];

                let _: () = msg_send![new_container, setIdentifier: identifier];
                container = new_container;
            }

            let _: () = msg_send![container, setFrame: container_frame];
            let subviews: id = msg_send![container, subviews];
            let icon_view: id = msg_send![subviews, objectAtIndex:0];
            let icon_size = style::ROW_ICON_SIZE;
            let icon_y = (container_height - icon_size - style::ROW_CONTENT_TOP_INSET)
                .max(style::ROW_VERTICAL_PADDING);
            let _: () = msg_send![icon_view, setFrame: NSRect::new(NSPoint::new(style::ROW_INTERNAL_PADDING, icon_y), NSSize::new(icon_size, icon_size))];
            if icon_image != nil {
                let _: () = msg_send![icon_view, setImage: icon_image];
            }

            let title_field: id = msg_send![subviews, objectAtIndex:1];
            let text_x =
                style::ROW_INTERNAL_PADDING + style::ROW_ICON_SIZE + style::ROW_ICON_TEXT_PADDING;
            let type_label_width = style::ROW_TYPE_LABEL_WIDTH;
            let text_width =
                (container_width - text_x - type_label_width - style::ROW_TRAILING_PADDING)
                    .max(120.0);
            let text_block_height =
                style::ROW_TITLE_HEIGHT + style::ROW_SUBTITLE_HEIGHT + style::ROW_TEXT_SPACING;
            let text_block_origin_y =
                (container_height - text_block_height - style::ROW_CONTENT_TOP_INSET)
                    .max(style::ROW_VERTICAL_PADDING);
            let title_y =
                text_block_origin_y + style::ROW_SUBTITLE_HEIGHT + style::ROW_TEXT_SPACING;
            let _: () = msg_send![title_field, setFrame: NSRect::new(NSPoint::new(text_x, title_y), NSSize::new(text_width, style::ROW_TITLE_HEIGHT))];
            let _: () =
                msg_send![title_field, setStringValue: NSString::alloc(nil).init_str(&title)];

            let subtitle_field: id = msg_send![subviews, objectAtIndex:2];
            let subtitle_y = text_block_origin_y;
            let _: () = msg_send![subtitle_field, setFrame: NSRect::new(NSPoint::new(text_x, subtitle_y), NSSize::new(text_width, style::ROW_SUBTITLE_HEIGHT))];
            let _: () =
                msg_send![subtitle_field, setStringValue: NSString::alloc(nil).init_str(&subtitle)];

            let type_field: id = msg_send![subviews, objectAtIndex:3];
            let type_x = container_width - type_label_width - style::ROW_TRAILING_PADDING;
            let type_height = style::ROW_SUBTITLE_HEIGHT;
            let type_y = (container_height - type_height - style::ROW_CONTENT_TOP_INSET)
                .max(style::ROW_VERTICAL_PADDING);
            let _: () = msg_send![type_field, setFrame: NSRect::new(NSPoint::new(type_x, type_y), NSSize::new(type_label_width, type_height))];
            let _: () = msg_send![type_field, setStringValue: NSString::alloc(nil).init_str(&type_label_str)];
            let _: () = msg_send![type_field, setWantsLayer: NO];
            let _: () = msg_send![type_field, setDrawsBackground: NO];

            let _: () = msg_send![container, setWantsLayer: YES];
            apply_selection_visuals(container, clipboard_mode, is_selected);
            container
        }
    }

    extern "C" fn row_view_for_row(_this: &Object, _cmd: Sel, _table: id, _row: isize) -> id {
        unsafe {
            let highlight_cls = class!(MKRowHighlightView);
            let row_view: id = msg_send![highlight_cls, alloc];
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
            let row_view: id = msg_send![row_view, initWithFrame: frame];
            let _: () = msg_send![row_view, setSelectionHighlightStyle:0];
            let _: () = msg_send![row_view, setEmphasized:NO];
            let clear_bg: id = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![row_view, setBackgroundColor: clear_bg];
            row_view
        }
    }

    unsafe fn apply_selection_visuals(container: id, clipboard_mode: bool, is_selected: bool) {
        if container == nil {
            return;
        }
        let container_layer: id = msg_send![container, layer];
        let _: () = msg_send![container_layer, setCornerRadius: style::ROW_CORNER_RADIUS];
        let _: () = msg_send![container_layer, setBorderWidth: style::ROW_BORDER_WIDTH];
        if is_selected {
            let accent_color: id = msg_send![class!(NSColor), controlAccentColor];
            let accent_bg: id = msg_send![
                accent_color,
                colorWithAlphaComponent: style::ROW_SELECTION_BG_ALPHA
            ];
            let accent_bg_cg: id = msg_send![accent_bg, CGColor];
            let _: () = msg_send![container_layer, setBackgroundColor: accent_bg_cg];
            let accent_border: id = msg_send![
                accent_color,
                colorWithAlphaComponent: style::ROW_SELECTION_BORDER_ALPHA
            ];
            let accent_border_cg: id = msg_send![accent_border, CGColor];
            let _: () = msg_send![container_layer, setBorderColor: accent_border_cg];
        } else if clipboard_mode {
            let card_bg: id =
                msg_send![class!(NSColor), colorWithCalibratedWhite:0.18f64 alpha:0.35f64];
            let card_bg_cg: id = msg_send![card_bg, CGColor];
            let _: () = msg_send![container_layer, setBackgroundColor: card_bg_cg];
            let border_clear: id =
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
            let border_clear_cg: id = msg_send![border_clear, CGColor];
            let _: () = msg_send![container_layer, setBorderColor: border_clear_cg];
        } else {
            let clear: id = msg_send![class!(NSColor), clearColor];
            let clear_cg: id = msg_send![clear, CGColor];
            let _: () = msg_send![container_layer, setBackgroundColor: clear_cg];
            let invisible: id =
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.05f64];
            let invisible_cg: id = msg_send![invisible, CGColor];
            let _: () = msg_send![container_layer, setBorderColor: invisible_cg];
        }
        let _: () = msg_send![container_layer, setMasksToBounds: NO];

        let subviews: id = msg_send![container, subviews];
        if subviews == nil {
            return;
        }
        let icon_view: id = msg_send![subviews, objectAtIndex:0];
        if icon_view != nil {
            let _: () = msg_send![icon_view, setWantsLayer: YES];
            let icon_layer: id = msg_send![icon_view, layer];
            if clipboard_mode || is_selected {
                let icon_bg: id =
                    msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
                let icon_bg_cg: id = msg_send![icon_bg, CGColor];
                let _: () = msg_send![icon_layer, setCornerRadius: 10.0f64];
                let _: () = msg_send![icon_layer, setMasksToBounds: YES];
                let _: () = msg_send![icon_layer, setBackgroundColor: icon_bg_cg];
            } else {
                let clear: id = msg_send![class!(NSColor), clearColor];
                let clear_cg: id = msg_send![clear, CGColor];
                let _: () = msg_send![icon_layer, setCornerRadius: 8.0f64];
                let _: () = msg_send![icon_layer, setMasksToBounds: YES];
                let _: () = msg_send![icon_layer, setBackgroundColor: clear_cg];
            }
            let icon_tint: id = if is_selected {
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.95f64]
            } else if clipboard_mode {
                msg_send![class!(NSColor), colorWithCalibratedRed:0.9f64 green:0.95f64 blue:1.0f64 alpha:0.85f64]
            } else {
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.85f64]
            };
            let _: () = msg_send![icon_view, setContentTintColor: icon_tint];
        }

        let title_field: id = msg_send![subviews, objectAtIndex:1];
        if title_field != nil {
            let primary_color: id =
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.96f64];
            let _: () = msg_send![title_field, setTextColor: primary_color];
        }
        let subtitle_field: id = msg_send![subviews, objectAtIndex:2];
        if subtitle_field != nil {
            let secondary_color: id =
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.66f64];
            let _: () = msg_send![subtitle_field, setTextColor: secondary_color];
        }
        let type_field: id = msg_send![subviews, objectAtIndex:3];
        if type_field != nil {
            let pill_text: id = if is_selected {
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.9f64]
            } else {
                msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.6f64]
            };
            let _: () = msg_send![type_field, setTextColor: pill_text];
        }
    }

    unsafe fn refresh_selection_visuals(
        table: id,
        clipboard_mode: bool,
        selected_row: Option<usize>,
    ) {
        let num_rows: isize = msg_send![table, numberOfRows];
        for row in 0..num_rows {
            let row_view: id = msg_send![table, viewAtColumn:0 row:row makeIfNecessary:NO];
            if row_view == nil {
                continue;
            }
            let is_selected = selected_row.map(|sel| sel as isize == row).unwrap_or(false);
            apply_selection_visuals(row_view, clipboard_mode, is_selected);
        }
    }

    extern "C" fn selection_changed(_this: &Object, _cmd: Sel, note: id) {
        unsafe {
            let table: id = msg_send![note, object];
            if table == nil {
                update_clipboard_preview_selection(None);
                return;
            }
            let selected_row: isize = msg_send![table, selectedRow];
            let row_option = if selected_row >= 0 {
                Some(selected_row as usize)
            } else {
                None
            };
            let mode = match TABLE_MODE.lock() {
                Ok(m) => *m,
                Err(_) => TableMode::Search,
            };
            let clipboard_mode = mode == TableMode::ClipboardHistory;
            if mode == TableMode::ClipboardHistory {
                update_clipboard_preview_selection(row_option);
            } else {
                update_clipboard_preview_selection(None);
            }
            refresh_selection_visuals(table, clipboard_mode, row_option);
        }
    }

    decl.add_method(
        sel!(numberOfRowsInTableView:),
        rows as extern "C" fn(&Object, Sel, id) -> isize,
    );
    decl.add_method(
        sel!(tableView:viewForTableColumn:row:),
        view_for_row as extern "C" fn(&Object, Sel, id, id, isize) -> id,
    );
    decl.add_method(
        sel!(tableView:rowViewForRow:),
        row_view_for_row as extern "C" fn(&Object, Sel, id, isize) -> id,
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
    unsafe fn hide_window_immediately() {
        if let Ok(mut showing) = WINDOW_SHOWING.lock() {
            *showing = false;
        }
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        if count > 0 {
            let window: id = msg_send![windows, objectAtIndex:0];
            crate::ui::settings_view::hide_settings_panel();
            let _: () = msg_send![window, orderOut: nil];
        }
        // Relinquish focus so the previous app regains key status for paste.
        let _: () = msg_send![app, deactivate];
    }

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
        search_engine::SearchResult::Clipboard {
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            hide_window_immediately();
            let content_clone = content.clone();
            let content_type_clone = content_type.clone();
            SEARCH_RT.spawn(async move {
                let _ = crate::clipboard::paste_history_entry(
                    &content_clone,
                    &content_type_clone,
                    image_width,
                    image_height,
                )
                .await;
            });
        }
        search_engine::SearchResult::Command { command, .. } => {
            SEARCH_RT.spawn(async move {
                let _ = system_commands::execute(&command).await;
            });
        }
        search_engine::SearchResult::Calculator { result, .. } => {
            hide_window_immediately();
            let to_paste = result.clone();
            SEARCH_RT.spawn(async move {
                let _ = crate::clipboard::paste_to_active_app(&to_paste).await;
            });
        }
        search_engine::SearchResult::Emoji { emoji, .. } => {
            hide_window_immediately();
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

    // Non-paste actions hide after triggering.
    hide_window_immediately();
}

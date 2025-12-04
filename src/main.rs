#![allow(unexpected_cfgs)] // `objc` macros gate on `cargo-clippy`; accept it to satisfy check-cfg

use cacao::appkit::{App, AppDelegate};
use cocoa::appkit::NSWindowStyleMask;
use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

mod app_launcher;
mod calculator;
mod clipboard;
mod database;
mod dictionary;
mod emoji;
mod file_search;
mod search_engine;
mod settings;
mod system_commands;
mod ui;
mod usage;
mod web_search;

use log::{error, info};
use std::sync::atomic::Ordering;
use ui::clipboard_view::{
    apply_clipboard_history_state, build_clipboard_history_payload, create_clipboard_preview_view,
    show_clipboard_history_view, update_clipboard_preview_selection,
};
use ui::helpers::{run_on_main, style};
use ui::settings_view;
use ui::state::*;
use ui::table;
use viceroy::updater;
use viceroy::updater::{UPDATE_CHECK_DISABLED_ENV, UPDATE_METADATA_URL_ENV, UPDATE_SILENT_ENV};

struct ViceroyApp;

impl Default for ViceroyApp {
    fn default() -> Self {
        ViceroyApp
    }
}

impl AppDelegate for ViceroyApp {
    fn did_finish_launching(&self) {
        // Load settings early so UI reflects preferences
        if let Ok(s) = settings::load() {
            if let Ok(mut d) = DISMISS_ON_ESCAPE.lock() {
                *d = s.dismiss_on_escape;
            }
            if let Ok(mut d2) = DISMISS_ON_CLICK_AWAY.lock() {
                *d2 = s.dismiss_on_click_away;
            }
        }
        unsafe {
            // Create a custom window class that can become key even when borderless
            register_key_window_class();

            let ns_window: id = msg_send![class!(MKKeyWindow), alloc];
            let rect = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(960.0, 132.0)); // Better proportions

            let style_mask = NSWindowStyleMask::NSBorderlessWindowMask
                | NSWindowStyleMask::NSFullSizeContentViewWindowMask;

            let ns_window: id = msg_send![ns_window,
                initWithContentRect:rect
                styleMask:style_mask
                backing:2 // NSBackingStoreBuffered
                defer:NO
            ];

            // Make borderless window accept key status and mouse events
            let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: YES];
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: NO];

            // Borderless with full size content
            let style_mask = NSWindowStyleMask::NSBorderlessWindowMask
                | NSWindowStyleMask::NSFullSizeContentViewWindowMask;
            let _: () = msg_send![ns_window, setStyleMask: style_mask];
            let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];
            let _: () = msg_send![ns_window, setMovable: NO]; // Cannot be moved
            let _: () = msg_send![ns_window, setMovableByWindowBackground: NO];
            let _: () = msg_send![ns_window, setLevel: 1]; // Floating

            // Rounded corners
            let _: () = msg_send![ns_window, setOpaque: NO];
            let clear_color: id = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![ns_window, setBackgroundColor: clear_color];
            let _: () = msg_send![ns_window, setHasShadow: YES];

            let content_view: id = msg_send![ns_window, contentView];
            let bounds: NSRect = msg_send![content_view, bounds];

            // Force dark appearance for modern look
            let dark_appearance: id = msg_send![class!(NSAppearance), appearanceNamed: NSString::alloc(nil).init_str("NSAppearanceNameVibrantDark")];
            let _: () = msg_send![ns_window, setAppearance: dark_appearance];

            // iOS-style translucent blur background
            let effect_view: id = msg_send![class!(NSVisualEffectView), alloc];
            let effect_view: id = msg_send![effect_view, initWithFrame: bounds];
            let _: () = msg_send![effect_view, setMaterial: 7]; // Fullscreen UI - modern translucent
            let _: () = msg_send![effect_view, setBlendingMode: 0]; // BehindWindow
            let _: () = msg_send![effect_view, setState: 1]; // Active
            let _: () = msg_send![effect_view, setAutoresizingMask: 18]; // Width+Height
            let _: () = msg_send![effect_view, setWantsLayer: YES];

            // Set corner radius directly - simpler approach
            let _: () = msg_send![content_view, setWantsLayer: YES];
            let content_layer: id = msg_send![content_view, layer];
            let _: () = msg_send![content_layer, setCornerRadius: 24.0f64];
            let _: () = msg_send![content_layer, setMasksToBounds: YES];

            // Also set on effect view
            let effect_layer: id = msg_send![effect_view, layer];
            let _: () = msg_send![effect_layer, setCornerRadius: 24.0f64];

            // Add subtle shadow for floating effect
            let _: () = msg_send![ns_window, setHasShadow: YES];

            let _: () = msg_send![content_view, addSubview: effect_view];

            // Search field with shimmer
            create_search_field(content_view, bounds);

            // Results table (Viceroy style placeholder)
            create_results_table(content_view, bounds);

            // Center on screen with snap animation
            center_window_with_snap(ns_window);

            // Prevent window from closing (only allow hiding)
            let _: () = msg_send![ns_window, setReleasedWhenClosed: NO];

            // Don't show window initially - wait for hotkey
            // let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
            let _: () = msg_send![ns_window, setCollectionBehavior: 1]; // CanJoinAllSpaces

            // Setup window delegate for click-away dismissal
            setup_window_delegate(ns_window);
            setup_app_observer(ns_window);

            // Create menu bar icon
            create_status_bar_item();

            // Retain the window so it doesn't get deallocated
            let _: id = msg_send![ns_window, retain];
        }

        // Register global hotkey (Command+Shift+Space)
        std::thread::spawn(move || {
            use std::time::{Duration, Instant};

            match GlobalHotKeyManager::new() {
                Ok(manager) => {
                    let hotkey =
                        HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);

                    match manager.register(hotkey) {
                        Ok(_) => {
                            let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
                            // Debounce to avoid rapid repeat events while key held
                            let mut last_toggle = Instant::now() - Duration::from_secs(1);
                            let mut last_escape_check = Instant::now();

                            #[link(name = "Carbon", kind = "framework")]
                            extern "C" {
                                fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
                            }

                            loop {
                                // Check for Escape key every 50ms
                                if last_escape_check.elapsed() >= Duration::from_millis(50) {
                                    last_escape_check = Instant::now();

                                    let showing = match WINDOW_SHOWING.lock() {
                                        Ok(g) => *g,
                                        Err(_) => false,
                                    };
                                    if showing {
                                        if let Ok(dismiss_flag) = DISMISS_ON_ESCAPE.lock() {
                                            if *dismiss_flag
                                                && unsafe { CGEventSourceKeyState(1, 53) }
                                            {
                                                // 53 = Escape
                                                // Use exec_sync to hide immediately
                                                dispatch::Queue::main().exec_sync(|| unsafe {
                                                    if let Ok(mut w) = WINDOW_SHOWING.lock() {
                                                        *w = false;
                                                    }
                                                    let app: id = msg_send![
                                                        class!(NSApplication),
                                                        sharedApplication
                                                    ];
                                                    let windows: id = msg_send![app, windows];
                                                    let count: usize = msg_send![windows, count];
                                                    if count > 0 {
                                                        let window: id =
                                                            msg_send![windows, objectAtIndex:0];
                                                        let _: () =
                                                            msg_send![window, orderOut: nil];
                                                    }
                                                });
                                                std::thread::sleep(Duration::from_millis(300));
                                            }
                                        }
                                    }
                                }

                                // Check for hotkey events with timeout so Escape checking continues
                                if let Ok(_event) = receiver.recv_timeout(Duration::from_millis(50))
                                {
                                    // Ignore if within debounce interval (key repeat spam)
                                    if last_toggle.elapsed() < Duration::from_millis(250) {
                                        continue;
                                    }
                                    last_toggle = Instant::now();
                                    let should_show = match WINDOW_SHOWING.lock() {
                                        Ok(mut guard) => {
                                            let new = !*guard;
                                            *guard = new;
                                            new
                                        }
                                        Err(_) => {
                                            // Failed to lock - default to showing
                                            true
                                        }
                                    };

                                    // Dispatch to main queue
                                    unsafe {
                                        if should_show {
                                            dispatch::Queue::main().exec_async(move || {
                                                let app: id = msg_send![class!(NSApplication), sharedApplication];
                                                let windows: id = msg_send![app, windows];
                                                let count: usize = msg_send![windows, count];

                                                if count > 0 {
                                                    let window: id = msg_send![windows, objectAtIndex: 0];
                                                    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                                                    let _: () = msg_send![window, makeKeyAndOrderFront: nil];

                                                    // Focus and reset search field on each hotkey show
                                                    if let Some(search_field) = find_search_field() {
                                                        // Clear previous query text and any existing results
                                                        let empty: id = NSString::alloc(nil).init_str("");
                                                        let _: () = msg_send![search_field, setStringValue: empty];

                                                        if let Ok(mut mode) = TABLE_MODE.lock() {
                                                            *mode = TableMode::Search;
                                                        }
                                                        update_clipboard_preview_selection(None);
                                                        table::update_preview_layout(false);

                                                        if let Ok(mut tr) = TABLE_RESULTS.lock() {
                                                            tr.clear();
                                                        }
                                                        if let Ok(mut td) = TABLE_DATA.lock() {
                                                            td.clear();
                                                        }
                                                        table::schedule_table_update_next_tick();

                                                        // Ensure white insertion point before typing
                                                        let field_editor: id = msg_send![window, fieldEditor:YES forObject:search_field];
                                                        if field_editor != nil {
                                                            let white: id = msg_send![class!(NSColor), whiteColor];
                                                            let _: () = msg_send![field_editor, setInsertionPointColor: white];
                                                        }

                                                        let _: () = msg_send![window, makeFirstResponder: search_field];
                                                    }
                                                }
                                            });
                                        } else {
                                            dispatch::Queue::main().exec_async(move || {
                                                let app: id = msg_send![
                                                    class!(NSApplication),
                                                    sharedApplication
                                                ];
                                                let windows: id = msg_send![app, windows];
                                                let count: usize = msg_send![windows, count];

                                                if count > 0 {
                                                    let window: id =
                                                        msg_send![windows, objectAtIndex: 0];
                                                    let _: () = msg_send![window, orderOut: nil];
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("✗ Hotkey failed: {} (need Accessibility permission?)", e)
                        }
                    }
                }
                Err(e) => eprintln!("✗ Hotkey manager failed: {}", e),
            }
        });

        // Initialize database (lightweight)
        if let Err(e) = database::init() {
            eprintln!("Database init error: {}", e);
        }

        // Monitor clipboard in a background thread so history stays populated
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = crate::clipboard::start_monitor().await {
                    eprintln!("Clipboard monitor error: {}", e);
                }
            });
        });

        // Pre-warm app cache in background
        std::thread::spawn(|| {
            let _ = app_launcher::get_all_apps();
        });
    }

    fn should_terminate_after_last_window_closed(&self) -> bool {
        false // Don't quit when window closes, we're a background app
    }
}

unsafe fn register_key_window_class() {
    if objc::runtime::Class::get("MKKeyWindow").is_some() {
        return;
    }
    let superclass = class!(NSWindow);
    let mut decl = ClassDecl::new("MKKeyWindow", superclass).unwrap();

    extern "C" fn can_become_key(_this: &Object, _cmd: Sel) -> BOOL {
        YES
    }

    extern "C" fn can_become_main(_this: &Object, _cmd: Sel) -> BOOL {
        YES
    }

    decl.add_method(
        sel!(canBecomeKeyWindow),
        can_become_key as extern "C" fn(&Object, Sel) -> BOOL,
    );
    decl.add_method(
        sel!(canBecomeMainWindow),
        can_become_main as extern "C" fn(&Object, Sel) -> BOOL,
    );
    decl.register();
}

unsafe fn register_custom_textfield_cell() {
    if objc::runtime::Class::get("MKTextFieldCell").is_some() {
        return;
    }
    let superclass = class!(NSTextFieldCell);
    let mut decl = ClassDecl::new("MKTextFieldCell", superclass).unwrap();

    extern "C" fn draw_interior(this: &Object, _cmd: Sel, frame: NSRect, view: id) {
        // With isFlipped = YES (0 is top)
        // Height 60. Center is 30.
        // Font height ~22-28.
        // Rect height 28.
        // Top y = 30 - 14 = 16.
        let inset_frame = NSRect::new(
            NSPoint::new(frame.origin.x + 54.0, frame.origin.y + 16.0),
            NSSize::new(frame.size.width - 70.0, 28.0),
        );
        unsafe {
            let superclass = class!(NSTextFieldCell);
            let _: () =
                msg_send![super(this, superclass), drawInteriorWithFrame:inset_frame inView:view];
        }
    }

    extern "C" fn editing_rect(_this: &Object, _cmd: Sel, frame: NSRect) -> NSRect {
        NSRect::new(
            NSPoint::new(frame.origin.x + 54.0, frame.origin.y + 16.0),
            NSSize::new(frame.size.width - 70.0, 28.0),
        )
    }

    extern "C" fn drawing_rect(_this: &Object, _cmd: Sel, frame: NSRect) -> NSRect {
        NSRect::new(
            NSPoint::new(frame.origin.x + 54.0, frame.origin.y + 16.0),
            NSSize::new(frame.size.width - 70.0, 28.0),
        )
    }

    extern "C" fn select_rect(_this: &Object, _cmd: Sel, frame: NSRect) -> NSRect {
        NSRect::new(
            NSPoint::new(frame.origin.x + 54.0, frame.origin.y + 16.0),
            NSSize::new(frame.size.width - 70.0, 28.0),
        )
    }

    unsafe {
        decl.add_method(
            sel!(drawInteriorWithFrame:inView:),
            draw_interior as extern "C" fn(&Object, Sel, NSRect, id),
        );
        decl.add_method(
            sel!(editingRectForBounds:),
            editing_rect as extern "C" fn(&Object, Sel, NSRect) -> NSRect,
        );
        decl.add_method(
            sel!(drawingRectForBounds:),
            drawing_rect as extern "C" fn(&Object, Sel, NSRect) -> NSRect,
        );
        decl.add_method(
            sel!(selectRectForBounds:),
            select_rect as extern "C" fn(&Object, Sel, NSRect) -> NSRect,
        );
        decl.register();
    }
}

unsafe fn register_escape_textfield_class() {
    if objc::runtime::Class::get("MKEscapeTextField").is_some() {
        return;
    }
    let superclass = class!(NSTextField);
    let mut decl = ClassDecl::new("MKEscapeTextField", superclass).unwrap();

    extern "C" fn is_flipped(_this: &Object, _cmd: Sel) -> BOOL {
        YES
    }

    extern "C" fn cancel_operation(_this: &Object, _cmd: Sel, _sender: id) {
        // cancelOperation: is called when Escape is pressed
        unsafe {
            if let Ok(dismiss) = DISMISS_ON_ESCAPE.lock() {
                if *dismiss {
                    // Update global state
                    if let Ok(mut w) = WINDOW_SHOWING.lock() {
                        *w = false;
                    }
                    // Hide the window
                    let app: id = msg_send![class!(NSApplication), sharedApplication];
                    let windows: id = msg_send![app, windows];
                    let count: usize = msg_send![windows, count];
                    if count > 0 {
                        let window: id = msg_send![windows, objectAtIndex:0];
                        let _: () = msg_send![window, orderOut: nil];
                    }
                }
            }
        }
    }

    extern "C" fn insert_newline(_this: &Object, _cmd: Sel, _sender: id) {
        unsafe {
            table::activate_selected_row_or_first();
        }
    }

    extern "C" fn text_view_do_command(
        _this: &Object,
        _cmd: Sel,
        _text_view: id,
        command_selector: Sel,
    ) -> BOOL {
        unsafe {
            let selector_name =
                std::ffi::CStr::from_ptr(objc::runtime::sel_getName(command_selector))
                    .to_str()
                    .unwrap_or("");

            eprintln!("Command: {}", selector_name); // Debug

            match selector_name {
                "moveDown:" => {
                    table::move_table_selection(true);
                    YES // Handled
                }
                "moveUp:" => {
                    table::move_table_selection(false);
                    YES // Handled
                }
                "moveRight:" => {
                    // Let NSTextField handle it normally
                    NO
                }
                "insertTab:" => {
                    if let Some(query) = get_current_search_query() {
                        if query.is_empty() {
                            show_clipboard_history_view();
                        } else {
                            table::move_table_selection(true);
                        }
                    } else {
                        show_clipboard_history_view();
                    }
                    YES // Handled
                }
                "insertNewline:" => {
                    table::activate_selected_row_or_first();
                    YES // Handled
                }
                _ => NO, // Not handled, let NSTextField process it
            }
        }
    }

    // Override drawFocusRingMask to prevent any focus ring drawing
    extern "C" fn draw_focus_ring_mask(_this: &Object, _cmd: Sel) {
        // Do nothing - completely skip focus ring drawing
    }

    // Return empty rect for focus ring mask
    extern "C" fn focus_ring_mask_bounds(_this: &Object, _cmd: Sel) -> NSRect {
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
    }

    extern "C" fn perform_key_equivalent(this: &Object, _cmd: Sel, event: id) -> BOOL {
        unsafe {
            let flags: u64 = msg_send![event, modifierFlags];
            let chars: id = msg_send![event, charactersIgnoringModifiers];
            let s: *const i8 = msg_send![chars, UTF8String];
            let s = std::ffi::CStr::from_ptr(s).to_str().unwrap_or("");

            if (flags & (1 << 20)) != 0 {
                // Command key
                if s == "a" {
                    let _: () = msg_send![this, selectText:nil];
                    return YES;
                }
                if s == "c" {
                    let _: () =
                        msg_send![class!(NSApplication), sendAction:sel!(copy:) to:nil from:this];
                    return YES;
                }
                if s == "v" {
                    let _: () =
                        msg_send![class!(NSApplication), sendAction:sel!(paste:) to:nil from:this];
                    return YES;
                }
                if s == "x" {
                    let _: () =
                        msg_send![class!(NSApplication), sendAction:sel!(cut:) to:nil from:this];
                    return YES;
                }
                if s == "," {
                    settings_view::show_settings_panel();
                    return YES;
                }
            }

            // Call super
            let superclass = class!(NSTextField);
            msg_send![super(this, superclass), performKeyEquivalent:event]
        }
    }

    unsafe {
        decl.add_method(
            sel!(performKeyEquivalent:),
            perform_key_equivalent as extern "C" fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_method(
            sel!(isFlipped),
            is_flipped as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(cancelOperation:),
            cancel_operation as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(insertNewline:),
            insert_newline as extern "C" fn(&Object, Sel, id),
        );
        // NSTextFieldDelegate method - called when field editor processes commands
        decl.add_method(
            sel!(textView:doCommandBySelector:),
            text_view_do_command as extern "C" fn(&Object, Sel, id, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(drawFocusRingMask),
            draw_focus_ring_mask as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(focusRingMaskBounds),
            focus_ring_mask_bounds as extern "C" fn(&Object, Sel) -> NSRect,
        );
        decl.register();
    }
}

unsafe fn create_search_field(content_view: id, bounds: NSRect) {
    // Register custom classes
    register_custom_textfield_cell();
    register_escape_textfield_class();

    // 1. Create Container View (The "Search Bar" visual)
    let container: id = msg_send![class!(NSView), alloc];
    let container_frame = NSRect::new(
        NSPoint::new(
            style::CONTENT_SIDE_INSET,
            bounds.size.height - style::TABLE_TOP_OFFSET,
        ),
        NSSize::new(
            bounds.size.width - style::CONTENT_SIDE_INSET * 2.0,
            style::SEARCH_BAR_HEIGHT,
        ),
    );
    let container: id = msg_send![container, initWithFrame: container_frame];
    let _: () = msg_send![container, setWantsLayer: YES];
    let layer: id = msg_send![container, layer];
    let _: () = msg_send![layer, setCornerRadius: 20.0f64];
    let _: () = msg_send![layer, setMasksToBounds: YES];

    // Add subtle border
    let _: () = msg_send![layer, setBorderWidth: 0.8f64];
    let border_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let border_cg: id = msg_send![border_color, CGColor];
    let _: () = msg_send![layer, setBorderColor: border_cg];
    let bg_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.05f64 alpha:0.9f64];
    let bg_color_cg: id = msg_send![bg_color, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_color_cg];

    // Add Visual Effect View for Glass Look (Dark HUD style)
    let effect_view: id = msg_send![class!(NSVisualEffectView), alloc];
    let effect_view: id = msg_send![effect_view, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), container_frame.size)];
    let _: () = msg_send![effect_view, setMaterial: 12]; // Sidebar
    let _: () = msg_send![effect_view, setBlendingMode: 0]; // BehindWindow
    let _: () = msg_send![effect_view, setState: 1]; // Active
    let _: () = msg_send![effect_view, setAutoresizingMask: 18]; // Width+Height
    let _: () = msg_send![container, addSubview: effect_view];

    // 2. Add Icon badge
    let icon_badge: id = msg_send![class!(NSView), alloc];
    let badge_size = 38.0;
    let badge_origin_y = (style::SEARCH_BAR_HEIGHT - badge_size) / 2.0;
    let icon_badge: id = msg_send![icon_badge, initWithFrame: NSRect::new(NSPoint::new(18.0, badge_origin_y), NSSize::new(badge_size, badge_size))];
    let _: () = msg_send![icon_badge, setWantsLayer: YES];
    let badge_layer: id = msg_send![icon_badge, layer];
    let badge_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let badge_bg_cg: id = msg_send![badge_bg, CGColor];
    let _: () = msg_send![badge_layer, setCornerRadius: 13.0f64];
    let _: () = msg_send![badge_layer, setBackgroundColor: badge_bg_cg];
    let _: () = msg_send![badge_layer, setBorderWidth: 0.0f64];
    let icon_view: id = msg_send![class!(NSImageView), alloc];
    let icon_view: id = msg_send![icon_view, initWithFrame: NSRect::new(NSPoint::new(6.0, 6.0), NSSize::new(26.0, 26.0))];
    let icon_name = NSString::alloc(nil).init_str("magnifyingglass");
    let image: id = msg_send![class!(NSImage), imageWithSystemSymbolName:icon_name accessibilityDescription:nil];
    let _: () = msg_send![icon_view, setImage: image];
    let icon_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.5f64]; // Subtle gray
    let _: () = msg_send![icon_view, setContentTintColor: icon_color];
    let _: () = msg_send![icon_badge, addSubview: icon_view];
    let _: () = msg_send![container, addSubview: icon_badge];

    // 3. Create Search Input (Transparent, Centered)
    let search_field: id = msg_send![class!(MKEscapeTextField), alloc];
    // Height 32px centered vertically.
    let input_frame = NSRect::new(
        NSPoint::new(18.0 + badge_size + 16.0, (style::SEARCH_BAR_HEIGHT - 32.0) / 2.0),
        NSSize::new(container_frame.size.width - (18.0 + badge_size + 16.0) - 190.0, 32.0),
    );
    let search_field: id = msg_send![search_field, initWithFrame: input_frame];

    // Configure Input Style (Transparent)
    let _: () = msg_send![search_field, setBezeled: NO];
    let _: () = msg_send![search_field, setBordered: NO];
    let _: () = msg_send![search_field, setDrawsBackground: NO];
    let _: () = msg_send![search_field, setFocusRingType: 0]; // None
    let _: () = msg_send![search_field, setEditable: YES];
    let _: () = msg_send![search_field, setSelectable: YES];

    // Font and Color
    let font = create_search_font();
    let _: () = msg_send![search_field, setFont: font];
    let text_color: id = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![search_field, setTextColor: text_color];

    // Placeholder
    let placeholder_text =
        NSString::alloc(nil).init_str("Search apps, files, clipboard history, and more");
    let placeholder_attrs: id = msg_send![class!(NSMutableDictionary), dictionary];
    let placeholder_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.3f64]; // Very subtle
    let _: () = msg_send![placeholder_attrs, setObject:placeholder_color forKey:NSString::alloc(nil).init_str("NSColor")];
    let _: () =
        msg_send![placeholder_attrs, setObject:font forKey:NSString::alloc(nil).init_str("NSFont")];
    let attributed_placeholder: id = msg_send![class!(NSAttributedString), alloc];
    let attributed_placeholder: id = msg_send![attributed_placeholder, initWithString:placeholder_text attributes:placeholder_attrs];

    let cell: id = msg_send![search_field, cell];
    let _: () = msg_send![cell, setPlaceholderAttributedString: attributed_placeholder];
    let _: () = msg_send![cell, setScrollable: YES];
    let _: () = msg_send![cell, setUsesSingleLineMode: YES];

    // Right-side hint label
    let hint_label: id = msg_send![class!(NSTextField), alloc];
    let hint_label: id = msg_send![hint_label, initWithFrame: NSRect::new(NSPoint::new(container_frame.size.width - 190.0, (style::SEARCH_BAR_HEIGHT - 32.0) / 2.0), NSSize::new(170.0, 32.0))];
    let _: () = msg_send![hint_label, setBezeled: NO];
    let _: () = msg_send![hint_label, setEditable: NO];
    let _: () = msg_send![hint_label, setDrawsBackground: NO];
    let _: () = msg_send![hint_label, setBordered: NO];
    let _: () = msg_send![hint_label, setAlignment: 2];
    let hint_font: id = msg_send![class!(NSFont), systemFontOfSize:12.0 weight:0.4];
    let _: () = msg_send![hint_label, setFont: hint_font];
    let _: () = msg_send![hint_label, setUsesSingleLineMode: YES];
    let _: () = msg_send![hint_label, setLineBreakMode: 4];
    let hint_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.55f64];
    let _: () = msg_send![hint_label, setTextColor: hint_color];
    let hint_text = NSString::alloc(nil).init_str("⌘ + , Preferences");
    let _: () = msg_send![hint_label, setStringValue: hint_text];

    // Secondary hint chips stacked below input for quick shortcuts
    let chip_container: id = msg_send![class!(NSView), alloc];
    let chip_container: id = msg_send![chip_container, initWithFrame: NSRect::new(NSPoint::new(18.0 + badge_size + 16.0, 6.0), NSSize::new(container_frame.size.width - (18.0 + badge_size + 16.0) - 200.0, 18.0))];
    let _: () = msg_send![chip_container, setWantsLayer: NO];
    let chip_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:11.0 weight:0.2];
    let chip_text: id = msg_send![class!(NSTextField), alloc];
    let chip_text: id = msg_send![chip_text, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(container_frame.size.width - (18.0 + badge_size + 16.0) - 220.0, 18.0))];
    let _: () = msg_send![chip_text, setBezeled: NO];
    let _: () = msg_send![chip_text, setEditable: NO];
    let _: () = msg_send![chip_text, setDrawsBackground: NO];
    let _: () = msg_send![chip_text, setBordered: NO];
    let _: () = msg_send![chip_text, setFont: chip_font];
    let _: () = msg_send![chip_text, setUsesSingleLineMode: YES];
    let _: () = msg_send![chip_text, setLineBreakMode: 4];
    let chip_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.45f64];
    let _: () = msg_send![chip_text, setTextColor: chip_color];
    let chip_value =
        NSString::alloc(nil).init_str("Tab = clipboard · / = web search · Esc = hide");
    let _: () = msg_send![chip_text, setStringValue: chip_value];
    let _: () = msg_send![chip_container, addSubview: chip_text];

    let _: () = msg_send![container, addSubview: search_field];
    let _ = SEARCH_FIELD.set(search_field as usize);
    let _: () = msg_send![container, addSubview: hint_label];
    let _: () = msg_send![container, addSubview: chip_container];
    let _: () = msg_send![content_view, addSubview: container];

    // Focus immediately
    let window: id = msg_send![content_view, window];
    let _: () = msg_send![window, makeFirstResponder: search_field];

    // Add delegate for live search updates
    register_search_delegate_class();
    let delegate_class = class!(MKSearchDelegate);
    let delegate_instance: id = msg_send![delegate_class, new];
    let _: () = msg_send![search_field, setDelegate: delegate_instance];
}

unsafe fn create_results_table(content_view: id, bounds: NSRect) {
    table::register_table_delegate_class();

    // Layout constants for list + preview split
    let table_height =
        (bounds.size.height - style::RESULTS_TOP_OFFSET - style::TABLE_FOOTER_HEIGHT).max(0.0);
    let available_width = (bounds.size.width
        - style::CONTENT_SIDE_INSET * 2.0
        - style::LIST_EXTRA_MARGIN * 2.0)
        .max(style::LIST_MIN_WIDTH);
    let split_list_width = (available_width * style::LIST_WIDTH_RATIO).max(style::LIST_MIN_WIDTH);
    let preview_width =
        (available_width - split_list_width - style::PREVIEW_GAP).max(style::PREVIEW_MIN_WIDTH);
    let list_origin_x = style::CONTENT_SIDE_INSET + style::LIST_EXTRA_MARGIN;
    let preview_origin_x = list_origin_x + split_list_width + style::PREVIEW_GAP;

    // Scroll view container with padding on the left
    let scroll: id = msg_send![class!(NSScrollView), alloc];
    let frame = NSRect::new(
        NSPoint::new(list_origin_x, style::TABLE_FOOTER_HEIGHT),
        NSSize::new(split_list_width, table_height),
    );
    let scroll: id = msg_send![scroll, initWithFrame: frame];
    table::install_constrained_clip_view(scroll, NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(frame.size.width, table_height)));
    let _: () = msg_send![scroll, setBorderType: 0];
    let _: () = msg_send![scroll, setDrawsBackground: NO];
    let _: () = msg_send![scroll, setWantsLayer: YES];
    let scroll_layer: id = msg_send![scroll, layer];
    let _: () = msg_send![scroll_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![scroll_layer, setMasksToBounds: YES];
    let scroll_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.05f64 alpha:0.38f64];
    let scroll_bg_cg: id = msg_send![scroll_bg, CGColor];
    let _: () = msg_send![scroll_layer, setBackgroundColor: scroll_bg_cg];
    let _: () = msg_send![scroll, setHasVerticalScroller: YES];
    let _: () = msg_send![scroll, setHasHorizontalScroller: NO];
    let _: () = msg_send![scroll, setAutohidesScrollers: YES];
    let _: () = msg_send![scroll, setScrollerStyle:1];
    let _: () = msg_send![scroll, setHorizontalScrollElasticity:0];
    let _: () = msg_send![scroll, setAutoresizingMask: 16]; // height only
    let _ = TABLE_SCROLL_VIEW.set(scroll as usize);

    // Table view with modern spacing
    let table: id = msg_send![class!(NSTableView), alloc];
    let table: id = msg_send![table, initWithFrame: NSRect::new(NSPoint::new(0.0,0.0), NSSize::new(frame.size.width, table_height))];
    let _: () = msg_send![table, setHeaderView: nil];
    let _: () = msg_send![table, setRowHeight: table::ROW_HEIGHT];
    let _: () = msg_send![table, setIntercellSpacing: NSSize::new(0.0, style::ROW_STACK_SPACING)];
    let _: () = msg_send![table, setColumnAutoresizingStyle: 1u64]; // uniform auto-resize
    let _: () = msg_send![table, setSelectionHighlightStyle: -1]; // Custom selection drawing
    let _: () = msg_send![table, setFocusRingType: 0];
    let bg_color: id = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![table, setBackgroundColor: bg_color];
    let _: () = msg_send![table, setGridStyleMask: 0]; // No grid
    let _: () = msg_send![table, setBackgroundColor: bg_color];
    let _: () = msg_send![table, setAllowsExpansionToolTips: YES];

    // Enable alternating row colors set to clear for consistent look
    let _: () = msg_send![table, setUsesAlternatingRowBackgroundColors: NO];

    // Single column
    let column: id = msg_send![class!(NSTableColumn), alloc];
    let column: id = msg_send![column, initWithIdentifier: NSString::alloc(nil).init_str("main")];
    let _: () = msg_send![column, setWidth: frame.size.width];
    let _: () = msg_send![column, setResizingMask: 1u64];
    let _: () = msg_send![table, addTableColumn: column];

    // Data source & delegate
    let delegate_class = class!(MKTableDelegate);
    let delegate_instance: id = msg_send![delegate_class, new];
    let _: () = msg_send![table, setDelegate: delegate_instance];
    let _: () = msg_send![table, setDataSource: delegate_instance];

    // Row activation target for double-click
    if objc::runtime::Class::get("MKRowActions").is_none() {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MKRowActions", superclass).unwrap();
        extern "C" fn row_activate(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                table::activate_selected_row_or_first();
            }
        }
        unsafe {
            decl.add_method(
                sel!(rowActivate:),
                row_activate as extern "C" fn(&Object, Sel, id),
            );
            decl.register();
        }
    }
    let row_actions_class = class!(MKRowActions);
    let row_actions: id = unsafe { msg_send![row_actions_class, new] };
    let _: id = unsafe { msg_send![row_actions, retain] };
    let _: () = unsafe { msg_send![table, setTarget: row_actions] };
    let _: () = unsafe { msg_send![table, setDoubleAction: sel!(rowActivate:)] };

    // Embed table in scroll
    let _: () = msg_send![scroll, setDocumentView: table];
    let _: () = msg_send![content_view, addSubview: scroll];

    // Clipboard preview panel to the right of the list
    let preview_frame = NSRect::new(
        NSPoint::new(preview_origin_x, style::TABLE_FOOTER_HEIGHT),
        NSSize::new(preview_width, table_height),
    );
    create_clipboard_preview_view(content_view, preview_frame);
    table::update_preview_layout(false);

    // Footer hint bar
    let footer_height = style::TABLE_FOOTER_HEIGHT;
    let footer_frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(bounds.size.width, footer_height),
    );
    let footer: id = msg_send![class!(NSView), alloc];
    let footer: id = msg_send![footer, initWithFrame: footer_frame];
    let _: () = msg_send![footer, setWantsLayer: YES];
    let footer_layer: id = msg_send![footer, layer];
    let footer_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.08f64 alpha:0.95f64];
    let footer_bg_cg: id = msg_send![footer_bg, CGColor];
    let _: () = msg_send![footer_layer, setBackgroundColor: footer_bg_cg];
    let border_layer: id = msg_send![class!(CALayer), layer];
    let divider_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.05f64];
    let divider_color_cg: id = msg_send![divider_color, CGColor];
    let _: () = msg_send![border_layer, setBackgroundColor: divider_color_cg];
    let _: () = msg_send![border_layer, setFrame:NSRect::new(NSPoint::new(0.0, footer_height - 1.0), NSSize::new(bounds.size.width, 1.0))];
    let _: () = msg_send![footer_layer, addSublayer:border_layer];

    // Left label (static hint)
    let left_label: id = msg_send![class!(NSTextField), alloc];
    let left_label: id = msg_send![left_label, initWithFrame: NSRect::new(NSPoint::new(style::CONTENT_SIDE_INSET, 6.0), NSSize::new(260.0, footer_height - 10.0))];
    let _: () = msg_send![left_label, setBezeled: NO];
    let _: () = msg_send![left_label, setEditable: NO];
    let _: () = msg_send![left_label, setDrawsBackground: NO];
    let _: () = msg_send![left_label, setBordered: NO];
    let left_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:11.0 weight:0.2];
    let _: () = msg_send![left_label, setFont: left_font];
    let left_text = NSString::alloc(nil).init_str("↑ / ↓  Navigate    •    Tab  Clipboard history");
    let _: () = msg_send![left_label, setStringValue: left_text];
    let left_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.45f64];
    let _: () = msg_send![left_label, setTextColor: left_color];
    let _: () = msg_send![footer, addSubview: left_label];

    // Center label
    let center_label: id = msg_send![class!(NSTextField), alloc];
    let center_label: id = msg_send![center_label, initWithFrame: NSRect::new(NSPoint::new(bounds.size.width / 2.0 - 160.0, 6.0), NSSize::new(320.0, footer_height - 10.0))];
    let _: () = msg_send![center_label, setBezeled: NO];
    let _: () = msg_send![center_label, setEditable: NO];
    let _: () = msg_send![center_label, setDrawsBackground: NO];
    let _: () = msg_send![center_label, setBordered: NO];
    let _: () = msg_send![center_label, setAlignment: 1];
    let center_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:11.0 weight:0.2];
    let _: () = msg_send![center_label, setFont: center_font];
    let center_text = NSString::alloc(nil).init_str("⌘⌫ Clear query    •    ⌘, Settings");
    let _: () = msg_send![center_label, setStringValue: center_text];
    let center_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.45f64];
    let _: () = msg_send![center_label, setTextColor: center_color];
    let _: () = msg_send![footer, addSubview: center_label];

    // Right label (static for now)
    let right_label: id = msg_send![class!(NSTextField), alloc];
    let right_label: id = msg_send![right_label, initWithFrame: NSRect::new(NSPoint::new(bounds.size.width - style::CONTENT_SIDE_INSET - 200.0, 6.0), NSSize::new(190.0, footer_height - 10.0))];
    let _: () = msg_send![right_label, setBezeled: NO];
    let _: () = msg_send![right_label, setEditable: NO];
    let _: () = msg_send![right_label, setDrawsBackground: NO];
    let _: () = msg_send![right_label, setBordered: NO];
    let _: () = msg_send![right_label, setAlignment: 2];
    let right_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:11.0 weight:0.2];
    let _: () = msg_send![right_label, setFont: right_font];
    let right_text = NSString::alloc(nil).init_str("↩  Launch / Paste    •    Esc  Hide");
    let _: () = msg_send![right_label, setStringValue: right_text];
    let right_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.45f64];
    let _: () = msg_send![right_label, setTextColor: right_color];
    let _: () = msg_send![footer, addSubview: right_label];

    let _: () = msg_send![content_view, addSubview: footer];

    // Initial load
    let _: () = msg_send![table, reloadData];
}

// Search field delegate for live updates
unsafe fn register_search_delegate_class() {
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
            // Get the text field from notification
            let object: id = msg_send![notification, object];
            if object == nil {
                return;
            }

            // Ensure cursor is white (sometimes needs re-applying)
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

            // Cancel previous search
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

            // Spawn fast search + final file-enhanced search sequentially
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

            // Store the new handle
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

fn build_search_rows(results: &[search_engine::SearchResult]) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = Vec::new();
    for r in results {
        match r {
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
                ..
            } => {
                let title = custom_name
                    .clone()
                    .unwrap_or_else(|| content.chars().take(40).collect());
                rows.push((title, preview.chars().take(80).collect()));
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

fn dispatch_search_results(results: Vec<search_engine::SearchResult>, rows: Vec<(String, String)>) {
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

fn should_fetch_file_results(query: &str) -> bool {
    query.len() >= 3 && !query.starts_with(':')
}

fn abort_current_search() {
    if let Ok(mut handle_guard) = CURRENT_SEARCH.lock() {
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }
}

unsafe fn find_search_field() -> Option<id> {
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

unsafe fn get_current_search_query() -> Option<String> {
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

unsafe fn create_search_font() -> id {
    // Larger, medium weight for better readability and prominence
    msg_send![class!(NSFont), systemFontOfSize:22.0 weight:0.23] // Medium weight
}

unsafe fn center_window_with_snap(ns_window: id) {
    // Get screen frame
    let screen: id = msg_send![class!(NSScreen), mainScreen];
    let screen_frame: NSRect = msg_send![screen, visibleFrame];
    let window_frame: NSRect = msg_send![ns_window, frame];

    // Center horizontally, place near top
    let x = screen_frame.origin.x + (screen_frame.size.width - window_frame.size.width) / 2.0;
    let y = screen_frame.origin.y + screen_frame.size.height - window_frame.size.height - 120.0;

    // Position window
    let _: () = msg_send![ns_window, setFrame: NSRect::new(NSPoint::new(x, y), window_frame.size) display: YES];

    // Quick fade-in
    let _: () = msg_send![ns_window, setAlphaValue: 0.0f64];
    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let context: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![context, setDuration: 0.2f64];

    let animator: id = msg_send![ns_window, animator];
    let _: () = msg_send![animator, setAlphaValue: 1.0f64];

    let _: () = msg_send![class!(NSAnimationContext), endGrouping];
}

unsafe fn create_status_bar_item() {
    let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
    let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64]; // NSVariableStatusItemLength

    // Retain the status item so it doesn't get deallocated
    let _: id = msg_send![status_item, retain];

    // Set icon (using SF Symbol or text for now)
    let button: id = msg_send![status_item, button];

    // Try to use SF Symbol (macOS 11+), fallback to text
    let symbol_name = NSString::alloc(nil).init_str("crown.fill");
    let image: id = msg_send![class!(NSImage), imageWithSystemSymbolName:symbol_name accessibilityDescription:nil];

    if image != nil {
        let _: () = msg_send![button, setImage: image];
    } else {
        // Fallback: use text icon
        let _: () = msg_send![button, setTitle: NSString::alloc(nil).init_str("👑")];
    }

    // Create menu
    let menu: id = msg_send![class!(NSMenu), alloc];
    let menu: id = msg_send![menu, init];

    // Get the app for terminate action
    let app: id = msg_send![class!(NSApplication), sharedApplication];

    // Menu item: Open ViceroyKiller with shortcut shown
    let open_item: id = msg_send![class!(NSMenuItem), alloc];
    let open_item: id = msg_send![open_item,
        initWithTitle: NSString::alloc(nil).init_str("Open ViceroyKiller")
        action: sel!(showMainWindow:)
        keyEquivalent: NSString::alloc(nil).init_str(" ") // Space key
    ];
    // Show shortcut as ⇧⌘Space (Shift+Command+Space)
    let modifiers: usize = (1 << 17) | (1 << 20); // NSEventModifierFlagShift | NSEventModifierFlagCommand
    let _: () = msg_send![open_item, setKeyEquivalentModifierMask: modifiers];
    let _: () = msg_send![menu, addItem: open_item];

    // Separator
    let sep1: id = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep1];

    // Version number (grayed out, non-interactive)
    let version_item: id = msg_send![class!(NSMenuItem), alloc];
    let version_item: id = msg_send![version_item,
        initWithTitle: NSString::alloc(nil).init_str("ViceroyKiller v1.0.0")
        action: nil
        keyEquivalent: NSString::alloc(nil).init_str("")
    ];
    let _: () = msg_send![version_item, setEnabled: NO];
    let _: () = msg_send![menu, addItem: version_item];

    // About
    let about_item: id = msg_send![class!(NSMenuItem), alloc];
    let about_item: id = msg_send![about_item,
        initWithTitle: NSString::alloc(nil).init_str("About ViceroyKiller")
        action: sel!(orderFrontStandardAboutPanel:)
        keyEquivalent: NSString::alloc(nil).init_str("")
    ];
    let _: () = msg_send![about_item, setTarget: app];
    let _: () = msg_send![menu, addItem: about_item];

    // Separator
    let sep2: id = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep2];

    // Settings
    let settings_item: id = msg_send![class!(NSMenuItem), alloc];
    let settings_item: id = msg_send![settings_item,
        initWithTitle: NSString::alloc(nil).init_str("Settings...")
        action: sel!(showSettings:)
        keyEquivalent: NSString::alloc(nil).init_str(",")
    ];
    let _: () = msg_send![menu, addItem: settings_item];

    // Clipboard history quick access
    let history_item: id = msg_send![class!(NSMenuItem), alloc];
    let history_item: id = msg_send![history_item,
        initWithTitle: NSString::alloc(nil).init_str("Clipboard History")
        action: sel!(showClipboardHistory:)
        keyEquivalent: NSString::alloc(nil).init_str("h")
    ];
    let _: () = msg_send![menu, addItem: history_item];

    // Preferences toggles
    let sep_toggle: id = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep_toggle];

    // Dismiss on Escape
    let esc_item: id = msg_send![class!(NSMenuItem), alloc];
    let esc_item: id = msg_send![esc_item,
        initWithTitle: NSString::alloc(nil).init_str("Dismiss on Escape")
        action: sel!(toggleDismissOnEscape:)
        keyEquivalent: NSString::alloc(nil).init_str("")
    ];
    // Set initial state from loaded settings
    let esc_state: i64 = match DISMISS_ON_ESCAPE.lock() {
        Ok(g) => {
            if *g {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    };
    let _: () = msg_send![esc_item, setState: esc_state];
    let _: () = msg_send![menu, addItem: esc_item];

    // Dismiss on Click Away
    let click_item: id = msg_send![class!(NSMenuItem), alloc];
    let click_item: id = msg_send![click_item,
        initWithTitle: NSString::alloc(nil).init_str("Dismiss on Click Away")
        action: sel!(toggleDismissOnClickAway:)
        keyEquivalent: NSString::alloc(nil).init_str("")
    ];
    let click_state: i64 = match DISMISS_ON_CLICK_AWAY.lock() {
        Ok(g) => {
            if *g {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    };
    let _: () = msg_send![click_item, setState: click_state];
    let _: () = msg_send![menu, addItem: click_item];

    // Separator
    let sep3: id = msg_send![class!(NSMenuItem), separatorItem];
    let _: () = msg_send![menu, addItem: sep3];

    // Quit with shortcut
    let quit_item: id = msg_send![class!(NSMenuItem), alloc];
    let quit_item: id = msg_send![quit_item,
        initWithTitle: NSString::alloc(nil).init_str("Quit ViceroyKiller")
        action: sel!(terminate:)
        keyEquivalent: NSString::alloc(nil).init_str("q")
    ];
    let _: () = msg_send![quit_item, setTarget: app];
    let _: () = msg_send![menu, addItem: quit_item];

    // Create menu action handler object
    let actions_target = create_menu_actions_target();

    // Set targets for our custom menu items
    let _: () = msg_send![open_item, setTarget: actions_target];
    let _: () = msg_send![settings_item, setTarget: actions_target];
    let _: () = msg_send![history_item, setTarget: actions_target];
    let _: () = msg_send![esc_item, setTarget: actions_target];
    let _: () = msg_send![click_item, setTarget: actions_target];

    // Attach menu to status item
    let _: () = msg_send![status_item, setMenu: menu];
}

unsafe fn create_menu_actions_target() -> id {
    // Register class if needed
    if objc::runtime::Class::get("MKMenuActions").is_none() {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MKMenuActions", superclass).unwrap();

        extern "C" fn show_main_window(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                settings_view::hide_settings_panel();
                // Update global state
                if let Ok(mut w) = WINDOW_SHOWING.lock() {
                    *w = true;
                }

                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];

                if count > 0 {
                    let window: id = msg_send![windows, objectAtIndex: 0];
                    bring_window_to_front_with_search_reset(window);
                }
            }
        }

        extern "C" fn show_settings(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                // Update global state
                if let Ok(mut w) = WINDOW_SHOWING.lock() {
                    *w = true;
                }

                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];

                if count > 0 {
                    let window: id = msg_send![windows, objectAtIndex: 0];
                    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                    let _: () = msg_send![window, makeKeyAndOrderFront: nil];

                    settings_view::show_settings_panel();
                }
            }
        }

        extern "C" fn show_clipboard_history(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                settings_view::hide_settings_panel();
                if let Ok(mut w) = WINDOW_SHOWING.lock() {
                    *w = true;
                }
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];
                if count > 0 {
                    let window: id = msg_send![windows, objectAtIndex: 0];
                    bring_window_to_front_with_search_reset(window);
                }
                abort_current_search();
                show_clipboard_history_view();
            }
        }

        extern "C" fn toggle_dismiss_on_escape(_this: &Object, _cmd: Sel, sender: id) {
            unsafe {
                // Toggle global flag (best-effort)
                let new_state = match DISMISS_ON_ESCAPE.lock() {
                    Ok(mut val) => {
                        *val = !*val;
                        *val
                    }
                    Err(poisoned) => {
                        let mut guard = poisoned.into_inner();
                        *guard = !*guard;
                        *guard
                    }
                };
                let state: i64 = if new_state { 1 } else { 0 };
                let _: () = msg_send![sender, setState: state];

                // Persist to settings.json (best-effort)
                if let Ok(mut s) = settings::load() {
                    s.dismiss_on_escape = new_state;
                    let _ = settings::save(&s);
                }
            }
        }

        extern "C" fn toggle_dismiss_on_click(_this: &Object, _cmd: Sel, sender: id) {
            unsafe {
                let new_state = match DISMISS_ON_CLICK_AWAY.lock() {
                    Ok(mut v) => {
                        *v = !*v;
                        *v
                    }
                    Err(poisoned) => {
                        let mut g = poisoned.into_inner();
                        *g = !*g;
                        *g
                    }
                };
                let state: i64 = if new_state { 1 } else { 0 };
                let _: () = msg_send![sender, setState: state];

                if let Ok(mut s) = settings::load() {
                    s.dismiss_on_click_away = new_state;
                    let _ = settings::save(&s);
                }
            }
        }

        decl.add_method(
            sel!(showMainWindow:),
            show_main_window as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(showClipboardHistory:),
            show_clipboard_history as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(showSettings:),
            show_settings as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(toggleDismissOnEscape:),
            toggle_dismiss_on_escape as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(toggleDismissOnClickAway:),
            toggle_dismiss_on_click as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }

    // Create and retain an instance
    let actions_class = class!(MKMenuActions);
    let actions: id = msg_send![actions_class, new];
    let _: id = msg_send![actions, retain];
    actions
}

unsafe fn bring_window_to_front_only(window: id) {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let _: () = msg_send![app, activateIgnoringOtherApps: YES];
    let _: () = msg_send![window, makeKeyAndOrderFront: nil];
}

unsafe fn bring_window_to_front_with_search_reset(window: id) {
    bring_window_to_front_only(window);
    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::Search;
    }
    update_clipboard_preview_selection(None);
    table::update_preview_layout(false);

    if let Some(search_field) = find_search_field() {
        let empty: id = NSString::alloc(nil).init_str("");
        let _: () = msg_send![search_field, setStringValue: empty];

        if let Ok(mut tr) = TABLE_RESULTS.lock() {
            tr.clear();
        } else {
            eprintln!("WARNING: TABLE_RESULTS lock poisoned in menu action");
        }
        if let Ok(mut td) = TABLE_DATA.lock() {
            td.clear();
        } else {
            eprintln!("WARNING: TABLE_DATA lock poisoned in menu action");
        }
        table::reload_table();

        let field_editor: id = msg_send![window, fieldEditor:YES forObject:search_field];
        if field_editor != nil {
            let white: id = msg_send![class!(NSColor), whiteColor];
            let _: () = msg_send![field_editor, setInsertionPointColor: white];
        }

        let _: () = msg_send![window, makeFirstResponder: search_field];
    }
}

unsafe fn setup_app_observer(_ns_window: id) {
    // Create observer class for app deactivation (click away)
    if objc::runtime::Class::get("MKAppObserver").is_some() {
        return;
    }

    let superclass = class!(NSObject);
    let mut decl = ClassDecl::new("MKAppObserver", superclass).unwrap();

    extern "C" fn app_did_resign_active(_this: &Object, _cmd: Sel, _notification: id) {
        unsafe {
            // Best-effort: check showing flag without panicking on poisoned mutex
            let showing = match WINDOW_SHOWING.lock() {
                Ok(g) => *g,
                Err(poisoned) => *poisoned.into_inner(),
            };
            if showing {
                // Check click-away preference
                let dismiss_click = match DISMISS_ON_CLICK_AWAY.lock() {
                    Ok(g) => *g,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                if !dismiss_click {
                    return;
                }
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];
                if count > 0 {
                    let window: id = msg_send![windows, objectAtIndex:0];
                    let _: () = msg_send![window, orderOut: nil];
                    if let Ok(mut w) = WINDOW_SHOWING.lock() {
                        *w = false;
                    }
                } else {
                    // No windows found
                }
            }
        }
    }

    decl.add_method(
        sel!(appDidResignActive:),
        app_did_resign_active as extern "C" fn(&Object, Sel, id),
    );

    let observer_class = decl.register();
    let observer: id = msg_send![observer_class, new];

    // Register for deactivation notifications
    let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let name_cstr = std::ffi::CString::new("NSApplicationDidResignActiveNotification").unwrap();
    let name: id = msg_send![class!(NSString), stringWithUTF8String: name_cstr.as_ptr()];
    let _: () = msg_send![center, addObserver:observer selector:sel!(appDidResignActive:) name:name object:app];
}

unsafe fn setup_window_delegate(ns_window: id) {
    if objc::runtime::Class::get("MKWindowDelegate").is_some() {
        let delegate_class = class!(MKWindowDelegate);
        let delegate: id = msg_send![delegate_class, new];
        let _: () = msg_send![ns_window, setDelegate: delegate];
        return;
    }

    let superclass = class!(NSObject);
    let mut decl = ClassDecl::new("MKWindowDelegate", superclass).unwrap();

    extern "C" fn window_did_become_key(_this: &Object, _cmd: Sel, _notification: id) {
        unsafe {
            // Ensure search field has focus when window becomes key
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let windows: id = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            if count > 0 {
                let window: id = msg_send![windows, objectAtIndex:0];
                if let Some(search_field) = find_search_field() {
                    let _: () = msg_send![window, makeFirstResponder: search_field];
                }
            }
        }
    }

    extern "C" fn window_did_resign_key(_this: &Object, _cmd: Sel, _notification: id) {
        // Window lost key focus - hide it after brief delay
        // Dispatch to main thread after small delay
        dispatch::Queue::main().exec_after(std::time::Duration::from_millis(100), move || {
            unsafe {
                // Check preference (best-effort)
                let dismiss_click = match DISMISS_ON_CLICK_AWAY.lock() {
                    Ok(g) => *g,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                if !dismiss_click {
                    return;
                }
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let count: usize = msg_send![windows, count];
                if count > 0 {
                    let window: id = msg_send![windows, objectAtIndex:0];
                    let is_key: BOOL = msg_send![window, isKeyWindow];
                    let is_visible: BOOL = msg_send![window, isVisible];

                    // Only hide if window is visible but not key
                    if is_visible == YES && is_key == NO {
                        if let Ok(mut w) = WINDOW_SHOWING.lock() {
                            *w = false;
                        }
                        let _: () = msg_send![window, orderOut: nil];
                    }
                }
            }
        });
    }

    // Add protocol conformance
    decl.add_method(
        sel!(windowDidBecomeKey:),
        window_did_become_key as extern "C" fn(&Object, Sel, id),
    );
    decl.add_method(
        sel!(windowDidResignKey:),
        window_did_resign_key as extern "C" fn(&Object, Sel, id),
    );

    let delegate_class = decl.register();
    let delegate: id = msg_send![delegate_class, new];
    let _: () = msg_send![ns_window, setDelegate: delegate];
}

fn print_cli_help() {
    println!("Viceroy v{}", env!("CARGO_PKG_VERSION"));
    println!("Usage: viceroy [--no-update-check] [--silent-update-check]\n");
    println!("Options:");
    println!(
        "  --no-update-check        Skip background update checks (or set {}=1)",
        UPDATE_CHECK_DISABLED_ENV
    );
    println!(
        "  --silent-update-check    Run update checks without prompting (or set {}=1)",
        UPDATE_SILENT_ENV
    );
    println!("Environment:");
    println!(
        "  {}    Override the metadata URL used for update checks",
        UPDATE_METADATA_URL_ENV
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_cli_help();
        return;
    }

    // Set panic hook to see errors
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("!!! PANIC: {:?}", panic_info);
        // Capture and print a backtrace to help locate source of panic
        let bt = std::backtrace::Backtrace::force_capture();
        eprintln!("Backtrace:\n{:?}", bt);
    }));

    // Minimal logging for production
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Error)
        .init();

    if updater::update_check_disabled(&args) {
        info!(
            "Skipping update check because --no-update-check was passed or {} is set",
            UPDATE_CHECK_DISABLED_ENV
        );
    } else {
        let silent = updater::silent_update_check(&args);
        ui::state::SEARCH_RT.spawn(async move {
            if let Err(err) = updater::check_for_updates(silent).await {
                error!("Update check failed: {err:#}");
            }
        });
    }

    let app = App::new("com.viceroy.app", ViceroyApp);
    app.run();
}

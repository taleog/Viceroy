use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::OnceLock;

use crate::ui::state::{TableMode, TABLE_MODE};
use crate::ui::table;

static SETTINGS_PANEL: OnceLock<usize> = OnceLock::new();
static SETTINGS_ACTION_TARGET: OnceLock<usize> = OnceLock::new();

pub unsafe fn show_settings_panel() {
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    let windows: id = msg_send![app, windows];
    let count: usize = msg_send![windows, count];
    if count == 0 {
        return;
    }
    let window: id = msg_send![windows, objectAtIndex: 0];
    let content_view: id = msg_send![window, contentView];
    if content_view == nil {
        return;
    }

    let bounds: NSRect = msg_send![content_view, bounds];
    let panel = ensure_panel(content_view, bounds);
    let _: () = msg_send![panel, setHidden: NO];
    let _: () = msg_send![panel, setNeedsDisplay: YES];
    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::Settings;
    }
    unsafe {
        table::sync_window_height_with_state();
    }
}

pub unsafe fn hide_settings_panel() {
    if let Some(ptr) = SETTINGS_PANEL.get() {
        let panel: id = *ptr as id;
        let _: () = msg_send![panel, setHidden: YES];
    }
    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::Search;
    }
    unsafe {
        table::sync_window_height_with_state();
    }
}

unsafe fn ensure_panel(content_view: id, bounds: NSRect) -> id {
    if let Some(ptr) = SETTINGS_PANEL.get() {
        let panel: id = *ptr as id;
        let _: () = msg_send![panel, setFrame: bounds];
        return panel;
    }

    let panel = create_panel(content_view, bounds);
    let _ = SETTINGS_PANEL.set(panel as usize);
    panel
}

unsafe fn create_panel(content_view: id, bounds: NSRect) -> id {
    let panel: id = msg_send![class!(NSView), alloc];
    let panel: id = msg_send![panel, initWithFrame: bounds];
    let _: () = msg_send![panel, setWantsLayer: YES];
    let layer: id = msg_send![panel, layer];
    let bg_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.08f64 alpha:0.9f64];
    let bg_cg: id = msg_send![bg_color, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_cg];
    let _: () = msg_send![layer, setCornerRadius: 26.0f64];
    let _: () = msg_send![panel, setHidden: YES];
    let _: () = msg_send![panel, setAutoresizingMask: 3];

    let height = bounds.size.height;
    let title_frame = NSRect::new(NSPoint::new(36.0, height - 76.0), NSSize::new(320.0, 34.0));
    let title: id = msg_send![class!(NSTextField), alloc];
    let title: id = msg_send![title, initWithFrame: title_frame];
    let _: () = msg_send![title, setBezeled: NO];
    let _: () = msg_send![title, setEditable: NO];
    let _: () = msg_send![title, setDrawsBackground: NO];
    let _: () = msg_send![title, setSelectable: NO];
    let title_font: id = msg_send![class!(NSFont), boldSystemFontOfSize:24.0];
    let title_text_color: id = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![title, setFont: title_font];
    let _: () = msg_send![title, setTextColor: title_text_color];
    let _: () = msg_send![title, setStringValue: NSString::alloc(nil).init_str("Settings")];

    let detail_frame = NSRect::new(NSPoint::new(36.0, height - 112.0), NSSize::new(420.0, 42.0));
    let detail: id = msg_send![class!(NSTextField), alloc];
    let detail: id = msg_send![detail, initWithFrame: detail_frame];
    let _: () = msg_send![detail, setBezeled: NO];
    let _: () = msg_send![detail, setEditable: NO];
    let _: () = msg_send![detail, setDrawsBackground: NO];
    let _: () = msg_send![detail, setSelectable: NO];
    let detail_font: id = msg_send![class!(NSFont), systemFontOfSize:13.5];
    let detail_text_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.6f64];
    let _: () = msg_send![detail, setFont: detail_font];
    let _: () = msg_send![detail, setTextColor: detail_text_color];
    let _: () = msg_send![detail, setStringValue: NSString::alloc(nil).init_str("Configure hotkey, clipboard history, and theme preferences from here.")];

    let button_width = 150.0;
    let button_height = 32.0;
    let button_frame = NSRect::new(
        NSPoint::new(bounds.size.width - button_width - 30.0, height - 70.0),
        NSSize::new(button_width, button_height),
    );
    let button: id = msg_send![class!(NSButton), alloc];
    let button: id = msg_send![button, initWithFrame: button_frame];
    let _: () = msg_send![button, setBezelStyle: 1];
    let _: () = msg_send![button, setBordered: YES];
    let _: () = msg_send![button, setTitle: NSString::alloc(nil).init_str("Back to search")];
    let target = ensure_actions_target();
    let _: () = msg_send![button, setTarget: target];
    let _: () = msg_send![button, setAction: sel!(closeSettingsPanel:)];

    let _: () = msg_send![panel, addSubview: title];
    let _: () = msg_send![panel, addSubview: detail];
    let _: () = msg_send![panel, addSubview: button];
    let _: () = msg_send![content_view, addSubview: panel];
    panel
}

unsafe fn ensure_actions_target() -> id {
    if let Some(ptr) = SETTINGS_ACTION_TARGET.get() {
        return *ptr as id;
    }
    let target = register_action_class();
    let _ = SETTINGS_ACTION_TARGET.set(target as usize);
    target
}

unsafe fn register_action_class() -> id {
    if Class::get("MKSettingsPanelActions").is_none() {
        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("MKSettingsPanelActions", superclass).unwrap();

        extern "C" fn close_panel(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                hide_settings_panel();
            }
        }

        decl.add_method(
            sel!(closeSettingsPanel:),
            close_panel as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }

    let cls = class!(MKSettingsPanelActions);
    let target: id = msg_send![cls, new];
    let _: () = msg_send![target, retain];
    target
}

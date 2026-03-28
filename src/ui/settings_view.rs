use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::OnceLock;

use crate::settings;
use crate::sync;
use crate::ui::state::{TableMode, DISMISS_ON_CLICK_AWAY, DISMISS_ON_ESCAPE, TABLE_MODE};
use crate::ui::table;

static SETTINGS_PANEL: OnceLock<usize> = OnceLock::new();
static SETTINGS_ACTION_TARGET: OnceLock<usize> = OnceLock::new();
static SETTINGS_CONTROLS: OnceLock<SettingsControls> = OnceLock::new();

struct SettingsControls {
    hotkey_field: usize,
    max_slider: usize,
    max_label: usize,
    toggle_escape: usize,
    toggle_click: usize,
    sync_enabled_toggle: usize,
    sync_device_name_field: usize,
    sync_device_id_field: usize,
    sync_server_url_field: usize,
    sync_auth_token_field: usize,
    sync_status_label: usize,
    sync_message_label: usize,
}

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
    let _: () = msg_send![panel, removeFromSuperview];
    let _: () = msg_send![content_view, addSubview: panel];
    let _: () = msg_send![panel, setHidden: NO];
    let _: () = msg_send![panel, setNeedsDisplay: YES];
    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::Settings;
    }
    unsafe {
        table::sync_window_height_with_state();
    }
    populate_controls_from_settings();
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
    let _: () = msg_send![detail, setStringValue: NSString::alloc(nil).init_str("Configure hotkey, behavior, and self-hosted sync preferences from here.")];

    let target = ensure_actions_target();
    let card_margin = 36.0;
    let card_inset = 20.0;
    let card_width = bounds.size.width - card_margin * 2.0;
    let mut top_anchor = height - 170.0;

    // General card for hotkey
    let general_card_height = 136.0;
    let general_card_y = (top_anchor - general_card_height).max(card_margin);
    let general_card: id = msg_send![class!(NSView), alloc];
    let general_card: id = msg_send![general_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, general_card_y), NSSize::new(card_width, general_card_height))];
    let _: () = msg_send![general_card, setWantsLayer: YES];
    let general_layer: id = msg_send![general_card, layer];
    let general_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.1f64 alpha:0.85f64];
    let general_bg_cg: id = msg_send![general_bg, CGColor];
    let _: () = msg_send![general_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![general_layer, setBackgroundColor: general_bg_cg];
    let _: () = msg_send![general_layer, setBorderWidth: 1.0f64];
    let general_border: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let general_border_cg: id = msg_send![general_border, CGColor];
    let _: () = msg_send![general_layer, setBorderColor: general_border_cg];

    let hotkey_heading: id = msg_send![class!(NSTextField), alloc];
    let hotkey_heading: id = msg_send![hotkey_heading, initWithFrame:NSRect::new(NSPoint::new(card_inset, general_card_height - card_inset - 24.0), NSSize::new(card_width - card_inset * 2.0, 22.0))];
    let _: () = msg_send![hotkey_heading, setBezeled: NO];
    let _: () = msg_send![hotkey_heading, setEditable: NO];
    let _: () = msg_send![hotkey_heading, setDrawsBackground: NO];
    let _: () = msg_send![hotkey_heading, setBordered: NO];
    let general_font: id = msg_send![class!(NSFont), systemFontOfSize:15.0 weight:0.6];
    let _: () = msg_send![hotkey_heading, setFont: general_font];
    let heading_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.9f64];
    let _: () = msg_send![hotkey_heading, setTextColor: heading_color];
    let _: () =
        msg_send![hotkey_heading, setStringValue: NSString::alloc(nil).init_str("Global hotkey")];

    let hotkey_caption: id = msg_send![class!(NSTextField), alloc];
    let hotkey_caption: id = msg_send![hotkey_caption, initWithFrame:NSRect::new(NSPoint::new(card_inset, general_card_height - card_inset - 48.0), NSSize::new(card_width - card_inset * 2.0, 18.0))];
    let _: () = msg_send![hotkey_caption, setBezeled: NO];
    let _: () = msg_send![hotkey_caption, setEditable: NO];
    let _: () = msg_send![hotkey_caption, setDrawsBackground: NO];
    let _: () = msg_send![hotkey_caption, setBordered: NO];
    let caption_font: id = msg_send![class!(NSFont), systemFontOfSize:12.5];
    let caption_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.65f64];
    let _: () = msg_send![hotkey_caption, setFont: caption_font];
    let _: () = msg_send![hotkey_caption, setTextColor: caption_color];
    let _: () = msg_send![hotkey_caption, setStringValue: NSString::alloc(nil).init_str("Choose the keyboard shortcut that summons Viceroy from anywhere.")];

    let hotkey_field: id = msg_send![class!(NSTextField), alloc];
    let hotkey_field_frame = NSRect::new(
        NSPoint::new(card_inset, card_inset),
        NSSize::new(card_width - card_inset * 2.0, 30.0),
    );
    let hotkey_field: id = msg_send![hotkey_field, initWithFrame: hotkey_field_frame];
    let _: () = msg_send![hotkey_field, setBezeled: YES];
    let _: () = msg_send![hotkey_field, setEditable: YES];
    let _: () = msg_send![hotkey_field, setDrawsBackground: YES];
    let _: () = msg_send![hotkey_field, setBordered: YES];
    let _: () = msg_send![general_card, addSubview: hotkey_heading];
    let _: () = msg_send![general_card, addSubview: hotkey_caption];
    let _: () = msg_send![general_card, addSubview: hotkey_field];

    // Behavior card
    top_anchor = general_card_y - 24.0;
    let behavior_card_height = 220.0;
    let behavior_card_y = (top_anchor - behavior_card_height).max(card_margin);
    let behavior_card: id = msg_send![class!(NSView), alloc];
    let behavior_card: id = msg_send![behavior_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, behavior_card_y), NSSize::new(card_width, behavior_card_height))];
    let _: () = msg_send![behavior_card, setWantsLayer: YES];
    let behavior_layer: id = msg_send![behavior_card, layer];
    let behavior_bg: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:0.11f64 alpha:0.82f64];
    let behavior_bg_cg: id = msg_send![behavior_bg, CGColor];
    let _: () = msg_send![behavior_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![behavior_layer, setBackgroundColor: behavior_bg_cg];
    let _: () = msg_send![behavior_layer, setBorderWidth: 1.0f64];
    let behavior_border: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let behavior_border_cg: id = msg_send![behavior_border, CGColor];
    let _: () = msg_send![behavior_layer, setBorderColor: behavior_border_cg];

    let behavior_heading: id = msg_send![class!(NSTextField), alloc];
    let behavior_heading: id = msg_send![behavior_heading, initWithFrame:NSRect::new(NSPoint::new(card_inset, behavior_card_height - card_inset - 24.0), NSSize::new(card_width - card_inset * 2.0, 22.0))];
    let _: () = msg_send![behavior_heading, setBezeled: NO];
    let _: () = msg_send![behavior_heading, setEditable: NO];
    let _: () = msg_send![behavior_heading, setDrawsBackground: NO];
    let _: () = msg_send![behavior_heading, setBordered: NO];
    let _: () = msg_send![behavior_heading, setFont: general_font];
    let _: () = msg_send![behavior_heading, setTextColor: heading_color];
    let _: () = msg_send![behavior_heading, setStringValue: NSString::alloc(nil).init_str("Results & behavior")];

    let max_label: id = msg_send![class!(NSTextField), alloc];
    let max_label: id = msg_send![max_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, behavior_card_height - card_inset - 50.0), NSSize::new(card_width - card_inset * 2.0, 20.0))];
    let _: () = msg_send![max_label, setBezeled: NO];
    let _: () = msg_send![max_label, setEditable: NO];
    let _: () = msg_send![max_label, setDrawsBackground: NO];
    let _: () = msg_send![max_label, setBordered: NO];
    let _: () =
        msg_send![max_label, setStringValue: NSString::alloc(nil).init_str("Max results: 50")];
    let max_label_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.65f64];
    let _: () = msg_send![max_label, setTextColor: max_label_color];

    let slider_frame = NSRect::new(
        NSPoint::new(card_inset, behavior_card_height - card_inset - 90.0),
        NSSize::new(card_width - card_inset * 2.0, 26.0),
    );
    let max_slider: id = msg_send![class!(NSSlider), alloc];
    let max_slider: id = msg_send![max_slider, initWithFrame: slider_frame];
    let _: () = msg_send![max_slider, setMinValue: 10.0];
    let _: () = msg_send![max_slider, setMaxValue: 200.0];
    let _: () = msg_send![max_slider, setAllowsTickMarkValues: YES];
    let _: () = msg_send![max_slider, setNumberOfTickMarks: 10];
    let _: () = msg_send![max_slider, setTarget: target];
    let _: () = msg_send![max_slider, setAction: sel!(maxResultsSliderChanged:)];

    let esc_toggle: id = msg_send![class!(NSButton), alloc];
    let esc_toggle: id = msg_send![esc_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, card_inset + 60.0), NSSize::new(card_width / 2.0 - card_inset, 28.0))];
    let _: () = msg_send![esc_toggle, setButtonType: 3];
    let _: () = msg_send![esc_toggle, setTitle: NSString::alloc(nil).init_str("Dismiss on Escape")];
    let _: () = msg_send![esc_toggle, setTarget: target];
    let _: () = msg_send![esc_toggle, setAction: sel!(toggleSetting:)];

    let click_toggle: id = msg_send![class!(NSButton), alloc];
    let click_toggle: id = msg_send![click_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, card_inset + 24.0), NSSize::new(card_width / 2.0 - card_inset, 28.0))];
    let _: () = msg_send![click_toggle, setButtonType: 3];
    let _: () =
        msg_send![click_toggle, setTitle: NSString::alloc(nil).init_str("Dismiss on click away")];
    let _: () = msg_send![click_toggle, setTarget: target];
    let _: () = msg_send![click_toggle, setAction: sel!(toggleSetting:)];

    let _: () = msg_send![behavior_card, addSubview: behavior_heading];
    let _: () = msg_send![behavior_card, addSubview: max_label];
    let _: () = msg_send![behavior_card, addSubview: max_slider];
    let _: () = msg_send![behavior_card, addSubview: esc_toggle];
    let _: () = msg_send![behavior_card, addSubview: click_toggle];

    // Sync card
    top_anchor = behavior_card_y - 24.0;
    let sync_card_height = 320.0;
    let sync_card_y = (top_anchor - sync_card_height).max(card_margin);
    let sync_card: id = msg_send![class!(NSView), alloc];
    let sync_card: id = msg_send![sync_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, sync_card_y), NSSize::new(card_width, sync_card_height))];
    let _: () = msg_send![sync_card, setWantsLayer: YES];
    let sync_layer: id = msg_send![sync_card, layer];
    let sync_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.1f64 alpha:0.84f64];
    let sync_bg_cg: id = msg_send![sync_bg, CGColor];
    let _: () = msg_send![sync_layer, setCornerRadius: 18.0f64];
    let _: () = msg_send![sync_layer, setBackgroundColor: sync_bg_cg];
    let _: () = msg_send![sync_layer, setBorderWidth: 1.0f64];
    let sync_border: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let sync_border_cg: id = msg_send![sync_border, CGColor];
    let _: () = msg_send![sync_layer, setBorderColor: sync_border_cg];

    let sync_heading: id = msg_send![class!(NSTextField), alloc];
    let sync_heading: id = msg_send![sync_heading, initWithFrame:NSRect::new(NSPoint::new(card_inset, sync_card_height - card_inset - 24.0), NSSize::new(card_width - card_inset * 2.0, 22.0))];
    let _: () = msg_send![sync_heading, setBezeled: NO];
    let _: () = msg_send![sync_heading, setEditable: NO];
    let _: () = msg_send![sync_heading, setDrawsBackground: NO];
    let _: () = msg_send![sync_heading, setBordered: NO];
    let _: () = msg_send![sync_heading, setFont: general_font];
    let _: () = msg_send![sync_heading, setTextColor: heading_color];
    let _: () =
        msg_send![sync_heading, setStringValue: NSString::alloc(nil).init_str("Cross-device sync")];

    let sync_caption: id = msg_send![class!(NSTextField), alloc];
    let sync_caption: id = msg_send![sync_caption, initWithFrame:NSRect::new(NSPoint::new(card_inset, sync_card_height - card_inset - 48.0), NSSize::new(card_width - card_inset * 2.0, 18.0))];
    let _: () = msg_send![sync_caption, setBezeled: NO];
    let _: () = msg_send![sync_caption, setEditable: NO];
    let _: () = msg_send![sync_caption, setDrawsBackground: NO];
    let _: () = msg_send![sync_caption, setBordered: NO];
    let _: () = msg_send![sync_caption, setFont: caption_font];
    let _: () = msg_send![sync_caption, setTextColor: caption_color];
    let _: () = msg_send![sync_caption, setStringValue: NSString::alloc(nil).init_str("Point Viceroy at your self-hosted sync server and inspect the current device state.")];

    let left_label_width = 82.0;
    let left_column_width = card_width - 360.0;
    let right_column_x = card_width - 280.0;
    let field_width = left_column_width - left_label_width - 12.0;

    let sync_enabled_toggle: id = msg_send![class!(NSButton), alloc];
    let sync_enabled_toggle: id = msg_send![sync_enabled_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, 132.0), NSSize::new(160.0, 28.0))];
    let _: () = msg_send![sync_enabled_toggle, setButtonType: 3];
    let _: () =
        msg_send![sync_enabled_toggle, setTitle: NSString::alloc(nil).init_str("Enable sync")];
    let _: () = msg_send![sync_enabled_toggle, setTarget: target];
    let _: () = msg_send![sync_enabled_toggle, setAction: sel!(toggleSetting:)];

    let refresh_button: id = msg_send![class!(NSButton), alloc];
    let refresh_button: id = msg_send![refresh_button, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 130.0), NSSize::new(120.0, 30.0))];
    let _: () = msg_send![refresh_button, setBezelStyle: 1];
    let _: () =
        msg_send![refresh_button, setTitle: NSString::alloc(nil).init_str("Refresh status")];
    let _: () = msg_send![refresh_button, setTarget: target];
    let _: () = msg_send![refresh_button, setAction: sel!(refreshSyncStatus:)];

    let device_name_label: id = msg_send![class!(NSTextField), alloc];
    let device_name_label: id = msg_send![device_name_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 98.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![device_name_label, setBezeled: NO];
    let _: () = msg_send![device_name_label, setEditable: NO];
    let _: () = msg_send![device_name_label, setDrawsBackground: NO];
    let _: () = msg_send![device_name_label, setBordered: NO];
    let _: () = msg_send![device_name_label, setFont: caption_font];
    let _: () = msg_send![device_name_label, setTextColor: caption_color];
    set_string(device_name_label, "Device");

    let sync_device_name_field: id = msg_send![class!(NSTextField), alloc];
    let sync_device_name_field: id = msg_send![sync_device_name_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 92.0), NSSize::new(field_width, 30.0))];
    let _: () = msg_send![sync_device_name_field, setBezeled: YES];
    let _: () = msg_send![sync_device_name_field, setEditable: YES];
    let _: () = msg_send![sync_device_name_field, setDrawsBackground: YES];
    let _: () = msg_send![sync_device_name_field, setBordered: YES];

    let server_url_label: id = msg_send![class!(NSTextField), alloc];
    let server_url_label: id = msg_send![server_url_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 62.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![server_url_label, setBezeled: NO];
    let _: () = msg_send![server_url_label, setEditable: NO];
    let _: () = msg_send![server_url_label, setDrawsBackground: NO];
    let _: () = msg_send![server_url_label, setBordered: NO];
    let _: () = msg_send![server_url_label, setFont: caption_font];
    let _: () = msg_send![server_url_label, setTextColor: caption_color];
    set_string(server_url_label, "Server");

    let sync_server_url_field: id = msg_send![class!(NSTextField), alloc];
    let sync_server_url_field: id = msg_send![sync_server_url_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 56.0), NSSize::new(field_width, 30.0))];
    let _: () = msg_send![sync_server_url_field, setBezeled: YES];
    let _: () = msg_send![sync_server_url_field, setEditable: YES];
    let _: () = msg_send![sync_server_url_field, setDrawsBackground: YES];
    let _: () = msg_send![sync_server_url_field, setBordered: YES];

    let auth_token_label: id = msg_send![class!(NSTextField), alloc];
    let auth_token_label: id = msg_send![auth_token_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 26.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![auth_token_label, setBezeled: NO];
    let _: () = msg_send![auth_token_label, setEditable: NO];
    let _: () = msg_send![auth_token_label, setDrawsBackground: NO];
    let _: () = msg_send![auth_token_label, setBordered: NO];
    let _: () = msg_send![auth_token_label, setFont: caption_font];
    let _: () = msg_send![auth_token_label, setTextColor: caption_color];
    set_string(auth_token_label, "Token");

    let sync_auth_token_field: id = msg_send![class!(NSSecureTextField), alloc];
    let sync_auth_token_field: id = msg_send![sync_auth_token_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 20.0), NSSize::new(field_width, 30.0))];
    let _: () = msg_send![sync_auth_token_field, setBezeled: YES];
    let _: () = msg_send![sync_auth_token_field, setEditable: YES];
    let _: () = msg_send![sync_auth_token_field, setDrawsBackground: YES];
    let _: () = msg_send![sync_auth_token_field, setBordered: YES];

    let sync_status_label: id = msg_send![class!(NSTextField), alloc];
    let sync_status_label: id = msg_send![sync_status_label, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 82.0), NSSize::new(240.0, 130.0))];
    let _: () = msg_send![sync_status_label, setBezeled: NO];
    let _: () = msg_send![sync_status_label, setEditable: NO];
    let _: () = msg_send![sync_status_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_status_label, setBordered: NO];
    let _: () = msg_send![sync_status_label, setUsesSingleLineMode: NO];
    let _: () = msg_send![sync_status_label, setLineBreakMode: 4];
    let _: () = msg_send![sync_status_label, setFont: caption_font];
    let _: () = msg_send![sync_status_label, setTextColor: caption_color];

    let sync_device_id_field: id = msg_send![class!(NSTextField), alloc];
    let sync_device_id_field: id = msg_send![sync_device_id_field, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 52.0), NSSize::new(240.0, 24.0))];
    let _: () = msg_send![sync_device_id_field, setBezeled: NO];
    let _: () = msg_send![sync_device_id_field, setEditable: NO];
    let _: () = msg_send![sync_device_id_field, setDrawsBackground: NO];
    let _: () = msg_send![sync_device_id_field, setBordered: NO];
    let _: () = msg_send![sync_device_id_field, setSelectable: YES];
    let _: () = msg_send![sync_device_id_field, setFont: caption_font];
    let _: () = msg_send![sync_device_id_field, setTextColor: caption_color];

    let sync_message_label: id = msg_send![class!(NSTextField), alloc];
    let sync_message_label: id = msg_send![sync_message_label, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 14.0), NSSize::new(240.0, 28.0))];
    let _: () = msg_send![sync_message_label, setBezeled: NO];
    let _: () = msg_send![sync_message_label, setEditable: NO];
    let _: () = msg_send![sync_message_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_message_label, setBordered: NO];
    let _: () = msg_send![sync_message_label, setSelectable: NO];
    let _: () = msg_send![sync_message_label, setFont: caption_font];
    let message_color: id = msg_send![class!(NSColor), colorWithCalibratedRed:0.51f64 green:0.76f64 blue:1.0f64 alpha:1.0f64];
    let _: () = msg_send![sync_message_label, setTextColor: message_color];

    let _: () = msg_send![sync_card, addSubview: sync_heading];
    let _: () = msg_send![sync_card, addSubview: sync_caption];
    let _: () = msg_send![sync_card, addSubview: sync_enabled_toggle];
    let _: () = msg_send![sync_card, addSubview: refresh_button];
    let _: () = msg_send![sync_card, addSubview: device_name_label];
    let _: () = msg_send![sync_card, addSubview: sync_device_name_field];
    let _: () = msg_send![sync_card, addSubview: server_url_label];
    let _: () = msg_send![sync_card, addSubview: sync_server_url_field];
    let _: () = msg_send![sync_card, addSubview: auth_token_label];
    let _: () = msg_send![sync_card, addSubview: sync_auth_token_field];
    let _: () = msg_send![sync_card, addSubview: sync_status_label];
    let _: () = msg_send![sync_card, addSubview: sync_device_id_field];
    let _: () = msg_send![sync_card, addSubview: sync_message_label];

    let save_button_frame = NSRect::new(
        NSPoint::new(bounds.size.width - card_margin - 160.0, card_margin + 12.0),
        NSSize::new(150.0, 32.0),
    );
    let save_button: id = msg_send![class!(NSButton), alloc];
    let save_button: id = msg_send![save_button, initWithFrame: save_button_frame];
    let _: () = msg_send![save_button, setBezelStyle: 4];
    let _: () = msg_send![save_button, setTitle: NSString::alloc(nil).init_str("Save settings")];
    let _: () = msg_send![save_button, setTarget: target];
    let _: () = msg_send![save_button, setAction: sel!(saveSettings:)];

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
    let _: () = msg_send![panel, addSubview: general_card];
    let _: () = msg_send![panel, addSubview: behavior_card];
    let _: () = msg_send![panel, addSubview: sync_card];
    let _: () = msg_send![panel, addSubview: save_button];
    let _: () = msg_send![panel, addSubview: button];
    let controls = SettingsControls {
        hotkey_field: hotkey_field as usize,
        max_slider: max_slider as usize,
        max_label: max_label as usize,
        toggle_escape: esc_toggle as usize,
        toggle_click: click_toggle as usize,
        sync_enabled_toggle: sync_enabled_toggle as usize,
        sync_device_name_field: sync_device_name_field as usize,
        sync_device_id_field: sync_device_id_field as usize,
        sync_server_url_field: sync_server_url_field as usize,
        sync_auth_token_field: sync_auth_token_field as usize,
        sync_status_label: sync_status_label as usize,
        sync_message_label: sync_message_label as usize,
    };
    let _ = SETTINGS_CONTROLS.set(controls);
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

fn populate_controls_from_settings() {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        if let Ok(settings) = settings::load() {
            unsafe {
                set_string(id_from(controls.hotkey_field), &settings.hotkey);
                let slider: id = id_from(controls.max_slider);
                let _: () = msg_send![slider, setIntValue: settings.max_results as i32];
                slider_value_changed(slider);
                let esc_state: i64 = if settings.dismiss_on_escape { 1 } else { 0 };
                let click_state: i64 = if settings.dismiss_on_click_away { 1 } else { 0 };
                let _: () = msg_send![id_from(controls.toggle_escape), setState: esc_state];
                let _: () = msg_send![id_from(controls.toggle_click), setState: click_state];
                let sync_enabled_state: i64 = if settings.sync.enabled { 1 } else { 0 };
                let _: () =
                    msg_send![id_from(controls.sync_enabled_toggle), setState: sync_enabled_state];
                set_string(
                    id_from(controls.sync_device_name_field),
                    &settings.sync.device_name,
                );
                set_string(
                    id_from(controls.sync_device_id_field),
                    &settings.sync.device_id,
                );
                set_string(
                    id_from(controls.sync_server_url_field),
                    settings.sync.server_url.as_deref().unwrap_or(""),
                );
                set_string(
                    id_from(controls.sync_auth_token_field),
                    settings.sync.auth_token.as_deref().unwrap_or(""),
                );
                let summary = build_sync_summary(&sync::SyncStatus {
                    device: sync::LocalDevice {
                        device_id: settings.sync.device_id.clone(),
                        device_name: settings.sync.device_name.clone(),
                        platform: std::env::consts::OS.to_string(),
                    },
                    server_url: settings.sync.server_url.clone(),
                    connection_state: if settings.sync.enabled {
                        sync::SyncConnectionState::Disconnected
                    } else {
                        sync::SyncConnectionState::Disabled
                    },
                    last_successful_sync_at: None,
                    last_error: None,
                    pending_operations: 0,
                });
                set_string(id_from(controls.sync_status_label), &summary);
                set_sync_message(
                    "Sync status loaded. Save settings after changing server details.",
                );
            }
        }
    }
}

unsafe fn slider_value_changed(slider: id) {
    let value: i32 = msg_send![slider, intValue];
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        set_string(
            id_from(controls.max_label),
            &format!("Max results: {}", value.max(10)),
        );
    }
}

unsafe fn apply_settings_from_ui() {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        let mut current_settings =
            settings::load().unwrap_or_else(|_| settings::Settings::default());
        let hotkey = get_string(id_from(controls.hotkey_field));
        let slider_value: i32 = msg_send![id_from(controls.max_slider), intValue];
        let slider_value = slider_value.clamp(10, 200);
        let esc_state: i16 = msg_send![id_from(controls.toggle_escape), state];
        let click_state: i16 = msg_send![id_from(controls.toggle_click), state];
        let sync_enabled_state: i16 = msg_send![id_from(controls.sync_enabled_toggle), state];
        let sync_enabled = sync_enabled_state == 1;
        let sync_device_name = get_string(id_from(controls.sync_device_name_field));
        let sync_server_url_input = get_string(id_from(controls.sync_server_url_field));
        let sync_auth_token = get_string(id_from(controls.sync_auth_token_field));
        let old_enabled = current_settings.sync.enabled;
        let old_server_url = current_settings.sync.server_url.clone().unwrap_or_default();
        let old_auth_token = current_settings.sync.auth_token.clone().unwrap_or_default();
        let normalized_server_url = if sync_enabled {
            let input = sync_server_url_input.trim();
            if input.is_empty() {
                set_sync_message("Enter a sync server URL before enabling sync.");
                return;
            }
            match sync::normalize_server_url(input) {
                Ok(url) => {
                    if let Err(err) = sync::validate_server_url_for_local_device(&url) {
                        set_sync_message(&format!("Invalid sync server URL: {err:#}"));
                        return;
                    }
                    url
                }
                Err(err) => {
                    set_sync_message(&format!("Invalid sync server URL: {err:#}"));
                    return;
                }
            }
        } else {
            sync_server_url_input.trim().to_string()
        };
        current_settings.hotkey = hotkey;
        current_settings.max_results = slider_value as usize;
        current_settings.dismiss_on_escape = esc_state == 1;
        current_settings.dismiss_on_click_away = click_state == 1;
        current_settings.sync.enabled = sync_enabled;
        current_settings.sync.device_name = sync_device_name.trim().to_string();
        current_settings.sync.server_url = non_empty(normalized_server_url.trim());
        current_settings.sync.auth_token = non_empty(sync_auth_token.trim());
        if let Err(err) = settings::save(&current_settings) {
            set_sync_message(&format!("Failed to save settings: {err:#}"));
            eprintln!("Failed to save settings: {}", err);
            return;
        }
        if let Ok(mut esc_guard) = DISMISS_ON_ESCAPE.lock() {
            *esc_guard = current_settings.dismiss_on_escape;
        }
        if let Ok(mut click_guard) = DISMISS_ON_CLICK_AWAY.lock() {
            *click_guard = current_settings.dismiss_on_click_away;
        }
        set_string(
            id_from(controls.sync_server_url_field),
            current_settings.sync.server_url.as_deref().unwrap_or(""),
        );
        match sync::init() {
            Ok(status) => {
                set_sync_summary(&status);
                set_string(
                    id_from(controls.sync_device_id_field),
                    &status.device.device_id,
                );
            }
            Err(err) => {
                set_sync_message(&format!("Settings saved, but sync init failed: {err:#}"));
                slider_value_changed(id_from(controls.max_slider));
                return;
            }
        }
        if sync_enabled {
            if let Err(err) = sync::start_background_worker() {
                set_sync_message(&format!(
                    "Settings saved, but sync worker failed to start: {err:#}"
                ));
                slider_value_changed(id_from(controls.max_slider));
                return;
            }
        }
        let connection_changed = old_enabled
            && sync_enabled
            && (old_server_url != current_settings.sync.server_url.clone().unwrap_or_default()
                || old_auth_token != current_settings.sync.auth_token.clone().unwrap_or_default());
        let sync_message = if connection_changed {
            "Sync settings saved. Restart Viceroy to apply server URL or token changes."
        } else if sync_enabled && !old_enabled {
            "Sync enabled. The background worker will use this server for new uploads."
        } else if !sync_enabled {
            "Sync settings saved. Sync is disabled until you re-enable it."
        } else {
            "Sync settings saved."
        };
        if let Ok(status) = sync::status() {
            set_sync_summary(&status);
        }
        set_sync_message(sync_message);
        slider_value_changed(id_from(controls.max_slider));
    }
}

unsafe fn refresh_sync_status_controls(message: Option<&str>) {
    let Some(controls) = SETTINGS_CONTROLS.get() else {
        return;
    };

    match sync::status() {
        Ok(status) => {
            set_sync_summary(&status);
            set_string(
                id_from(controls.sync_device_id_field),
                &status.device.device_id,
            );
            set_sync_message(
                message
                    .unwrap_or("Sync status loaded. Save settings after changing server details."),
            );
        }
        Err(err) => {
            set_string(
                id_from(controls.sync_status_label),
                "Sync status is not available yet.",
            );
            if let Some(message) = message {
                set_sync_message(message);
            } else {
                set_sync_message(&format!("Failed to load sync status: {err:#}"));
            }
        }
    }
}

unsafe fn set_sync_message(message: &str) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        set_string(id_from(controls.sync_message_label), message);
    }
}

unsafe fn id_from(ptr: usize) -> id {
    ptr as id
}

unsafe fn set_string(view: id, value: &str) {
    let text = NSString::alloc(nil).init_str(value);
    let _: () = msg_send![view, setStringValue: text];
}

unsafe fn get_string(view: id) -> String {
    let value: id = msg_send![view, stringValue];
    if value == nil {
        return String::new();
    }
    let cstr: *const c_char = msg_send![value, UTF8String];
    if cstr.is_null() {
        return String::new();
    }
    CStr::from_ptr(cstr).to_string_lossy().to_string()
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

unsafe fn set_sync_summary(status: &sync::SyncStatus) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        set_string(
            id_from(controls.sync_status_label),
            &build_sync_summary(status),
        );
    }
}

fn build_sync_summary(status: &sync::SyncStatus) -> String {
    format!(
        "Connection: {}\nServer: {}\nLast sync: {}\nQueued: {}\nLast error: {}",
        status.connection_state.display_label(),
        status.server_url.as_deref().unwrap_or("Not configured"),
        sync::format_timestamp(status.last_successful_sync_at),
        status.pending_operations,
        status.last_error.as_deref().unwrap_or("None"),
    )
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

        extern "C" fn max_results_slider_changed(_this: &Object, _cmd: Sel, slider: id) {
            unsafe {
                slider_value_changed(slider);
            }
        }

        extern "C" fn toggle_setting(_this: &Object, _cmd: Sel, _sender: id) {
            // Handled automatically by the button; no extra action needed
        }

        extern "C" fn save_settings_action(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                apply_settings_from_ui();
            }
        }

        extern "C" fn refresh_sync_status_action(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                refresh_sync_status_controls(Some("Sync status refreshed."));
            }
        }

        decl.add_method(
            sel!(closeSettingsPanel:),
            close_panel as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(maxResultsSliderChanged:),
            max_results_slider_changed as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(toggleSetting:),
            toggle_setting as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(saveSettings:),
            save_settings_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(refreshSyncStatus:),
            refresh_sync_status_action as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }

    let cls = class!(MKSettingsPanelActions);
    let target: id = msg_send![cls, new];
    let _: () = msg_send![target, retain];
    target
}

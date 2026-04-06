use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use tokio::runtime::Runtime;

use crate::settings;
use crate::sync;
use crate::ui::state::{TableMode, DISMISS_ON_CLICK_AWAY, DISMISS_ON_ESCAPE, TABLE_MODE};
use crate::ui::table;

static SETTINGS_PANEL: OnceLock<usize> = OnceLock::new();
static SETTINGS_ACTION_TARGET: OnceLock<usize> = OnceLock::new();
static SETTINGS_CONTROLS: OnceLock<SettingsControls> = OnceLock::new();
static SETTINGS_ACTIVE_TAB: AtomicUsize = AtomicUsize::new(0);

const SETTINGS_TAB_GENERAL: usize = 0;
const SETTINGS_TAB_BEHAVIOR: usize = 1;
const SETTINGS_TAB_OBSIDIAN: usize = 2;
const SETTINGS_TAB_SYNC: usize = 3;
const SETTINGS_CARD_RADIUS: f64 = 22.0;
const SETTINGS_GROUP_RADIUS: f64 = 16.0;
const SETTINGS_SECTION_INSET: f64 = 24.0;
const SETTINGS_BUTTON_HEIGHT: f64 = 34.0;
const SETTINGS_STATUS_PANEL_WIDTH: f64 = 312.0;

struct SettingsControls {
    tab_control: usize,
    general_card: usize,
    behavior_card: usize,
    obsidian_card: usize,
    sync_card: usize,
    hotkey_field: usize,
    max_slider: usize,
    max_label: usize,
    toggle_paste_after_restore: usize,
    toggle_escape: usize,
    toggle_click: usize,
    obsidian_enabled_toggle: usize,
    obsidian_open_in_obsidian_toggle: usize,
    obsidian_vault_path_field: usize,
    obsidian_vault_name_field: usize,
    obsidian_status_label: usize,
    sync_enabled_toggle: usize,
    sync_mirror_clipboard_toggle: usize,
    sync_device_name_field: usize,
    sync_device_id_field: usize,
    sync_server_url_field: usize,
    sync_auth_token_field: usize,
    sync_indicator_label: usize,
    sync_status_label: usize,
    sync_devices_label: usize,
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

    if let Ok(mut mode) = TABLE_MODE.lock() {
        *mode = TableMode::Settings;
    }
    unsafe {
        table::sync_window_height_with_state();
    }
    let bounds: NSRect = msg_send![content_view, bounds];
    let panel = ensure_panel(content_view, bounds);
    let _: () = msg_send![panel, removeFromSuperview];
    let _: () = msg_send![content_view, addSubview: panel];
    let _: () = msg_send![panel, setHidden: NO];
    let _: () = msg_send![panel, setNeedsDisplay: YES];
    populate_controls_from_settings();
    apply_active_settings_tab();
    let window: id = msg_send![content_view, window];
    if window != nil {
        focus_active_settings_control(window);
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

unsafe fn settings_white(alpha: f64) -> id {
    msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:alpha]
}

unsafe fn settings_surface(alpha: f64) -> id {
    msg_send![class!(NSColor), colorWithCalibratedWhite:0.12f64 alpha:alpha]
}

unsafe fn apply_settings_surface(view: id, background: id, corner_radius: f64, border_alpha: f64) {
    let _: () = msg_send![view, setWantsLayer: YES];
    let layer: id = msg_send![view, layer];
    if layer == nil {
        return;
    }
    let bg_cg: id = msg_send![background, CGColor];
    let border_color: id = settings_white(border_alpha);
    let border_cg: id = msg_send![border_color, CGColor];
    let _: () = msg_send![layer, setCornerRadius: corner_radius];
    let _: () = msg_send![layer, setMasksToBounds: YES];
    let _: () = msg_send![layer, setBackgroundColor: bg_cg];
    let _: () = msg_send![layer, setBorderWidth: 1.0f64];
    let _: () = msg_send![layer, setBorderColor: border_cg];
}

unsafe fn set_button_title(button: id, title: &str, size: f64, weight: f64, alpha: f64) {
    let attrs: id = msg_send![class!(NSMutableDictionary), dictionary];
    let font: id = msg_send![class!(NSFont), systemFontOfSize:size weight:weight];
    let color: id = settings_white(alpha);
    let _: () = msg_send![attrs, setObject: font forKey: NSString::alloc(nil).init_str("NSFont")];
    let _: () = msg_send![attrs, setObject: color forKey: NSString::alloc(nil).init_str("NSColor")];
    let attributed: id = msg_send![class!(NSAttributedString), alloc];
    let attributed: id = msg_send![
        attributed,
        initWithString: NSString::alloc(nil).init_str(title)
        attributes: attrs
    ];
    let _: () = msg_send![button, setAttributedTitle: attributed];
}

unsafe fn style_action_button(button: id, title: &str, primary: bool) {
    let _: () = msg_send![button, setBordered: NO];
    let _: () = msg_send![button, setBezelStyle: 0];
    let _: () = msg_send![button, setFocusRingType: 0];
    let background: id = if primary {
        msg_send![class!(NSColor), controlAccentColor]
    } else {
        settings_surface(0.82)
    };
    let border_alpha = if primary { 0.0 } else { 0.08 };
    apply_settings_surface(button, background, 12.0, border_alpha);
    set_button_title(button, title, 13.0, 0.52, 0.96);
}

unsafe fn style_toggle(button: id, title: &str) {
    let _: () = msg_send![button, setButtonType: 3];
    let _: () = msg_send![button, setBordered: NO];
    let _: () = msg_send![button, setFocusRingType: 0];
    let accent: id = msg_send![class!(NSColor), controlAccentColor];
    let _: () = msg_send![button, setContentTintColor: accent];
    set_button_title(button, title, 13.0, 0.44, 0.88);
}

unsafe fn style_input(view: id, editable: bool, selectable: bool, monospaced: bool) {
    let _: () = msg_send![view, setBezeled: NO];
    let _: () = msg_send![view, setBordered: NO];
    let _: () = msg_send![view, setDrawsBackground: YES];
    let _: () = msg_send![view, setBackgroundColor: settings_surface(0.96)];
    let _: () = msg_send![view, setEditable: if editable { YES } else { NO }];
    let _: () = msg_send![view, setSelectable: if selectable { YES } else { NO }];
    let _: () = msg_send![view, setFocusRingType: 0];
    let font: id = if monospaced {
        msg_send![class!(NSFont), monospacedSystemFontOfSize:13.0 weight:0.3]
    } else {
        msg_send![class!(NSFont), systemFontOfSize:13.5 weight:0.42]
    };
    let _: () = msg_send![view, setFont: font];
    let _: () = msg_send![view, setTextColor: settings_white(0.94)];
    apply_settings_surface(view, settings_surface(0.96), 12.0, 0.08);
}

unsafe fn style_info_block(view: id, monospaced: bool) {
    let font: id = if monospaced {
        msg_send![class!(NSFont), monospacedSystemFontOfSize:12.0 weight:0.28]
    } else {
        msg_send![class!(NSFont), systemFontOfSize:12.5 weight:0.34]
    };
    let _: () = msg_send![view, setFont: font];
    let _: () = msg_send![view, setTextColor: settings_white(0.74)];
    let _: () = msg_send![view, setUsesSingleLineMode: NO];
    let _: () = msg_send![view, setLineBreakMode: 4];
    apply_settings_surface(view, settings_surface(0.52), 14.0, 0.08);
}

unsafe fn create_panel(content_view: id, bounds: NSRect) -> id {
    let panel: id = msg_send![class!(NSView), alloc];
    let panel: id = msg_send![panel, initWithFrame: bounds];
    let _: () = msg_send![panel, setWantsLayer: YES];
    let layer: id = msg_send![panel, layer];
    let bg_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.05f64 alpha:0.70f64];
    let bg_cg: id = msg_send![bg_color, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: bg_cg];
    let _: () = msg_send![layer, setBorderWidth: 1.0f64];
    let border_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let border_cg: id = msg_send![border_color, CGColor];
    let _: () = msg_send![layer, setBorderColor: border_cg];
    let _: () = msg_send![layer, setCornerRadius: 26.0f64];
    let _: () = msg_send![layer, setMasksToBounds: YES];
    let _: () = msg_send![panel, setHidden: YES];
    let _: () = msg_send![panel, setAutoresizingMask: 18];

    let backdrop: id = msg_send![class!(NSVisualEffectView), alloc];
    let backdrop: id = msg_send![backdrop, initWithFrame: bounds];
    let _: () = msg_send![backdrop, setMaterial: 12];
    let _: () = msg_send![backdrop, setBlendingMode: 0];
    let _: () = msg_send![backdrop, setState: 1];
    let _: () = msg_send![backdrop, setAutoresizingMask: 18];
    let _: () = msg_send![backdrop, setWantsLayer: YES];
    let backdrop_layer: id = msg_send![backdrop, layer];
    let _: () = msg_send![backdrop_layer, setCornerRadius: 26.0f64];
    let _: () = msg_send![panel, addSubview: backdrop];

    let height = bounds.size.height;
    let title_frame = NSRect::new(
        NSPoint::new(SETTINGS_SECTION_INSET, height - 72.0),
        NSSize::new(320.0, 34.0),
    );
    let title: id = msg_send![class!(NSTextField), alloc];
    let title: id = msg_send![title, initWithFrame: title_frame];
    let _: () = msg_send![title, setBezeled: NO];
    let _: () = msg_send![title, setEditable: NO];
    let _: () = msg_send![title, setDrawsBackground: NO];
    let _: () = msg_send![title, setSelectable: NO];
    let title_font: id = msg_send![class!(NSFont), boldSystemFontOfSize:28.0];
    let title_text_color: id = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![title, setFont: title_font];
    let _: () = msg_send![title, setTextColor: title_text_color];
    let _: () = msg_send![title, setStringValue: NSString::alloc(nil).init_str("Settings")];

    let detail_frame = NSRect::new(
        NSPoint::new(SETTINGS_SECTION_INSET, height - 108.0),
        NSSize::new(420.0, 42.0),
    );
    let detail: id = msg_send![class!(NSTextField), alloc];
    let detail: id = msg_send![detail, initWithFrame: detail_frame];
    let _: () = msg_send![detail, setBezeled: NO];
    let _: () = msg_send![detail, setEditable: NO];
    let _: () = msg_send![detail, setDrawsBackground: NO];
    let _: () = msg_send![detail, setSelectable: NO];
    let detail_font: id = msg_send![class!(NSFont), systemFontOfSize:13.5];
    let detail_text_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.68f64];
    let _: () = msg_send![detail, setFont: detail_font];
    let _: () = msg_send![detail, setTextColor: detail_text_color];
    let _: () = msg_send![detail, setStringValue: NSString::alloc(nil).init_str("Tune the hotkey, result behavior, and sync settings from one focused macOS panel.")];

    let target = ensure_actions_target();
    let card_margin = SETTINGS_SECTION_INSET;
    let card_inset = SETTINGS_GROUP_RADIUS + 4.0;
    let card_width = bounds.size.width - card_margin * 2.0;
    let card_top = height - 196.0;

    let tab_control_frame = NSRect::new(NSPoint::new(4.0, 6.0), NSSize::new(306.0, 28.0));
    let tab_control: id = msg_send![class!(NSSegmentedControl), alloc];
    let tab_control: id = msg_send![tab_control, initWithFrame: tab_control_frame];
    let _: () = msg_send![tab_control, setSegmentCount: 4];
    let _: () = msg_send![
        tab_control,
        setLabel: NSString::alloc(nil).init_str("General")
        forSegment: 0
    ];
    let _: () = msg_send![
        tab_control,
        setLabel: NSString::alloc(nil).init_str("Behavior")
        forSegment: 1
    ];
    let _: () = msg_send![
        tab_control,
        setLabel: NSString::alloc(nil).init_str("Obsidian")
        forSegment: 2
    ];
    let _: () = msg_send![
        tab_control,
        setLabel: NSString::alloc(nil).init_str("Sync")
        forSegment: 3
    ];
    let _: () = msg_send![tab_control, setTrackingMode: 1];
    let _: () = msg_send![tab_control, setSelectedSegment: SETTINGS_ACTIVE_TAB.load(Ordering::SeqCst) as isize];
    let _: () = msg_send![tab_control, setControlSize: 1];
    let _: () = msg_send![tab_control, setTarget: target];
    let _: () = msg_send![tab_control, setAction: sel!(changeSettingsTab:)];

    let tab_shell: id = msg_send![class!(NSView), alloc];
    let tab_shell: id = msg_send![
        tab_shell,
        initWithFrame: NSRect::new(
            NSPoint::new(card_margin - 4.0, height - 156.0),
            NSSize::new(410.0, 40.0)
        )
    ];
    apply_settings_surface(
        tab_shell,
        settings_surface(0.6),
        SETTINGS_GROUP_RADIUS,
        0.08,
    );
    let _: () = msg_send![tab_shell, addSubview: tab_control];

    // General card for hotkey
    let general_card_height = 196.0;
    let general_card_y = (card_top - general_card_height).max(card_margin);
    let general_card: id = msg_send![class!(NSView), alloc];
    let general_card: id = msg_send![general_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, general_card_y), NSSize::new(card_width, general_card_height))];
    let _: () = msg_send![general_card, setWantsLayer: YES];
    let general_layer: id = msg_send![general_card, layer];
    let general_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.10f64 alpha:0.86f64];
    let general_bg_cg: id = msg_send![general_bg, CGColor];
    let _: () = msg_send![general_layer, setCornerRadius: SETTINGS_CARD_RADIUS];
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
        NSPoint::new(card_inset, 72.0),
        NSSize::new(card_width - card_inset * 2.0, 30.0),
    );
    let hotkey_field: id = msg_send![hotkey_field, initWithFrame: hotkey_field_frame];
    style_input(hotkey_field, true, true, false);
    let hotkey_tip_block: id = msg_send![class!(NSTextField), alloc];
    let hotkey_tip_block: id = msg_send![
        hotkey_tip_block,
        initWithFrame: NSRect::new(
            NSPoint::new(card_inset, 24.0),
            NSSize::new(card_width - card_inset * 2.0, 34.0)
        )
    ];
    let _: () = msg_send![hotkey_tip_block, setBezeled: NO];
    let _: () = msg_send![hotkey_tip_block, setEditable: NO];
    let _: () = msg_send![hotkey_tip_block, setDrawsBackground: NO];
    let _: () = msg_send![hotkey_tip_block, setBordered: NO];
    let _: () = msg_send![hotkey_tip_block, setSelectable: NO];
    style_info_block(hotkey_tip_block, false);
    let _: () = msg_send![
        hotkey_tip_block,
        setStringValue: NSString::alloc(nil)
            .init_str("Choose something unlikely to collide with Spotlight. Cmd+, always reopens Settings.")
    ];
    let _: () = msg_send![general_card, addSubview: hotkey_heading];
    let _: () = msg_send![general_card, addSubview: hotkey_caption];
    let _: () = msg_send![general_card, addSubview: hotkey_field];
    let _: () = msg_send![general_card, addSubview: hotkey_tip_block];

    // Behavior card
    let behavior_card_height = 256.0;
    let behavior_card_y = (card_top - behavior_card_height).max(card_margin);
    let behavior_card: id = msg_send![class!(NSView), alloc];
    let behavior_card: id = msg_send![behavior_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, behavior_card_y), NSSize::new(card_width, behavior_card_height))];
    let _: () = msg_send![behavior_card, setWantsLayer: YES];
    let behavior_layer: id = msg_send![behavior_card, layer];
    let behavior_bg: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:0.11f64 alpha:0.82f64];
    let behavior_bg_cg: id = msg_send![behavior_bg, CGColor];
    let _: () = msg_send![behavior_layer, setCornerRadius: SETTINGS_CARD_RADIUS];
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
    let _: () = msg_send![max_slider, setAllowsTickMarkValuesOnly: YES];
    let _: () = msg_send![max_slider, setNumberOfTickMarks: 10];
    let _: () = msg_send![max_slider, setTarget: target];
    let _: () = msg_send![max_slider, setAction: sel!(maxResultsSliderChanged:)];

    let paste_toggle: id = msg_send![class!(NSButton), alloc];
    let paste_toggle: id = msg_send![paste_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, card_inset + 96.0), NSSize::new(card_width - card_inset * 2.0, 28.0))];
    style_toggle(
        paste_toggle,
        "Paste immediately after restoring a clipboard item",
    );
    let _: () = msg_send![paste_toggle, setTarget: target];
    let _: () = msg_send![paste_toggle, setAction: sel!(toggleSetting:)];

    let esc_toggle: id = msg_send![class!(NSButton), alloc];
    let esc_toggle: id = msg_send![esc_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, card_inset + 60.0), NSSize::new(card_width / 2.0 - card_inset, 28.0))];
    style_toggle(esc_toggle, "Dismiss on Escape");
    let _: () = msg_send![esc_toggle, setTarget: target];
    let _: () = msg_send![esc_toggle, setAction: sel!(toggleSetting:)];

    let click_toggle: id = msg_send![class!(NSButton), alloc];
    let click_toggle: id = msg_send![click_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, card_inset + 24.0), NSSize::new(card_width / 2.0 - card_inset, 28.0))];
    style_toggle(click_toggle, "Dismiss on click away");
    let _: () = msg_send![click_toggle, setTarget: target];
    let _: () = msg_send![click_toggle, setAction: sel!(toggleSetting:)];

    let _: () = msg_send![behavior_card, addSubview: behavior_heading];
    let _: () = msg_send![behavior_card, addSubview: max_label];
    let _: () = msg_send![behavior_card, addSubview: max_slider];
    let _: () = msg_send![behavior_card, addSubview: paste_toggle];
    let _: () = msg_send![behavior_card, addSubview: esc_toggle];
    let _: () = msg_send![behavior_card, addSubview: click_toggle];

    // Obsidian card
    let obsidian_card_height = 334.0;
    let obsidian_card_y = (card_top - obsidian_card_height).max(card_margin);
    let obsidian_card: id = msg_send![class!(NSView), alloc];
    let obsidian_card: id = msg_send![obsidian_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, obsidian_card_y), NSSize::new(card_width, obsidian_card_height))];
    let _: () = msg_send![obsidian_card, setWantsLayer: YES];
    let obsidian_layer: id = msg_send![obsidian_card, layer];
    let obsidian_bg: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:0.11f64 alpha:0.84f64];
    let obsidian_bg_cg: id = msg_send![obsidian_bg, CGColor];
    let _: () = msg_send![obsidian_layer, setCornerRadius: SETTINGS_CARD_RADIUS];
    let _: () = msg_send![obsidian_layer, setBackgroundColor: obsidian_bg_cg];
    let _: () = msg_send![obsidian_layer, setBorderWidth: 1.0f64];
    let obsidian_border: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.08f64];
    let obsidian_border_cg: id = msg_send![obsidian_border, CGColor];
    let _: () = msg_send![obsidian_layer, setBorderColor: obsidian_border_cg];

    let obsidian_heading: id = msg_send![class!(NSTextField), alloc];
    let obsidian_heading: id = msg_send![obsidian_heading, initWithFrame:NSRect::new(NSPoint::new(card_inset, obsidian_card_height - card_inset - 24.0), NSSize::new(card_width - card_inset * 2.0, 22.0))];
    let _: () = msg_send![obsidian_heading, setBezeled: NO];
    let _: () = msg_send![obsidian_heading, setEditable: NO];
    let _: () = msg_send![obsidian_heading, setDrawsBackground: NO];
    let _: () = msg_send![obsidian_heading, setBordered: NO];
    let _: () = msg_send![obsidian_heading, setFont: general_font];
    let _: () = msg_send![obsidian_heading, setTextColor: heading_color];
    let _: () = msg_send![
        obsidian_heading,
        setStringValue: NSString::alloc(nil).init_str("Obsidian vault")
    ];

    let obsidian_caption: id = msg_send![class!(NSTextField), alloc];
    let obsidian_caption: id = msg_send![obsidian_caption, initWithFrame:NSRect::new(NSPoint::new(card_inset, obsidian_card_height - card_inset - 48.0), NSSize::new(card_width - card_inset * 2.0, 18.0))];
    let _: () = msg_send![obsidian_caption, setBezeled: NO];
    let _: () = msg_send![obsidian_caption, setEditable: NO];
    let _: () = msg_send![obsidian_caption, setDrawsBackground: NO];
    let _: () = msg_send![obsidian_caption, setBordered: NO];
    let _: () = msg_send![obsidian_caption, setFont: caption_font];
    let _: () = msg_send![obsidian_caption, setTextColor: caption_color];
    let _: () = msg_send![
        obsidian_caption,
        setStringValue: NSString::alloc(nil).init_str("Choose a vault so notes show up as first-class search results with note-specific actions.")
    ];

    let obsidian_enabled_toggle: id = msg_send![class!(NSButton), alloc];
    let obsidian_enabled_toggle: id = msg_send![obsidian_enabled_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, 224.0), NSSize::new(card_width - card_inset * 2.0, 28.0))];
    style_toggle(obsidian_enabled_toggle, "Enable Obsidian note search");
    let _: () = msg_send![obsidian_enabled_toggle, setTarget: target];
    let _: () = msg_send![obsidian_enabled_toggle, setAction: sel!(toggleSetting:)];

    let obsidian_open_toggle: id = msg_send![class!(NSButton), alloc];
    let obsidian_open_toggle: id = msg_send![obsidian_open_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, 188.0), NSSize::new(card_width - card_inset * 2.0, 28.0))];
    style_toggle(
        obsidian_open_toggle,
        "Open note results in Obsidian when possible",
    );
    let _: () = msg_send![obsidian_open_toggle, setTarget: target];
    let _: () = msg_send![obsidian_open_toggle, setAction: sel!(toggleSetting:)];

    let obsidian_path_label: id = msg_send![class!(NSTextField), alloc];
    let obsidian_path_label: id = msg_send![obsidian_path_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 152.0), NSSize::new(card_width - card_inset * 2.0, 20.0))];
    let _: () = msg_send![obsidian_path_label, setBezeled: NO];
    let _: () = msg_send![obsidian_path_label, setEditable: NO];
    let _: () = msg_send![obsidian_path_label, setDrawsBackground: NO];
    let _: () = msg_send![obsidian_path_label, setBordered: NO];
    let _: () = msg_send![obsidian_path_label, setFont: caption_font];
    let _: () = msg_send![obsidian_path_label, setTextColor: caption_color];
    let _: () = msg_send![obsidian_path_label, setStringValue: NSString::alloc(nil).init_str("Vault folder")];

    let obsidian_path_field_width = card_width - card_inset * 2.0 - 214.0;
    let obsidian_vault_path_field: id = msg_send![class!(NSTextField), alloc];
    let obsidian_vault_path_field: id = msg_send![obsidian_vault_path_field, initWithFrame:NSRect::new(NSPoint::new(card_inset, 116.0), NSSize::new(obsidian_path_field_width, 30.0))];
    style_input(obsidian_vault_path_field, true, true, false);

    let choose_vault_button: id = msg_send![class!(NSButton), alloc];
    let choose_vault_button: id = msg_send![choose_vault_button, initWithFrame:NSRect::new(NSPoint::new(card_width - card_inset - 202.0, 116.0), NSSize::new(96.0, 30.0))];
    style_action_button(choose_vault_button, "Choose", false);
    let _: () = msg_send![choose_vault_button, setTarget: target];
    let _: () = msg_send![choose_vault_button, setAction: sel!(chooseObsidianVault:)];

    let clear_vault_button: id = msg_send![class!(NSButton), alloc];
    let clear_vault_button: id = msg_send![clear_vault_button, initWithFrame:NSRect::new(NSPoint::new(card_width - card_inset - 98.0, 116.0), NSSize::new(96.0, 30.0))];
    style_action_button(clear_vault_button, "Clear", false);
    let _: () = msg_send![clear_vault_button, setTarget: target];
    let _: () = msg_send![clear_vault_button, setAction: sel!(clearObsidianVault:)];

    let obsidian_name_label: id = msg_send![class!(NSTextField), alloc];
    let obsidian_name_label: id = msg_send![obsidian_name_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 82.0), NSSize::new(card_width - card_inset * 2.0, 20.0))];
    let _: () = msg_send![obsidian_name_label, setBezeled: NO];
    let _: () = msg_send![obsidian_name_label, setEditable: NO];
    let _: () = msg_send![obsidian_name_label, setDrawsBackground: NO];
    let _: () = msg_send![obsidian_name_label, setBordered: NO];
    let _: () = msg_send![obsidian_name_label, setFont: caption_font];
    let _: () = msg_send![obsidian_name_label, setTextColor: caption_color];
    let _: () = msg_send![
        obsidian_name_label,
        setStringValue: NSString::alloc(nil).init_str("Vault name override (optional)")
    ];

    let obsidian_vault_name_field: id = msg_send![class!(NSTextField), alloc];
    let obsidian_vault_name_field: id = msg_send![obsidian_vault_name_field, initWithFrame:NSRect::new(NSPoint::new(card_inset, 46.0), NSSize::new(card_width - card_inset * 2.0, 30.0))];
    style_input(obsidian_vault_name_field, true, true, false);

    let obsidian_status_label: id = msg_send![class!(NSTextField), alloc];
    let obsidian_status_label: id = msg_send![obsidian_status_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 10.0), NSSize::new(card_width - card_inset * 2.0, 30.0))];
    let _: () = msg_send![obsidian_status_label, setBezeled: NO];
    let _: () = msg_send![obsidian_status_label, setEditable: NO];
    let _: () = msg_send![obsidian_status_label, setDrawsBackground: NO];
    let _: () = msg_send![obsidian_status_label, setBordered: NO];
    let _: () = msg_send![obsidian_status_label, setSelectable: NO];
    let _: () = msg_send![obsidian_status_label, setUsesSingleLineMode: NO];
    let _: () = msg_send![obsidian_status_label, setLineBreakMode: 4];
    let _: () = msg_send![obsidian_status_label, setFont: caption_font];
    let _: () = msg_send![obsidian_status_label, setTextColor: color_rgb(0.51, 0.76, 1.0)];

    let _: () = msg_send![obsidian_card, addSubview: obsidian_heading];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_caption];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_enabled_toggle];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_open_toggle];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_path_label];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_vault_path_field];
    let _: () = msg_send![obsidian_card, addSubview: choose_vault_button];
    let _: () = msg_send![obsidian_card, addSubview: clear_vault_button];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_name_label];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_vault_name_field];
    let _: () = msg_send![obsidian_card, addSubview: obsidian_status_label];

    // Sync card
    let sync_card_height = 466.0;
    let sync_card_y = (card_top - sync_card_height).max(card_margin);
    let sync_card: id = msg_send![class!(NSView), alloc];
    let sync_card: id = msg_send![sync_card, initWithFrame:NSRect::new(NSPoint::new(card_margin, sync_card_y), NSSize::new(card_width, sync_card_height))];
    let _: () = msg_send![sync_card, setWantsLayer: YES];
    let sync_layer: id = msg_send![sync_card, layer];
    let sync_bg: id = msg_send![class!(NSColor), colorWithCalibratedWhite:0.1f64 alpha:0.84f64];
    let sync_bg_cg: id = msg_send![sync_bg, CGColor];
    let _: () = msg_send![sync_layer, setCornerRadius: SETTINGS_CARD_RADIUS];
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
    let sync_column_gap = 24.0;
    let right_column_width = SETTINGS_STATUS_PANEL_WIDTH;
    let right_column_x = card_width - card_inset - right_column_width;
    let left_column_width = right_column_x - card_inset - sync_column_gap;
    let field_width = left_column_width - left_label_width - 12.0;

    let sync_enabled_toggle: id = msg_send![class!(NSButton), alloc];
    let sync_enabled_toggle: id = msg_send![sync_enabled_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, 356.0), NSSize::new(160.0, 28.0))];
    style_toggle(sync_enabled_toggle, "Enable sync");
    let _: () = msg_send![sync_enabled_toggle, setTarget: target];
    let _: () = msg_send![sync_enabled_toggle, setAction: sel!(toggleSetting:)];

    let sync_mirror_toggle: id = msg_send![class!(NSButton), alloc];
    let sync_mirror_toggle: id = msg_send![sync_mirror_toggle, initWithFrame:NSRect::new(NSPoint::new(card_inset, 320.0), NSSize::new(left_column_width, 28.0))];
    style_toggle(
        sync_mirror_toggle,
        "Mirror latest synced item to this clipboard",
    );
    let _: () = msg_send![sync_mirror_toggle, setTarget: target];
    let _: () = msg_send![sync_mirror_toggle, setAction: sel!(toggleSetting:)];

    let refresh_button: id = msg_send![class!(NSButton), alloc];
    let refresh_button: id = msg_send![refresh_button, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 350.0), NSSize::new(right_column_width, 30.0))];
    style_action_button(refresh_button, "Refresh status", false);
    let _: () = msg_send![refresh_button, setTarget: target];
    let _: () = msg_send![refresh_button, setAction: sel!(refreshSyncStatus:)];

    let test_button: id = msg_send![class!(NSButton), alloc];
    let test_button: id = msg_send![test_button, initWithFrame:NSRect::new(NSPoint::new(right_column_x, 312.0), NSSize::new(right_column_width, 30.0))];
    style_action_button(test_button, "Test connection", false);
    let _: () = msg_send![test_button, setTarget: target];
    let _: () = msg_send![test_button, setAction: sel!(testSyncConnection:)];

    let device_name_label: id = msg_send![class!(NSTextField), alloc];
    let device_name_label: id = msg_send![device_name_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 278.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![device_name_label, setBezeled: NO];
    let _: () = msg_send![device_name_label, setEditable: NO];
    let _: () = msg_send![device_name_label, setDrawsBackground: NO];
    let _: () = msg_send![device_name_label, setBordered: NO];
    let _: () = msg_send![device_name_label, setFont: caption_font];
    let _: () = msg_send![device_name_label, setTextColor: caption_color];
    set_string(device_name_label, "Device");

    let sync_device_name_field: id = msg_send![class!(NSTextField), alloc];
    let sync_device_name_field: id = msg_send![sync_device_name_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 272.0), NSSize::new(field_width, 30.0))];
    style_input(sync_device_name_field, true, true, false);

    let server_url_label: id = msg_send![class!(NSTextField), alloc];
    let server_url_label: id = msg_send![server_url_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 230.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![server_url_label, setBezeled: NO];
    let _: () = msg_send![server_url_label, setEditable: NO];
    let _: () = msg_send![server_url_label, setDrawsBackground: NO];
    let _: () = msg_send![server_url_label, setBordered: NO];
    let _: () = msg_send![server_url_label, setFont: caption_font];
    let _: () = msg_send![server_url_label, setTextColor: caption_color];
    set_string(server_url_label, "Server");

    let sync_server_url_field: id = msg_send![class!(NSTextField), alloc];
    let sync_server_url_field: id = msg_send![sync_server_url_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 224.0), NSSize::new(field_width, 30.0))];
    style_input(sync_server_url_field, true, true, false);

    let auth_token_label: id = msg_send![class!(NSTextField), alloc];
    let auth_token_label: id = msg_send![auth_token_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 182.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![auth_token_label, setBezeled: NO];
    let _: () = msg_send![auth_token_label, setEditable: NO];
    let _: () = msg_send![auth_token_label, setDrawsBackground: NO];
    let _: () = msg_send![auth_token_label, setBordered: NO];
    let _: () = msg_send![auth_token_label, setFont: caption_font];
    let _: () = msg_send![auth_token_label, setTextColor: caption_color];
    set_string(auth_token_label, "Token");

    let sync_auth_token_field: id = msg_send![class!(NSSecureTextField), alloc];
    let sync_auth_token_field: id = msg_send![sync_auth_token_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 176.0), NSSize::new(field_width, 30.0))];
    style_input(sync_auth_token_field, true, true, false);

    let device_id_label: id = msg_send![class!(NSTextField), alloc];
    let device_id_label: id = msg_send![device_id_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 138.0), NSSize::new(left_label_width, 22.0))];
    let _: () = msg_send![device_id_label, setBezeled: NO];
    let _: () = msg_send![device_id_label, setEditable: NO];
    let _: () = msg_send![device_id_label, setDrawsBackground: NO];
    let _: () = msg_send![device_id_label, setBordered: NO];
    let _: () = msg_send![device_id_label, setFont: caption_font];
    let _: () = msg_send![device_id_label, setTextColor: caption_color];
    set_string(device_id_label, "Device ID");

    let sync_status_panel: id = msg_send![class!(NSView), alloc];
    let sync_status_panel: id = msg_send![
        sync_status_panel,
        initWithFrame: NSRect::new(
            NSPoint::new(right_column_x, 136.0),
            NSSize::new(right_column_width, 124.0)
        )
    ];
    apply_settings_surface(sync_status_panel, settings_surface(0.7), 18.0, 0.08);

    let sync_status_heading: id = msg_send![class!(NSTextField), alloc];
    let sync_status_heading: id = msg_send![
        sync_status_heading,
        initWithFrame: NSRect::new(
            NSPoint::new(16.0, 90.0),
            NSSize::new(right_column_width - 32.0, 16.0)
        )
    ];
    let _: () = msg_send![sync_status_heading, setBezeled: NO];
    let _: () = msg_send![sync_status_heading, setEditable: NO];
    let _: () = msg_send![sync_status_heading, setDrawsBackground: NO];
    let _: () = msg_send![sync_status_heading, setBordered: NO];
    let _: () = msg_send![sync_status_heading, setSelectable: NO];
    let _: () = msg_send![sync_status_heading, setFont: caption_font];
    let _: () = msg_send![sync_status_heading, setTextColor: caption_color];
    set_string(sync_status_heading, "Connection status");

    let sync_status_label: id = msg_send![class!(NSTextField), alloc];
    let sync_status_label: id = msg_send![
        sync_status_label,
        initWithFrame: NSRect::new(
            NSPoint::new(34.0, 18.0),
            NSSize::new(right_column_width - 50.0, 74.0)
        )
    ];
    let _: () = msg_send![sync_status_label, setBezeled: NO];
    let _: () = msg_send![sync_status_label, setEditable: NO];
    let _: () = msg_send![sync_status_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_status_label, setBordered: NO];
    let _: () = msg_send![sync_status_label, setSelectable: NO];
    let _: () = msg_send![sync_status_label, setUsesSingleLineMode: NO];
    let _: () = msg_send![sync_status_label, setLineBreakMode: 4];
    let status_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:12.0 weight:0.28];
    let _: () = msg_send![sync_status_label, setFont: status_font];
    let _: () = msg_send![sync_status_label, setTextColor: settings_white(0.74)];

    let sync_device_id_field: id = msg_send![class!(NSTextField), alloc];
    let sync_device_id_field: id = msg_send![sync_device_id_field, initWithFrame:NSRect::new(NSPoint::new(card_inset + left_label_width + 12.0, 132.0), NSSize::new(field_width, 30.0))];
    style_input(sync_device_id_field, false, true, true);

    let sync_indicator_label: id = msg_send![class!(NSTextField), alloc];
    let sync_indicator_label: id = msg_send![
        sync_indicator_label,
        initWithFrame: NSRect::new(NSPoint::new(14.0, 56.0), NSSize::new(14.0, 18.0))
    ];
    let _: () = msg_send![sync_indicator_label, setBezeled: NO];
    let _: () = msg_send![sync_indicator_label, setEditable: NO];
    let _: () = msg_send![sync_indicator_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_indicator_label, setBordered: NO];
    let _: () = msg_send![sync_indicator_label, setSelectable: NO];
    let indicator_font: id = msg_send![class!(NSFont), boldSystemFontOfSize:14.0];
    let _: () = msg_send![sync_indicator_label, setFont: indicator_font];
    let _: () = msg_send![sync_indicator_label, setAlignment: 1];
    let _: () = msg_send![sync_indicator_label, setStringValue: NSString::alloc(nil).init_str("●")];

    let sync_message_label: id = msg_send![class!(NSTextField), alloc];
    let sync_message_label: id = msg_send![
        sync_message_label,
        initWithFrame: NSRect::new(
            NSPoint::new(right_column_x, 94.0),
            NSSize::new(right_column_width, 34.0)
        )
    ];
    let _: () = msg_send![sync_message_label, setBezeled: NO];
    let _: () = msg_send![sync_message_label, setEditable: NO];
    let _: () = msg_send![sync_message_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_message_label, setBordered: NO];
    let _: () = msg_send![sync_message_label, setSelectable: NO];
    let _: () = msg_send![sync_message_label, setUsesSingleLineMode: NO];
    let _: () = msg_send![sync_message_label, setLineBreakMode: 4];
    let _: () = msg_send![sync_message_label, setFont: caption_font];
    let message_color: id = msg_send![class!(NSColor), colorWithCalibratedRed:0.51f64 green:0.76f64 blue:1.0f64 alpha:1.0f64];
    let _: () = msg_send![sync_message_label, setTextColor: message_color];

    let devices_heading: id = msg_send![class!(NSTextField), alloc];
    let devices_heading: id = msg_send![devices_heading, initWithFrame:NSRect::new(NSPoint::new(card_inset, 108.0), NSSize::new(left_column_width, 20.0))];
    let _: () = msg_send![devices_heading, setBezeled: NO];
    let _: () = msg_send![devices_heading, setEditable: NO];
    let _: () = msg_send![devices_heading, setDrawsBackground: NO];
    let _: () = msg_send![devices_heading, setBordered: NO];
    let _: () = msg_send![devices_heading, setFont: general_font];
    let _: () = msg_send![devices_heading, setTextColor: heading_color];
    let _: () =
        msg_send![devices_heading, setStringValue: NSString::alloc(nil).init_str("Devices")];

    let sync_devices_label: id = msg_send![class!(NSTextField), alloc];
    let sync_devices_label: id = msg_send![sync_devices_label, initWithFrame:NSRect::new(NSPoint::new(card_inset, 22.0), NSSize::new(left_column_width, 82.0))];
    let _: () = msg_send![sync_devices_label, setBezeled: NO];
    let _: () = msg_send![sync_devices_label, setEditable: NO];
    let _: () = msg_send![sync_devices_label, setDrawsBackground: NO];
    let _: () = msg_send![sync_devices_label, setBordered: NO];
    style_info_block(sync_devices_label, false);

    let _: () = msg_send![sync_card, addSubview: sync_heading];
    let _: () = msg_send![sync_card, addSubview: sync_caption];
    let _: () = msg_send![sync_card, addSubview: sync_enabled_toggle];
    let _: () = msg_send![sync_card, addSubview: sync_mirror_toggle];
    let _: () = msg_send![sync_card, addSubview: refresh_button];
    let _: () = msg_send![sync_card, addSubview: test_button];
    let _: () = msg_send![sync_card, addSubview: device_name_label];
    let _: () = msg_send![sync_card, addSubview: sync_device_name_field];
    let _: () = msg_send![sync_card, addSubview: server_url_label];
    let _: () = msg_send![sync_card, addSubview: sync_server_url_field];
    let _: () = msg_send![sync_card, addSubview: auth_token_label];
    let _: () = msg_send![sync_card, addSubview: sync_auth_token_field];
    let _: () = msg_send![sync_card, addSubview: device_id_label];
    let _: () = msg_send![sync_status_panel, addSubview: sync_status_heading];
    let _: () = msg_send![sync_status_panel, addSubview: sync_indicator_label];
    let _: () = msg_send![sync_status_panel, addSubview: sync_status_label];
    let _: () = msg_send![sync_card, addSubview: sync_status_panel];
    let _: () = msg_send![sync_card, addSubview: sync_device_id_field];
    let _: () = msg_send![sync_card, addSubview: sync_message_label];
    let _: () = msg_send![sync_card, addSubview: devices_heading];
    let _: () = msg_send![sync_card, addSubview: sync_devices_label];

    let footer_height = 58.0;
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
    let divider_layer: id = msg_send![class!(CALayer), layer];
    let divider_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.05f64];
    let divider_color_cg: id = msg_send![divider_color, CGColor];
    let _: () = msg_send![divider_layer, setBackgroundColor: divider_color_cg];
    let _: () = msg_send![divider_layer, setFrame:NSRect::new(NSPoint::new(0.0, footer_height - 1.0), NSSize::new(bounds.size.width, 1.0))];
    let _: () = msg_send![footer_layer, addSublayer: divider_layer];

    let footer_hint: id = msg_send![class!(NSTextField), alloc];
    let footer_hint: id = msg_send![footer_hint, initWithFrame:NSRect::new(NSPoint::new(card_margin, 12.0), NSSize::new(420.0, 20.0))];
    let _: () = msg_send![footer_hint, setBezeled: NO];
    let _: () = msg_send![footer_hint, setEditable: NO];
    let _: () = msg_send![footer_hint, setDrawsBackground: NO];
    let _: () = msg_send![footer_hint, setBordered: NO];
    let footer_font: id = msg_send![class!(NSFont), monospacedSystemFontOfSize:11.0 weight:0.2];
    let _: () = msg_send![footer_hint, setFont: footer_font];
    let footer_hint_text =
        NSString::alloc(nil).init_str("Cmd+, reopens settings  •  Save applies your changes");
    let _: () = msg_send![footer_hint, setStringValue: footer_hint_text];
    let footer_hint_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.45f64];
    let _: () = msg_send![footer_hint, setTextColor: footer_hint_color];
    let _: () = msg_send![footer, addSubview: footer_hint];

    let save_button_frame = NSRect::new(
        NSPoint::new(bounds.size.width - card_margin - 160.0, 13.0),
        NSSize::new(150.0, SETTINGS_BUTTON_HEIGHT),
    );
    let save_button: id = msg_send![class!(NSButton), alloc];
    let save_button: id = msg_send![save_button, initWithFrame: save_button_frame];
    style_action_button(save_button, "Save settings", true);
    let _: () = msg_send![save_button, setTarget: target];
    let _: () = msg_send![save_button, setAction: sel!(saveSettings:)];

    let button_width = 150.0;
    let button_height = SETTINGS_BUTTON_HEIGHT;
    let button_frame = NSRect::new(
        NSPoint::new(bounds.size.width - button_width * 2.0 - 44.0, 13.0),
        NSSize::new(button_width, button_height),
    );
    let button: id = msg_send![class!(NSButton), alloc];
    let button: id = msg_send![button, initWithFrame: button_frame];
    style_action_button(button, "Back to search", false);
    let _: () = msg_send![button, setTarget: target];
    let _: () = msg_send![button, setAction: sel!(closeSettingsPanel:)];

    let _: () = msg_send![panel, addSubview: title];
    let _: () = msg_send![panel, addSubview: detail];
    let _: () = msg_send![panel, addSubview: tab_shell];
    let _: () = msg_send![panel, addSubview: general_card];
    let _: () = msg_send![panel, addSubview: behavior_card];
    let _: () = msg_send![panel, addSubview: obsidian_card];
    let _: () = msg_send![panel, addSubview: sync_card];
    let _: () = msg_send![footer, addSubview: button];
    let _: () = msg_send![footer, addSubview: save_button];
    let _: () = msg_send![panel, addSubview: footer];
    let controls = SettingsControls {
        tab_control: tab_control as usize,
        general_card: general_card as usize,
        behavior_card: behavior_card as usize,
        obsidian_card: obsidian_card as usize,
        sync_card: sync_card as usize,
        hotkey_field: hotkey_field as usize,
        max_slider: max_slider as usize,
        max_label: max_label as usize,
        toggle_paste_after_restore: paste_toggle as usize,
        toggle_escape: esc_toggle as usize,
        toggle_click: click_toggle as usize,
        obsidian_enabled_toggle: obsidian_enabled_toggle as usize,
        obsidian_open_in_obsidian_toggle: obsidian_open_toggle as usize,
        obsidian_vault_path_field: obsidian_vault_path_field as usize,
        obsidian_vault_name_field: obsidian_vault_name_field as usize,
        obsidian_status_label: obsidian_status_label as usize,
        sync_enabled_toggle: sync_enabled_toggle as usize,
        sync_mirror_clipboard_toggle: sync_mirror_toggle as usize,
        sync_device_name_field: sync_device_name_field as usize,
        sync_device_id_field: sync_device_id_field as usize,
        sync_server_url_field: sync_server_url_field as usize,
        sync_auth_token_field: sync_auth_token_field as usize,
        sync_indicator_label: sync_indicator_label as usize,
        sync_status_label: sync_status_label as usize,
        sync_devices_label: sync_devices_label as usize,
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
                let paste_state: i64 = if settings.paste_after_restore { 1 } else { 0 };
                let esc_state: i64 = if settings.dismiss_on_escape { 1 } else { 0 };
                let click_state: i64 = if settings.dismiss_on_click_away { 1 } else { 0 };
                let _: () = msg_send![
                    id_from(controls.toggle_paste_after_restore),
                    setState: paste_state
                ];
                let _: () = msg_send![id_from(controls.toggle_escape), setState: esc_state];
                let _: () = msg_send![id_from(controls.toggle_click), setState: click_state];
                let obsidian_enabled_state: i64 = if settings.obsidian.enabled { 1 } else { 0 };
                let obsidian_open_state: i64 = if settings.obsidian.open_in_obsidian {
                    1
                } else {
                    0
                };
                let _: () = msg_send![
                    id_from(controls.obsidian_enabled_toggle),
                    setState: obsidian_enabled_state
                ];
                let _: () = msg_send![
                    id_from(controls.obsidian_open_in_obsidian_toggle),
                    setState: obsidian_open_state
                ];
                set_string(
                    id_from(controls.obsidian_vault_path_field),
                    settings.obsidian.vault_path.as_deref().unwrap_or(""),
                );
                set_string(
                    id_from(controls.obsidian_vault_name_field),
                    settings.obsidian.vault_name.as_deref().unwrap_or(""),
                );
                set_obsidian_message(if settings.obsidian.enabled {
                    "Obsidian note search is enabled. Save after changing the configured vault."
                } else {
                    "Choose a vault folder to enable Obsidian note search."
                });
                let sync_enabled_state: i64 = if settings.sync.enabled { 1 } else { 0 };
                let sync_mirror_state: i64 = if settings.sync.mirror_clipboard { 1 } else { 0 };
                let _: () =
                    msg_send![id_from(controls.sync_enabled_toggle), setState: sync_enabled_state];
                let _: () = msg_send![
                    id_from(controls.sync_mirror_clipboard_toggle),
                    setState: sync_mirror_state
                ];
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
                    known_devices: Vec::new(),
                });
                set_string(id_from(controls.sync_status_label), &summary);
                set_sync_devices_summary(&[]);
                set_sync_indicator(
                    if settings.sync.enabled {
                        sync::SyncConnectionState::Disconnected
                    } else {
                        sync::SyncConnectionState::Disabled
                    },
                    None,
                );
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

unsafe fn apply_active_settings_tab() {
    let Some(controls) = SETTINGS_CONTROLS.get() else {
        return;
    };

    let active = SETTINGS_ACTIVE_TAB.load(Ordering::SeqCst);
    let _: () = msg_send![id_from(controls.tab_control), setSelectedSegment: active as isize];
    let _: () = msg_send![
        id_from(controls.general_card),
        setHidden: if active == SETTINGS_TAB_GENERAL { NO } else { YES }
    ];
    let _: () = msg_send![
        id_from(controls.behavior_card),
        setHidden: if active == SETTINGS_TAB_BEHAVIOR { NO } else { YES }
    ];
    let _: () = msg_send![
        id_from(controls.obsidian_card),
        setHidden: if active == SETTINGS_TAB_OBSIDIAN { NO } else { YES }
    ];
    let _: () = msg_send![
        id_from(controls.sync_card),
        setHidden: if active == SETTINGS_TAB_SYNC { NO } else { YES }
    ];
}

pub unsafe fn focus_active_settings_control(window: id) {
    let Some(controls) = SETTINGS_CONTROLS.get() else {
        return;
    };

    let first_responder = match SETTINGS_ACTIVE_TAB.load(Ordering::SeqCst) {
        SETTINGS_TAB_GENERAL => id_from(controls.hotkey_field),
        SETTINGS_TAB_BEHAVIOR => id_from(controls.max_slider),
        SETTINGS_TAB_OBSIDIAN => id_from(controls.obsidian_vault_path_field),
        SETTINGS_TAB_SYNC => id_from(controls.sync_device_name_field),
        _ => id_from(controls.tab_control),
    };

    if first_responder != nil {
        let _: () = msg_send![window, makeFirstResponder: first_responder];
    }
}

unsafe fn apply_settings_from_ui() {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        let mut current_settings =
            settings::load().unwrap_or_else(|_| settings::Settings::default());
        let hotkey = get_string(id_from(controls.hotkey_field));
        if hotkey.trim().is_empty() {
            set_sync_message("Hotkey cannot be empty.");
            return;
        }
        let slider_value: i32 = msg_send![id_from(controls.max_slider), intValue];
        let slider_value = slider_value.clamp(10, 200);
        let paste_state: i16 = msg_send![id_from(controls.toggle_paste_after_restore), state];
        let esc_state: i16 = msg_send![id_from(controls.toggle_escape), state];
        let click_state: i16 = msg_send![id_from(controls.toggle_click), state];
        let obsidian_enabled_state: i16 =
            msg_send![id_from(controls.obsidian_enabled_toggle), state];
        let obsidian_open_state: i16 =
            msg_send![id_from(controls.obsidian_open_in_obsidian_toggle), state];
        let sync_enabled_state: i16 = msg_send![id_from(controls.sync_enabled_toggle), state];
        let sync_mirror_state: i16 =
            msg_send![id_from(controls.sync_mirror_clipboard_toggle), state];
        let obsidian_enabled = obsidian_enabled_state == 1;
        let obsidian_vault_path = get_string(id_from(controls.obsidian_vault_path_field));
        let obsidian_vault_name = get_string(id_from(controls.obsidian_vault_name_field));
        let sync_enabled = sync_enabled_state == 1;
        let sync_device_name = get_string(id_from(controls.sync_device_name_field));
        let sync_server_url_input = get_string(id_from(controls.sync_server_url_field));
        let sync_auth_token = get_string(id_from(controls.sync_auth_token_field));
        let prepared_obsidian = match settings::prepare_obsidian_settings(
            obsidian_enabled,
            &obsidian_vault_path,
            &obsidian_vault_name,
        ) {
            Ok(prepared) => prepared,
            Err(err) => {
                set_obsidian_message(&format!("{err:#}"));
                return;
            }
        };
        let old_enabled = current_settings.sync.enabled;
        let old_server_url = current_settings.sync.server_url.clone().unwrap_or_default();
        let old_auth_token = current_settings.sync.auth_token.clone().unwrap_or_default();
        let prepared_sync = match settings::prepare_sync_settings(
            sync_enabled,
            &sync_device_name,
            &sync_server_url_input,
            &sync_auth_token,
        ) {
            Ok(prepared) => prepared,
            Err(err) => {
                set_sync_message(&format!("{err:#}"));
                return;
            }
        };
        current_settings.hotkey = hotkey;
        current_settings.max_results = slider_value as usize;
        current_settings.paste_after_restore = paste_state == 1;
        current_settings.dismiss_on_escape = esc_state == 1;
        current_settings.dismiss_on_click_away = click_state == 1;
        current_settings.obsidian.enabled = obsidian_enabled;
        current_settings.obsidian.vault_path = prepared_obsidian.vault_path;
        current_settings.obsidian.vault_name = prepared_obsidian.vault_name;
        current_settings.obsidian.open_in_obsidian = obsidian_open_state == 1;
        current_settings.sync.enabled = sync_enabled;
        current_settings.sync.mirror_clipboard = sync_mirror_state == 1;
        current_settings.sync.device_name = prepared_sync.device_name;
        current_settings.sync.server_url = prepared_sync.server_url;
        current_settings.sync.auth_token = prepared_sync.auth_token;
        if let Err(err) = settings::save(&current_settings) {
            set_sync_message(&format!("Failed to save settings: {err:#}"));
            eprintln!("Failed to save settings: {}", err);
            return;
        }
        current_settings = match settings::load() {
            Ok(settings) => settings,
            Err(err) => {
                set_sync_message(&format!(
                    "Settings were written, but reloading them failed: {err:#}"
                ));
                return;
            }
        };
        if let Ok(mut esc_guard) = DISMISS_ON_ESCAPE.lock() {
            *esc_guard = current_settings.dismiss_on_escape;
        }
        if let Ok(mut click_guard) = DISMISS_ON_CLICK_AWAY.lock() {
            *click_guard = current_settings.dismiss_on_click_away;
        }
        set_string(id_from(controls.hotkey_field), &current_settings.hotkey);
        let slider: id = id_from(controls.max_slider);
        let _: () = msg_send![slider, setIntValue: current_settings.max_results as i32];
        set_string(
            id_from(controls.sync_server_url_field),
            current_settings.sync.server_url.as_deref().unwrap_or(""),
        );
        set_string(
            id_from(controls.sync_device_name_field),
            &current_settings.sync.device_name,
        );
        set_string(
            id_from(controls.sync_auth_token_field),
            current_settings.sync.auth_token.as_deref().unwrap_or(""),
        );
        set_string(
            id_from(controls.obsidian_vault_path_field),
            current_settings
                .obsidian
                .vault_path
                .as_deref()
                .unwrap_or(""),
        );
        set_string(
            id_from(controls.obsidian_vault_name_field),
            current_settings
                .obsidian
                .vault_name
                .as_deref()
                .unwrap_or(""),
        );
        let obsidian_enabled_state: i64 = if current_settings.obsidian.enabled {
            1
        } else {
            0
        };
        let obsidian_open_state: i64 = if current_settings.obsidian.open_in_obsidian {
            1
        } else {
            0
        };
        let _: () = msg_send![
            id_from(controls.obsidian_enabled_toggle),
            setState: obsidian_enabled_state
        ];
        let _: () = msg_send![
            id_from(controls.obsidian_open_in_obsidian_toggle),
            setState: obsidian_open_state
        ];
        set_obsidian_message(if current_settings.obsidian.enabled {
            "Obsidian settings saved."
        } else {
            "Obsidian note search is off until you enable it again."
        });
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
            "Sync settings saved. The background worker is reconnecting with the updated server details."
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
        if sync_enabled {
            match Runtime::new() {
                Ok(runtime) => {
                    let result = runtime.block_on(sync::test_connection(
                        current_settings.sync.server_url.as_deref().unwrap_or(""),
                        current_settings.sync.auth_token.as_deref(),
                    ));
                    if let Some(normalized) = result.normalized_server_url.as_deref() {
                        set_string(id_from(controls.sync_server_url_field), normalized);
                    }
                    set_sync_indicator_from_test_result(&result);
                    if result.ok {
                        refresh_sync_status_controls(Some(&format!(
                            "Sync settings saved. {}",
                            result.message
                        )));
                    } else {
                        set_sync_message(&format!("Sync settings saved, but {}", result.message));
                    }
                }
                Err(err) => {
                    set_sync_message(&format!(
                        "{sync_message} Connection test could not run: {err:#}"
                    ));
                }
            }
        } else {
            set_sync_indicator(sync::SyncConnectionState::Disabled, None);
            set_sync_devices_summary(&[]);
            set_sync_message(sync_message);
        }
        slider_value_changed(id_from(controls.max_slider));
    }
}

unsafe fn refresh_sync_status_controls(message: Option<&str>) {
    let Some(controls) = SETTINGS_CONTROLS.get() else {
        return;
    };

    let status_result = match Runtime::new() {
        Ok(runtime) => runtime.block_on(sync::refresh_remote_status()),
        Err(err) => Err(anyhow::anyhow!("failed to create sync runtime: {err:#}")),
    };

    match status_result {
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
            set_sync_devices_summary(&[]);
            set_sync_indicator(sync::SyncConnectionState::Disconnected, None);
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
        let message_view = id_from(controls.sync_message_label);
        set_string(message_view, message);
        let _: () = msg_send![message_view, setTextColor: sync_message_color(message)];
    }
}

unsafe fn set_obsidian_message(message: &str) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        let message_view = id_from(controls.obsidian_status_label);
        set_string(message_view, message);
        let _: () = msg_send![message_view, setTextColor: sync_message_color(message)];
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

unsafe fn choose_obsidian_vault_folder() -> Option<String> {
    let panel: id = msg_send![class!(NSOpenPanel), openPanel];
    let _: () = msg_send![panel, setCanChooseFiles: NO];
    let _: () = msg_send![panel, setCanChooseDirectories: YES];
    let _: () = msg_send![panel, setAllowsMultipleSelection: NO];
    let _: () = msg_send![panel, setCanCreateDirectories: YES];
    let response: i64 = msg_send![panel, runModal];
    if response != 1 {
        return None;
    }

    let url: id = msg_send![panel, URL];
    if url == nil {
        return None;
    }

    let path: id = msg_send![url, path];
    if path == nil {
        return None;
    }

    let cstr: *const c_char = msg_send![path, UTF8String];
    if cstr.is_null() {
        return None;
    }

    Some(CStr::from_ptr(cstr).to_string_lossy().to_string())
}

unsafe fn set_sync_summary(status: &sync::SyncStatus) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        set_string(
            id_from(controls.sync_status_label),
            &build_sync_summary(status),
        );
        set_sync_devices_summary(&status.known_devices);
        set_sync_indicator(status.connection_state.clone(), None);
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

unsafe fn set_sync_devices_summary(devices: &[sync::KnownSyncDevice]) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        set_string(
            id_from(controls.sync_devices_label),
            &build_device_summary(devices),
        );
    }
}

unsafe fn set_sync_indicator(
    state: sync::SyncConnectionState,
    test_result: Option<&sync::SyncConnectionTestResult>,
) {
    if let Some(controls) = SETTINGS_CONTROLS.get() {
        let indicator = id_from(controls.sync_indicator_label);
        let _: () = msg_send![indicator, setStringValue: NSString::alloc(nil).init_str("●")];
        let _: () = msg_send![indicator, setTextColor: sync_indicator_color(state, test_result)];
    }
}

unsafe fn set_sync_indicator_from_test_result(result: &sync::SyncConnectionTestResult) {
    let state = if result.ok {
        sync::SyncConnectionState::Connected
    } else {
        match result.issue {
            sync::SyncConnectionTestIssue::None => sync::SyncConnectionState::Connected,
            sync::SyncConnectionTestIssue::ServerUnreachable => {
                sync::SyncConnectionState::Disconnected
            }
            sync::SyncConnectionTestIssue::AuthenticationFailed => {
                sync::SyncConnectionState::Disconnected
            }
            sync::SyncConnectionTestIssue::InvalidConfiguration => {
                sync::SyncConnectionState::Disconnected
            }
            sync::SyncConnectionTestIssue::UnexpectedResponse => {
                sync::SyncConnectionState::Reconnecting
            }
        }
    };
    set_sync_indicator(state, Some(result));
}

fn build_device_summary(devices: &[sync::KnownSyncDevice]) -> String {
    if devices.is_empty() {
        return "No device roster is cached yet. Use Test connection or Refresh status to load it."
            .to_string();
    }

    devices
        .iter()
        .take(4)
        .map(|device| {
            let current = if device.is_current {
                "  This device"
            } else {
                ""
            };
            format!(
                "{} ({}){}  Last seen {}",
                device.device_name,
                device.platform,
                current,
                sync::format_timestamp(Some(device.last_seen_at))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

unsafe fn sync_indicator_color(
    state: sync::SyncConnectionState,
    test_result: Option<&sync::SyncConnectionTestResult>,
) -> id {
    if let Some(result) = test_result {
        return if result.ok {
            color_rgb(0.37, 0.79, 0.50)
        } else {
            match result.issue {
                sync::SyncConnectionTestIssue::None => color_rgb(0.37, 0.79, 0.50),
                sync::SyncConnectionTestIssue::UnexpectedResponse => color_rgb(0.95, 0.75, 0.30),
                sync::SyncConnectionTestIssue::InvalidConfiguration
                | sync::SyncConnectionTestIssue::AuthenticationFailed
                | sync::SyncConnectionTestIssue::ServerUnreachable => color_rgb(1.0, 0.49, 0.49),
            }
        };
    }

    match state {
        sync::SyncConnectionState::Connected => color_rgb(0.37, 0.79, 0.50),
        sync::SyncConnectionState::Reconnecting => color_rgb(0.95, 0.75, 0.30),
        sync::SyncConnectionState::Disabled => color_rgb(0.60, 0.63, 0.70),
        sync::SyncConnectionState::Disconnected => color_rgb(1.0, 0.49, 0.49),
    }
}

unsafe fn color_rgb(red: f64, green: f64, blue: f64) -> id {
    msg_send![class!(NSColor), colorWithCalibratedRed:red green:green blue:blue alpha:1.0f64]
}

unsafe fn sync_message_color(message: &str) -> id {
    let lower = message.to_lowercase();
    if lower.contains("saved")
        || lower.contains("succeeded")
        || lower.contains("loaded")
        || lower.contains("enabled")
    {
        color_rgb(0.37, 0.79, 0.50)
    } else if lower.contains("unreachable") || lower.contains("reconnecting") {
        color_rgb(0.95, 0.75, 0.30)
    } else if lower.contains("failed")
        || lower.contains("invalid")
        || lower.contains("rejected")
        || lower.contains("cannot")
        || lower.contains("empty")
        || lower.contains("error")
    {
        color_rgb(1.0, 0.49, 0.49)
    } else {
        color_rgb(0.51, 0.76, 1.0)
    }
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

        extern "C" fn change_settings_tab(_this: &Object, _cmd: Sel, sender: id) {
            unsafe {
                let selected: isize = msg_send![sender, selectedSegment];
                let tab_index = selected.clamp(0, SETTINGS_TAB_SYNC as isize) as usize;
                SETTINGS_ACTIVE_TAB.store(tab_index, Ordering::SeqCst);
                apply_active_settings_tab();
            }
        }

        extern "C" fn choose_obsidian_vault_action(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                let Some(controls) = SETTINGS_CONTROLS.get() else {
                    return;
                };
                if let Some(folder) = choose_obsidian_vault_folder() {
                    set_string(id_from(controls.obsidian_vault_path_field), &folder);
                    set_obsidian_message("Selected an Obsidian vault folder. Save to apply it.");
                }
            }
        }

        extern "C" fn clear_obsidian_vault_action(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                let Some(controls) = SETTINGS_CONTROLS.get() else {
                    return;
                };
                set_string(id_from(controls.obsidian_vault_path_field), "");
                set_string(id_from(controls.obsidian_vault_name_field), "");
                let _: () = msg_send![id_from(controls.obsidian_enabled_toggle), setState: 0i64];
                set_obsidian_message("Cleared the configured Obsidian vault. Save to apply it.");
            }
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

        extern "C" fn test_sync_connection_action(_this: &Object, _cmd: Sel, _sender: id) {
            unsafe {
                let Some(controls) = SETTINGS_CONTROLS.get() else {
                    return;
                };
                let server_url = get_string(id_from(controls.sync_server_url_field));
                let auth_token = get_string(id_from(controls.sync_auth_token_field));
                let sync_enabled_state: i16 =
                    msg_send![id_from(controls.sync_enabled_toggle), state];
                match Runtime::new() {
                    Ok(runtime) => {
                        let auth_token = non_empty(auth_token.trim());
                        let result = runtime
                            .block_on(sync::test_connection(&server_url, auth_token.as_deref()));
                        if let Some(normalized) = result.normalized_server_url.as_deref() {
                            set_string(id_from(controls.sync_server_url_field), normalized);
                        }
                        set_sync_message(&result.message);
                        set_sync_indicator_from_test_result(&result);
                        if result.ok {
                            if sync_enabled_state == 1 {
                                refresh_sync_status_controls(Some("Connection test succeeded."));
                            } else if let Ok(status) = sync::status() {
                                set_string(
                                    id_from(controls.sync_device_id_field),
                                    &status.device.device_id,
                                );
                                set_sync_devices_summary(&status.known_devices);
                            }
                        }
                    }
                    Err(err) => {
                        set_sync_message(&format!(
                            "Connection test could not run because the sync runtime failed to start: {err:#}"
                        ));
                    }
                }
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
            sel!(changeSettingsTab:),
            change_settings_tab as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(chooseObsidianVault:),
            choose_obsidian_vault_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(clearObsidianVault:),
            clear_obsidian_vault_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(saveSettings:),
            save_settings_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(refreshSyncStatus:),
            refresh_sync_status_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(testSyncConnection:),
            test_sync_connection_action as extern "C" fn(&Object, Sel, id),
        );
        decl.register();
    }

    let cls = class!(MKSettingsPanelActions);
    let target: id = msg_send![cls, new];
    let _: () = msg_send![target, retain];
    target
}

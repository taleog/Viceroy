use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::OnceLock;

use crate::settings;
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
    let _: () = msg_send![detail, setStringValue: NSString::alloc(nil).init_str("Configure hotkey, clipboard history, and theme preferences from here.")];

    let target = ensure_actions_target();

    let section_x = 36.0;
    let mut current_y = height - 150.0;
    let label_width = 160.0;

    // Hotkey input
    let hotkey_label: id = msg_send![class!(NSTextField), alloc];
    let hotkey_label: id = msg_send![hotkey_label, initWithFrame:NSRect::new(NSPoint::new(section_x, current_y), NSSize::new(label_width, 18.0))];
    let _: () = msg_send![hotkey_label, setBezeled: NO];
    let _: () = msg_send![hotkey_label, setEditable: NO];
    let _: () = msg_send![hotkey_label, setDrawsBackground: NO];
    let _: () = msg_send![hotkey_label, setBordered: NO];
    let _: () =
        msg_send![hotkey_label, setStringValue: NSString::alloc(nil).init_str("Global hotkey")];
    let hotkey_field: id = msg_send![class!(NSTextField), alloc];
    let hotkey_field_frame = NSRect::new(
        NSPoint::new(section_x + label_width + 12.0, current_y - 4.0),
        NSSize::new(240.0, 22.0),
    );
    let hotkey_field: id = msg_send![hotkey_field, initWithFrame: hotkey_field_frame];
    let _: () = msg_send![hotkey_field, setBezeled: YES];
    let _: () = msg_send![hotkey_field, setEditable: YES];
    let _: () = msg_send![hotkey_field, setDrawsBackground: YES];
    let _: () = msg_send![hotkey_field, setBordered: YES];
    current_y -= 34.0;

    // Max results slider
    let max_label: id = msg_send![class!(NSTextField), alloc];
    let max_label: id = msg_send![max_label, initWithFrame:NSRect::new(NSPoint::new(section_x, current_y), NSSize::new(label_width + 80.0, 18.0))];
    let _: () = msg_send![max_label, setBezeled: NO];
    let _: () = msg_send![max_label, setEditable: NO];
    let _: () = msg_send![max_label, setDrawsBackground: NO];
    let _: () = msg_send![max_label, setBordered: NO];
    let _: () =
        msg_send![max_label, setStringValue: NSString::alloc(nil).init_str("Max results: 50")];
    let slider_frame = NSRect::new(
        NSPoint::new(section_x, current_y - 30.0),
        NSSize::new(bounds.size.width - section_x * 2.0 - 20.0, 22.0),
    );
    let max_slider: id = msg_send![class!(NSSlider), alloc];
    let max_slider: id = msg_send![max_slider, initWithFrame: slider_frame];
    let _: () = msg_send![max_slider, setMinValue: 10.0];
    let _: () = msg_send![max_slider, setMaxValue: 200.0];
    let _: () = msg_send![max_slider, setAllowsTickMarkValues: YES];
    let _: () = msg_send![max_slider, setNumberOfTickMarks: 10];
    let _: () = msg_send![max_slider, setTarget: target];
    let _: () = msg_send![max_slider, setAction: sel!(maxResultsSliderChanged:)];
    current_y -= 60.0;

    // Toggle rows
    let esc_toggle_frame =
        NSRect::new(NSPoint::new(section_x, current_y), NSSize::new(240.0, 28.0));
    let esc_toggle: id = msg_send![class!(NSButton), alloc];
    let esc_toggle: id = msg_send![esc_toggle, initWithFrame: esc_toggle_frame];
    let _: () = msg_send![esc_toggle, setButtonType: 3];
    let _: () = msg_send![esc_toggle, setTitle: NSString::alloc(nil).init_str("Dismiss on Escape")];
    let _: () = msg_send![esc_toggle, setTarget: target];
    let _: () = msg_send![esc_toggle, setAction: sel!(toggleSetting:)];
    current_y -= 36.0;
    let click_toggle_frame =
        NSRect::new(NSPoint::new(section_x, current_y), NSSize::new(240.0, 28.0));
    let click_toggle: id = msg_send![class!(NSButton), alloc];
    let click_toggle: id = msg_send![click_toggle, initWithFrame: click_toggle_frame];
    let _: () = msg_send![click_toggle, setButtonType: 3];
    let _: () =
        msg_send![click_toggle, setTitle: NSString::alloc(nil).init_str("Dismiss on click away")];
    let _: () = msg_send![click_toggle, setTarget: target];
    let _: () = msg_send![click_toggle, setAction: sel!(toggleSetting:)];
    current_y -= 48.0;

    let save_button_frame =
        NSRect::new(NSPoint::new(section_x, current_y), NSSize::new(140.0, 30.0));
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
    let _: () = msg_send![panel, addSubview: hotkey_label];
    let _: () = msg_send![panel, addSubview: hotkey_field];
    let _: () = msg_send![panel, addSubview: max_label];
    let _: () = msg_send![panel, addSubview: max_slider];
    let _: () = msg_send![panel, addSubview: esc_toggle];
    let _: () = msg_send![panel, addSubview: click_toggle];
    let _: () = msg_send![panel, addSubview: save_button];
    let _: () = msg_send![panel, addSubview: button];
    let controls = SettingsControls {
        hotkey_field: hotkey_field as usize,
        max_slider: max_slider as usize,
        max_label: max_label as usize,
        toggle_escape: esc_toggle as usize,
        toggle_click: click_toggle as usize,
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
        current_settings.hotkey = hotkey;
        current_settings.max_results = slider_value as usize;
        current_settings.dismiss_on_escape = esc_state == 1;
        current_settings.dismiss_on_click_away = click_state == 1;
        if let Err(err) = settings::save(&current_settings) {
            eprintln!("Failed to save settings: {}", err);
        }
        if let Ok(mut esc_guard) = DISMISS_ON_ESCAPE.lock() {
            *esc_guard = current_settings.dismiss_on_escape;
        }
        if let Ok(mut click_guard) = DISMISS_ON_CLICK_AWAY.lock() {
            *click_guard = current_settings.dismiss_on_click_away;
        }
        slider_value_changed(id_from(controls.max_slider));
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
        decl.register();
    }

    let cls = class!(MKSettingsPanelActions);
    let target: id = msg_send![cls, new];
    let _: () = msg_send![target, retain];
    target
}

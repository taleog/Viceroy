use crate::search_engine::SearchResult;
use crate::ui::clipboard_view::{
    id_from, preview_refs, reset_text_scroll_position, set_hidden, set_string,
};
use crate::ui::helpers::run_on_main;
use crate::ui::state::{TableMode, SEARCH_RT, TABLE_MODE, TABLE_RESULTS};
use crate::web_search::{self, LinkPreviewData, LinkTarget};
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::NSString;
use lazy_static::lazy_static;
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

lazy_static! {
    static ref LINK_PREVIEW_CACHE: Mutex<HashMap<String, LinkPreviewData>> =
        Mutex::new(HashMap::new());
    static ref ACTIVE_PREVIEW_URL: Mutex<Option<String>> = Mutex::new(None);
}

static SEARCH_PREVIEW_REQUEST_ID: AtomicU64 = AtomicU64::new(0);

pub fn search_preview_visible_for_selection(row: Option<usize>) -> bool {
    let Some(row) = row else {
        return false;
    };
    selected_result(row)
        .as_ref()
        .and_then(link_target_for_result)
        .is_some()
}

pub fn update_search_preview_selection(row: Option<usize>) {
    let is_search_mode = match TABLE_MODE.lock() {
        Ok(mode) => *mode == TableMode::Search,
        Err(_) => false,
    };
    if !is_search_mode {
        clear_active_preview_url();
        return;
    }

    let Some(row) = row else {
        clear_active_preview_url();
        return;
    };
    let Some(result) = selected_result(row) else {
        clear_active_preview_url();
        return;
    };
    let Some(target) = link_target_for_result(&result) else {
        clear_active_preview_url();
        return;
    };

    if let Ok(mut active) = ACTIVE_PREVIEW_URL.lock() {
        *active = Some(target.url.clone());
    }

    if let Some(cached) = cached_preview(&target.url) {
        unsafe {
            show_link_preview(&cached, false);
        }
        return;
    }

    unsafe {
        show_loading_preview(&target);
    }

    let request_id = SEARCH_PREVIEW_REQUEST_ID.fetch_add(1, Ordering::SeqCst) + 1;
    SEARCH_RT.spawn(async move {
        let preview = web_search::fetch_link_preview(&target).await;
        if let Ok(mut cache) = LINK_PREVIEW_CACHE.lock() {
            cache.insert(preview.url.clone(), preview.clone());
        }

        run_on_main(move || {
            let is_still_latest = SEARCH_PREVIEW_REQUEST_ID.load(Ordering::SeqCst) == request_id;
            let active_url = ACTIVE_PREVIEW_URL
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            let is_search_mode = TABLE_MODE
                .lock()
                .map(|mode| *mode == TableMode::Search)
                .unwrap_or(false);

            if is_still_latest && is_search_mode && active_url.as_deref() == Some(&preview.url) {
                unsafe {
                    show_link_preview(&preview, false);
                }
            }
        });
    });
}

fn selected_result(row: usize) -> Option<SearchResult> {
    let results = TABLE_RESULTS.lock().ok()?;
    results.get(row).cloned()
}

fn link_target_for_result(result: &SearchResult) -> Option<LinkTarget> {
    match result {
        SearchResult::Link {
            url,
            display_url,
            host,
        } => Some(LinkTarget {
            url: url.clone(),
            display_url: display_url.clone(),
            host: host.clone(),
        }),
        SearchResult::Clipboard {
            content,
            content_type,
            ..
        } if content_type == "text" => web_search::detect_direct_link(content),
        _ => None,
    }
}

fn cached_preview(url: &str) -> Option<LinkPreviewData> {
    LINK_PREVIEW_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(url).cloned())
}

fn clear_active_preview_url() {
    if let Ok(mut active) = ACTIVE_PREVIEW_URL.lock() {
        *active = None;
    }
}

unsafe fn show_loading_preview(target: &LinkTarget) {
    let preview = LinkPreviewData {
        url: target.url.clone(),
        display_url: target.display_url.clone(),
        host: target.host.clone(),
        title: Some(format!("Open {}", target.host)),
        description: Some("Fetching page title and description...".to_string()),
        site_name: None,
        image_url: None,
        icon_url: None,
    };
    show_link_preview(&preview, true);
}

unsafe fn show_link_preview(preview: &LinkPreviewData, loading: bool) {
    let Some(refs) = preview_refs() else {
        return;
    };

    let title_field = id_from(refs.title_field);
    let detail_field = id_from(refs.detail_field);
    let action_bar = id_from(refs.action_bar);
    let placeholder = id_from(refs.placeholder_field);
    let text_scroll = id_from(refs.text_scroll);
    let text_view = id_from(refs.text_view);
    let image_view = id_from(refs.image_view);
    let text_background = id_from(refs.text_background);

    if title_field == nil || detail_field == nil || text_scroll == nil || text_view == nil {
        return;
    }

    let title = preview
        .title
        .as_deref()
        .or(preview.site_name.as_deref())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&preview.host);
    let detail = if loading {
        format!("{}  •  Loading preview...", preview.display_url)
    } else {
        format!("{}  •  Press Enter to open", preview.display_url)
    };
    let body = preview_body(preview, loading);

    let title_font: id = msg_send![class!(NSFont), systemFontOfSize:17.0 weight:0.62];
    let detail_font: id = msg_send![class!(NSFont), systemFontOfSize:13.0 weight:0.0];
    let body_font: id = msg_send![class!(NSFont), systemFontOfSize:13.0];
    let text_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.92f64];
    let detail_color: id =
        msg_send![class!(NSColor), colorWithCalibratedWhite:1.0f64 alpha:0.72f64];

    let _: () = msg_send![title_field, setFont: title_font];
    let _: () = msg_send![title_field, setTextColor: text_color];
    let _: () = msg_send![detail_field, setFont: detail_font];
    let _: () = msg_send![detail_field, setTextColor: detail_color];
    let _: () = msg_send![text_view, setFont: body_font];
    let _: () = msg_send![text_view, setTextColor: text_color];
    let _: () = msg_send![text_view, setEditable: NO];
    let _: () = msg_send![text_view, setSelectable: YES];
    let _: () = msg_send![text_view, setString: NSString::alloc(nil).init_str(&body)];

    set_string(title_field, title);
    set_string(detail_field, &detail);
    set_hidden(title_field, false);
    set_hidden(detail_field, false);
    set_hidden(action_bar, true);
    set_hidden(placeholder, true);
    set_hidden(text_background, false);
    set_hidden(text_scroll, false);
    set_hidden(image_view, true);
    reset_text_scroll_position(text_scroll, text_view);
}

fn preview_body(preview: &LinkPreviewData, loading: bool) -> String {
    let mut body = String::new();

    if let Some(site_name) = preview
        .site_name
        .as_deref()
        .filter(|site_name| *site_name != preview.host)
    {
        let _ = writeln!(body, "{site_name}");
        let _ = writeln!(body);
    }

    if let Some(description) = preview.description.as_deref() {
        body.push_str(description);
    } else if loading {
        body.push_str("Fetching page metadata...");
    } else {
        body.push_str("No page description was exposed. The link is still ready to open.");
    }

    let _ = writeln!(body);
    let _ = writeln!(body);
    body.push_str(&preview.url);
    body
}

use crate::search_engine;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

lazy_static! {
    pub static ref TABLE_DATA: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    pub static ref TABLE_RESULTS: Mutex<Vec<search_engine::SearchResult>> = Mutex::new(Vec::new());
    pub static ref SEARCH_RT: Runtime = Runtime::new().expect("failed to create tokio runtime");
    pub static ref WINDOW_SHOWING: Mutex<bool> = Mutex::new(false);
    pub static ref DISMISS_ON_ESCAPE: Mutex<bool> = Mutex::new(true);
    pub static ref DISMISS_ON_CLICK_AWAY: Mutex<bool> = Mutex::new(true);
    pub static ref ICON_CACHE: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
    pub static ref TABLE_UPDATE_PENDING: AtomicBool = AtomicBool::new(false);
    pub static ref TABLE_MODE: Mutex<TableMode> = Mutex::new(TableMode::Search);
    pub static ref CURRENT_SEARCH: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
    pub static ref WINDOW_IS_OPEN: AtomicBool = AtomicBool::new(false);
    pub static ref SEARCH_VERSION: AtomicU64 = AtomicU64::new(0);
}

pub static CLIPBOARD_PREVIEW: OnceLock<ClipboardPreviewRefs> = OnceLock::new();
pub static TABLE_SCROLL_VIEW: OnceLock<usize> = OnceLock::new();
pub static SEARCH_FIELD: OnceLock<usize> = OnceLock::new();

#[derive(Copy, Clone, PartialEq)]
pub enum TableMode {
    Search,
    Settings,
    ClipboardHistory,
}

#[derive(Clone)]
pub struct ClipboardPreviewRefs {
    pub root: usize,
    pub title_field: usize,
    pub detail_field: usize,
    pub placeholder_field: usize,
    pub text_scroll: usize,
    pub text_view: usize,
    pub image_view: usize,
    pub text_background: usize,
}

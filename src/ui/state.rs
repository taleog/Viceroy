use crate::search_engine;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
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
}

#[derive(Copy, Clone, PartialEq)]
pub enum TableMode {
    Search,
    ClipboardHistory,
}

#[cfg(target_os = "macos")]
mod app_launcher;
#[cfg(target_os = "macos")]
mod calculator;
#[cfg(target_os = "macos")]
mod clipboard;
#[cfg(target_os = "macos")]
mod database;
#[cfg(target_os = "macos")]
mod dictionary;
#[cfg(target_os = "macos")]
mod emoji;
#[cfg(target_os = "macos")]
mod file_search;
#[cfg(target_os = "macos")]
mod macos_app;
#[cfg(target_os = "macos")]
mod macos_search;
#[cfg(target_os = "macos")]
mod obsidian;
#[cfg(target_os = "macos")]
mod search_engine;
#[cfg(target_os = "macos")]
mod settings;
#[cfg(target_os = "macos")]
mod sync;
#[cfg(target_os = "macos")]
mod system_commands;
#[cfg(target_os = "macos")]
mod ui;
#[cfg(target_os = "macos")]
mod usage;
#[cfg(target_os = "macos")]
mod web_search;

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
mod cli_app;
#[cfg(target_os = "windows")]
mod windows_app;
#[cfg(target_os = "windows")]
mod windows_preview;
#[cfg(target_os = "windows")]
mod windows_style;

#[cfg(target_os = "macos")]
fn main() {
    macos_app::run();
}

#[cfg(target_os = "windows")]
fn main() {
    windows_app::run();
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn main() {
    cli_app::run();
}

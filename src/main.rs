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
mod search_engine;
#[cfg(target_os = "macos")]
mod settings;
#[cfg(target_os = "macos")]
mod system_commands;
#[cfg(target_os = "macos")]
mod ui;
#[cfg(target_os = "macos")]
mod usage;
#[cfg(target_os = "macos")]
mod web_search;

#[cfg(not(target_os = "macos"))]
mod cli_app;

#[cfg(target_os = "macos")]
fn main() {
    macos_app::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    cli_app::run();
}

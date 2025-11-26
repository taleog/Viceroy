#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app_launcher;
mod clipboard;
mod file_search;
mod search_engine;
mod system_commands;
mod calculator;
mod settings;
mod database;
mod emoji;
mod dictionary;
mod web_search;

use tauri::{
    CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    GlobalShortcutManager, Window,
};
use log::info;

#[cfg(target_os = "macos")]
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

#[tauri::command]
async fn search(query: String, mode: Option<String>) -> Result<Vec<search_engine::SearchResult>, String> {
    info!("Search command called with query: {} mode: {:?}", query, mode);
    
    let search_mode = match mode.as_deref() {
        Some("apps") => search_engine::SearchMode::Apps,
        Some("files") => search_engine::SearchMode::Files,
        Some("clipboard") => search_engine::SearchMode::Clipboard,
        Some("calculator") => search_engine::SearchMode::Calculator,
        Some("emoji") => search_engine::SearchMode::Emoji,
        _ => search_engine::SearchMode::All,
    };
    
    let result = search_engine::search_with_mode(&query, search_mode).await.map_err(|e| e.to_string());
    info!("Search results: {} items", result.as_ref().map(|r| r.len()).unwrap_or(0));
    result
}

#[tauri::command]
async fn launch_app(bundle_path: String) -> Result<(), String> {
    app_launcher::launch(&bundle_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    app_launcher::open_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_clipboard_history(limit: usize) -> Result<Vec<clipboard::ClipboardEntry>, String> {
    clipboard::get_history(limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_clipboard(query: String) -> Result<Vec<clipboard::ClipboardEntry>, String> {
    clipboard::search_history(&query).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_clipboard_entry(id: i64, new_name: String) -> Result<(), String> {
    clipboard::rename_entry(id, &new_name).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_pin_clipboard(id: i64) -> Result<(), String> {
    clipboard::toggle_pin(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_clipboard_entry(id: i64) -> Result<(), String> {
    clipboard::delete_entry(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn paste_clipboard_entry(content: String) -> Result<(), String> {
    clipboard::paste_to_active_app(&content).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_clipboard_monitor() -> Result<bool, String> {
    clipboard::toggle_monitor().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn is_clipboard_monitor_paused() -> Result<bool, String> {
    clipboard::is_monitor_paused().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn execute_system_command(command: String) -> Result<String, String> {
    system_commands::execute(&command).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn calculate(expression: String) -> Result<calculator::CalculatorResult, String> {
    calculator::evaluate(&expression).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings() -> Result<settings::Settings, String> {
    settings::load().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(settings: settings::Settings) -> Result<(), String> {
    settings::save(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn show_window(window: Window) {
    window.show().unwrap();
    window.set_focus().unwrap();
}

#[tauri::command]
fn hide_window(window: Window) {
    window.hide().unwrap();
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_dictionary(word: String) -> Result<(), String> {
    dictionary::open_dictionary(&word).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_web_search(url: String) -> Result<(), String> {
    web_search::open_web_search(&url).map_err(|e| e.to_string())
}

fn main() {
    env_logger::init();
    
    // Initialize database
    if let Err(e) = database::init() {
        eprintln!("Failed to initialize database: {}", e);
    }

    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let settings_item = CustomMenuItem::new("settings".to_string(), "Settings");
    let tray_menu = SystemTrayMenu::new()
        .add_item(settings_item)
        .add_item(quit);
    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "settings" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                _ => {}
            },
            _ => {}
        })
        .setup(|app| {
            let window = app.get_window("main").unwrap();
            let window_clone = window.clone();
            
            // Apply macOS vibrancy effect for that polished Viceroy look
            #[cfg(target_os = "macos")]
            apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, Some(12.0))
                .expect("Unsupported platform! 'apply_vibrancy' is only supported on macOS");
            
            // Show window briefly at startup to trigger macOS permission dialogs
            window.show().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(500));
            window.hide().unwrap();
            
            // Register global shortcut (Option+Space like Viceroy)
            app.global_shortcut_manager()
                .register("Alt+Space", move || {
                    if window_clone.is_visible().unwrap() {
                        window_clone.hide().unwrap();
                    } else {
                        window_clone.show().unwrap();
                        window_clone.set_focus().unwrap();
                    }
                })
                .map_err(|e| format!("Failed to register global shortcut: {}", e))?;
            
            // Start clipboard monitor in a separate thread
            std::thread::spawn(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = clipboard::start_monitor().await {
                        eprintln!("Clipboard monitor error: {}", e);
                    }
                });
            });
            
            info!("ViceroyKiller started successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            launch_app,
            open_file,
            get_clipboard_history,
            search_clipboard,
            rename_clipboard_entry,
            toggle_pin_clipboard,
            delete_clipboard_entry,
            paste_clipboard_entry,
            toggle_clipboard_monitor,
            is_clipboard_monitor_paused,
            execute_system_command,
            calculate,
            get_settings,
            save_settings,
            show_window,
            hide_window,
            copy_to_clipboard,
            open_dictionary,
            open_web_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

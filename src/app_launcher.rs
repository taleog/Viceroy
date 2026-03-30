use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct FrontmostApp {
    pub name: String,
    pub pid: i32,
}

lazy_static::lazy_static! {
    static ref APP_CACHE: Mutex<Option<(Vec<AppInfo>, Instant)>> = Mutex::new(None);
}

const CACHE_DURATION: Duration = Duration::from_secs(300);

pub fn search_apps(query: &str) -> Result<Vec<AppInfo>> {
    let apps = get_all_apps()?;
    let query_lower = query.to_lowercase();

    Ok(apps
        .into_iter()
        .filter(|app| app.name.to_lowercase().contains(&query_lower))
        .collect())
}

pub fn get_all_apps() -> Result<Vec<AppInfo>> {
    let mut cache = APP_CACHE.lock().unwrap();

    if let Some((apps, timestamp)) = &*cache {
        if timestamp.elapsed() < CACHE_DURATION {
            return Ok(apps.clone());
        }
    }

    let apps = platform::discover_apps()?;
    *cache = Some((apps.clone(), Instant::now()));
    Ok(apps)
}

pub fn find_app_path_by_name(name: &str) -> Option<String> {
    if name.trim().is_empty() {
        return None;
    }

    if let Ok(apps) = get_all_apps() {
        let lower_name = name.to_lowercase();
        if let Some(app) = apps.iter().find(|app| app.name.eq_ignore_ascii_case(name)) {
            return Some(app.path.clone());
        }
        if let Some(app) = apps
            .iter()
            .find(|app| app.name.to_lowercase().contains(&lower_name))
        {
            return Some(app.path.clone());
        }
    }

    None
}

pub fn launch(path: &str) -> Result<()> {
    platform::launch(path)
}

pub fn open_file(path: &str) -> Result<()> {
    platform::open_file(path)
}

pub fn get_frontmost_app_name() -> Option<String> {
    platform::get_frontmost_app_name()
}

pub fn get_frontmost_app() -> Option<FrontmostApp> {
    platform::get_frontmost_app()
}

pub fn activate_frontmost_app(app: &FrontmostApp) -> Result<()> {
    platform::activate_frontmost_app(app)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{AppInfo, FrontmostApp};
    use anyhow::Result;
    use cocoa::base::{id, nil, BOOL, YES};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{class, msg_send, sel, sel_impl};
    use std::process::Command;

    pub fn discover_apps() -> Result<Vec<AppInfo>> {
        let mut apps = Vec::new();

        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let _workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];

            let home_apps = format!("{}/Applications", std::env::var("HOME").unwrap_or_default());
            let app_dirs = vec!["/Applications", "/System/Applications", home_apps.as_str()];

            for dir in app_dirs {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".app") {
                                let path = entry.path().to_string_lossy().to_string();
                                let display_name = name.trim_end_matches(".app").to_string();
                                apps.push(AppInfo {
                                    name: display_name,
                                    path,
                                });
                            }
                        }
                    }
                }
            }
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(apps)
    }

    pub fn launch(path: &str) -> Result<()> {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];

            let ns_path = NSString::alloc(nil);
            let ns_path = NSString::init_str(ns_path, path);

            let _: id = msg_send![workspace, openFile: ns_path];
        }

        Ok(())
    }

    pub fn open_file(path: &str) -> Result<()> {
        Command::new("open").arg(path).spawn()?;
        Ok(())
    }

    pub fn get_frontmost_app_name() -> Option<String> {
        get_frontmost_app().map(|app| app.name)
    }

    pub fn get_frontmost_app() -> Option<FrontmostApp> {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let frontmost_app: id = msg_send![workspace, frontmostApplication];

            if frontmost_app != nil {
                let pid: i32 = msg_send![frontmost_app, processIdentifier];
                let localized_name: id = msg_send![frontmost_app, localizedName];
                if localized_name != nil {
                    let name_ptr: *const i8 = msg_send![localized_name, UTF8String];
                    if !name_ptr.is_null() {
                        let name = std::ffi::CStr::from_ptr(name_ptr)
                            .to_string_lossy()
                            .to_string();
                        return Some(FrontmostApp { name, pid });
                    }
                }
            }
        }

        None
    }

    pub fn activate_frontmost_app(app: &FrontmostApp) -> Result<()> {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let running_app_class = class!(NSRunningApplication);
            let running_app: id = msg_send![
                running_app_class,
                runningApplicationWithProcessIdentifier: app.pid
            ];

            if running_app != nil {
                let options: u64 = 1 << 1;
                let activated: BOOL = msg_send![running_app, activateWithOptions: options];
                if activated == YES {
                    return Ok(());
                }
            }
        }

        let script = format!(
            r#"tell application "{}" to activate"#,
            app.name.replace('\"', "\\\"")
        );
        let status = Command::new("osascript").arg("-e").arg(&script).status()?;
        if status.success() {
            return Ok(());
        }

        anyhow::bail!("failed to activate app: {}", app.name)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{launch_via_shell, AppInfo, FrontmostApp};
    use anyhow::Result;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use walkdir::WalkDir;

    pub fn discover_apps() -> Result<Vec<AppInfo>> {
        let mut apps = Vec::new();
        let mut seen = HashSet::new();

        for root in app_roots() {
            if !root.exists() {
                continue;
            }

            for entry in WalkDir::new(root)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_map(|entry| entry.ok())
            {
                let path = entry.path();
                if !path.is_file() || !is_app_candidate(path) {
                    continue;
                }

                let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                    continue;
                };

                let normalized = name.to_ascii_lowercase();
                if !seen.insert(normalized) {
                    continue;
                }

                apps.push(AppInfo {
                    name: name.to_string(),
                    path: path.to_string_lossy().to_string(),
                });
            }
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(apps)
    }

    pub fn launch(path: &str) -> Result<()> {
        launch_via_shell(path)
    }

    pub fn open_file(path: &str) -> Result<()> {
        launch_via_shell(path)
    }

    pub fn get_frontmost_app_name() -> Option<String> {
        None
    }

    pub fn get_frontmost_app() -> Option<FrontmostApp> {
        None
    }

    pub fn activate_frontmost_app(_app: &FrontmostApp) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn app_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(app_data) = std::env::var("APPDATA") {
            roots.push(
                PathBuf::from(app_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            roots.push(
                PathBuf::from(program_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
        if let Some(desktop) = dirs::desktop_dir() {
            roots.push(desktop);
        }

        roots
    }

    #[cfg(not(target_os = "windows"))]
    fn app_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Some(data_dir) = dirs::data_dir() {
            roots.push(data_dir.join("applications"));
        }
        if let Some(home_dir) = dirs::home_dir() {
            roots.push(home_dir.join(".local").join("share").join("applications"));
        }

        roots
    }

    #[cfg(target_os = "windows")]
    fn is_app_candidate(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(ext) if ext.eq_ignore_ascii_case("lnk")
                || ext.eq_ignore_ascii_case("exe")
                || ext.eq_ignore_ascii_case("url")
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn is_app_candidate(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(ext) if ext.eq_ignore_ascii_case("desktop") || ext.eq_ignore_ascii_case("AppImage")
        )
    }
}

#[cfg(target_os = "windows")]
fn launch_via_shell(target: &str) -> Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", target])
        .spawn()?;
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn launch_via_shell(target: &str) -> Result<()> {
    std::process::Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

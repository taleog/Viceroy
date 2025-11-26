use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::process::Command;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSString, NSAutoreleasePool};
use objc::{msg_send, sel, sel_impl, class};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub path: String,
}

lazy_static::lazy_static! {
    static ref APP_CACHE: Mutex<Option<(Vec<AppInfo>, Instant)>> = Mutex::new(None);
}

const CACHE_DURATION: Duration = Duration::from_secs(300); // 5 minutes

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
    
    // Check if cache is valid
    if let Some((apps, timestamp)) = &*cache {
        if timestamp.elapsed() < CACHE_DURATION {
            return Ok(apps.clone());
        }
    }
    
    // Rebuild cache
    let apps = discover_apps()?;
    *cache = Some((apps.clone(), Instant::now()));
    
    Ok(apps)
}

fn discover_apps() -> Result<Vec<AppInfo>> {
    let mut apps = Vec::new();
    
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let _workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        
        // Get applications from common directories
        let home_apps = format!("{}/Applications", std::env::var("HOME").unwrap_or_default());
        let app_dirs = vec![
            "/Applications",
            "/System/Applications",
            home_apps.as_str(),
        ];
        
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

pub fn launch(bundle_path: &str) -> Result<()> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        
        let ns_path = NSString::alloc(nil);
        let ns_path = NSString::init_str(ns_path, bundle_path);
        
        let _: id = msg_send![workspace, openFile: ns_path];
    }
    
    Ok(())
}

pub fn open_file(path: &str) -> Result<()> {
    Command::new("open")
        .arg(path)
        .spawn()?;
    
    Ok(())
}

pub fn get_frontmost_app_name() -> Option<String> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let frontmost_app: id = msg_send![workspace, frontmostApplication];
        
        if frontmost_app != nil {
            let localized_name: id = msg_send![frontmost_app, localizedName];
            if localized_name != nil {
                let name_ptr: *const i8 = msg_send![localized_name, UTF8String];
                if !name_ptr.is_null() {
                    let name = std::ffi::CStr::from_ptr(name_ptr)
                        .to_string_lossy()
                        .to_string();
                    return Some(name);
                }
            }
        }
    }
    
    None
}

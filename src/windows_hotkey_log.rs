#![cfg(target_os = "windows")]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn append(line: &str) {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let mut dir = PathBuf::from(appdata);
        dir.push("viceroy");
        let _ = fs::create_dir_all(&dir);
        let mut file = dir;
        file.push("hotkey.log");

        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&file) {
            let _ = writeln!(f, "{}", line);
        }
    }
}

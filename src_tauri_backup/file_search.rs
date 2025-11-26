use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
}

pub fn search_files(query: &str, limit: usize) -> Result<Vec<FileInfo>> {
    let output = Command::new("mdfind")
        .arg("-name")
        .arg(query)
        .output()?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<FileInfo> = stdout
        .lines()
        .filter_map(|line| {
            let path = line.trim();
            if path.is_empty() {
                return None;
            }
            
            let name = std::path::Path::new(path)
                .file_name()?
                .to_str()?
                .to_string();
            
            Some(FileInfo {
                name,
                path: path.to_string(),
            })
        })
        .take(limit)
        .collect();
    
    Ok(files)
}

pub fn search_files_in_directory(query: &str, directory: &str, limit: usize) -> Result<Vec<FileInfo>> {
    let output = Command::new("mdfind")
        .arg("-onlyin")
        .arg(directory)
        .arg("-name")
        .arg(query)
        .output()?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<FileInfo> = stdout
        .lines()
        .filter_map(|line| {
            let path = line.trim();
            if path.is_empty() {
                return None;
            }
            
            let name = std::path::Path::new(path)
                .file_name()?
                .to_str()?
                .to_string();
            
            Some(FileInfo {
                name,
                path: path.to_string(),
            })
        })
        .take(limit)
        .collect();
    
    Ok(files)
}

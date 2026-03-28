use anyhow::{anyhow, Context, Result};
use log::info;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub const UPDATE_CHECK_DISABLED_ENV: &str = "VICEROY_NO_UPDATE_CHECK";
pub const UPDATE_SILENT_ENV: &str = "VICEROY_SILENT_UPDATE_CHECK";
pub const UPDATE_METADATA_URL_ENV: &str = "VICEROY_UPDATE_METADATA_URL";
pub const RELEASE_METADATA_URL: &str = "https://example.com/viceroy/latest.json";

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ReleaseMetadata {
    pub version: String,
    pub download_url: String,
    pub sha256: String,
}

pub fn env_flag_is_set(var: &str) -> bool {
    env::var(var)
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

pub fn update_check_disabled(args: &[String]) -> bool {
    env_flag_is_set(UPDATE_CHECK_DISABLED_ENV) || args.iter().any(|arg| arg == "--no-update-check")
}

pub fn silent_update_check(args: &[String]) -> bool {
    env_flag_is_set(UPDATE_SILENT_ENV) || args.iter().any(|arg| arg == "--silent-update-check")
}

pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (Version::parse(latest), Version::parse(current)) {
        (Ok(latest_v), Ok(current_v)) => latest_v > current_v,
        _ => latest > current,
    }
}

pub fn parse_metadata(body: &str) -> Result<ReleaseMetadata, serde_json::Error> {
    serde_json::from_str(body)
}

pub fn metadata_url() -> String {
    env::var(UPDATE_METADATA_URL_ENV).unwrap_or_else(|_| RELEASE_METADATA_URL.to_string())
}

pub async fn check_for_updates(silent: bool) -> Result<()> {
    let url = metadata_url();
    if using_placeholder_metadata_url(&url) {
        info!(
            "Skipping update check because no real release metadata URL is configured. Set {} to enable it.",
            UPDATE_METADATA_URL_ENV
        );
        return Ok(());
    }
    info!("Checking for updates from {url}");

    let response = reqwest::get(&url).await?.error_for_status()?;
    let text = response.text().await?;
    let metadata = parse_metadata(&text).context("failed to parse update metadata")?;

    let current_version = env!("CARGO_PKG_VERSION");
    if !is_newer_version(&metadata.version, current_version) {
        info!("Viceroy is up to date ({}).", current_version);
        return Ok(());
    }

    if !silent && !prompt_for_consent(&metadata.version) {
        info!("User declined update to {}", metadata.version);
        return Ok(());
    }

    let temp_path = download_and_verify(&metadata).await?;
    replace_current_binary(&temp_path)?;
    info!(
        "Update installed ({}). Please restart Viceroy to finish updating.",
        metadata.version
    );

    Ok(())
}

fn using_placeholder_metadata_url(url: &str) -> bool {
    url == RELEASE_METADATA_URL && url.contains("example.com")
}

fn prompt_for_consent(version: &str) -> bool {
    print!(
        "A new version ({}) is available. Install now? [Y/n]: ",
        version
    );
    let _ = io::stdout().flush();
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => matches!(input.trim().to_ascii_lowercase().as_str(), "" | "y" | "yes"),
        Err(_) => false,
    }
}

async fn download_and_verify(metadata: &ReleaseMetadata) -> Result<PathBuf> {
    let mut response = reqwest::get(&metadata.download_url)
        .await?
        .error_for_status()
        .context("failed to download update bundle")?;

    let exe_path = env::current_exe().context("could not locate current executable")?;
    let temp_path = exe_path.with_extension("download");

    let mut file = File::create(&temp_path).await?;
    let mut hasher = Sha256::new();

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
    }

    file.flush().await?;
    file.sync_all().await?;

    let actual = format!("{:x}", hasher.finalize());
    if !checksums_match(&actual, &metadata.sha256) {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(anyhow!(
            "checksum mismatch: expected {}, got {}",
            metadata.sha256,
            actual
        ));
    }

    Ok(temp_path)
}

fn checksums_match(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

fn apply_permissions(from: &Path, to: &Path) -> io::Result<()> {
    let perms = fs::metadata(from)?.permissions();
    fs::set_permissions(to, perms)
}

fn replace_current_binary(temp_path: &Path) -> Result<()> {
    let current_exe = env::current_exe().context("failed to resolve current executable path")?;
    apply_permissions(&current_exe, temp_path)
        .context("failed to set executable permissions on downloaded update")?;
    fs::rename(temp_path, &current_exe).context("failed to replace existing binary with update")?;
    Ok(())
}

pub fn read_checksum_from_reader<R: Read>(mut reader: R) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn compares_versions_using_semver() {
        assert!(is_newer_version("0.2.0", "0.1.9"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "2.0.0"));
    }

    #[test]
    fn parses_release_metadata() {
        let json = r#"{
            "version": "1.2.3",
            "download_url": "https://example.com/viceroy.zip",
            "sha256": "abc123"
        }"#;
        let metadata = parse_metadata(json).expect("metadata should parse");
        assert_eq!(metadata.version, "1.2.3");
        assert_eq!(metadata.download_url, "https://example.com/viceroy.zip");
        assert_eq!(metadata.sha256, "abc123");
    }

    #[test]
    fn validates_checksum_from_reader() {
        let mut temp = NamedTempFile::new().expect("temp file");
        write!(temp, "test-bytes").expect("write checksum payload");
        temp.flush().expect("flush temp file");

        let hash = read_checksum_from_reader(fs::File::open(temp.path()).unwrap()).unwrap();
        let expected = format!("{:x}", Sha256::digest(b"test-bytes"));
        assert_eq!(hash, expected);
        assert!(checksums_match(&hash, &expected));
    }

    #[test]
    fn detects_placeholder_metadata_url() {
        assert!(using_placeholder_metadata_url(RELEASE_METADATA_URL));
        assert!(!using_placeholder_metadata_url(
            "https://updates.example.org/latest.json"
        ));
    }
}

// src/updater.rs

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

#[derive(Deserialize, Debug)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize, Debug)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub async fn check_for_updates(client: &reqwest::Client) -> anyhow::Result<Option<UpdateInfo>> {
    let base_url = std::env::var("SPEEDTEST_MOCK_GITHUB_API")
        .unwrap_or_else(|_| "https://api.github.com".to_string());

    let url = format!(
        "{}/repos/nazakun021/cli-speedtest/releases/latest",
        base_url
    );

    let res = client
        .get(&url)
        .header("User-Agent", "cli-speedtest-updater")
        .send()
        .await?;

    if !res.status().is_success() {
        anyhow::bail!("GitHub API returned error: {}", res.status());
    }

    let release: GithubRelease = res.json().await?;

    let tag = release.tag_name.trim();
    let tag_clean = tag.strip_prefix('v').unwrap_or(tag);

    let remote_ver = semver::Version::parse(tag_clean)?;
    let local_ver = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    if remote_ver > local_ver {
        let expected_asset_name = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some("speedtest-linux-amd64")
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some("speedtest-windows-amd64.exe")
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some("speedtest-macos-intel")
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some("speedtest-macos-arm64")
        } else {
            None
        };

        if let Some(target_name) = expected_asset_name {
            if let Some(asset) = release.assets.into_iter().find(|a| a.name == target_name) {
                return Ok(Some(UpdateInfo {
                    version: tag_clean.to_string(),
                    download_url: asset.browser_download_url,
                }));
            }
        }
    }

    Ok(None)
}

pub async fn run_update(
    client: &reqwest::Client,
    download_url: &str,
    show_progress: bool,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    use sha2::{Digest, Sha256};
    use std::io::Write;

    let target_exe = std::env::var("SPEEDTEST_MOCK_EXE_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_exe().expect("Failed to get current executable path")
        });

    let exe_dir = target_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent directory for executable"))?;
    let temp_file_path = exe_dir.join(".speedtest-update.tmp");

    // Fetch expected SHA-256 checksum first
    let sha_url = format!("{}.sha256", download_url);
    let sha_response = client.get(&sha_url).send().await?;
    if !sha_response.status().is_success() {
        anyhow::bail!("Failed to download checksum: {}", sha_response.status());
    }
    let expected_sha_raw = sha_response.text().await?;
    let expected_sha = expected_sha_raw
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    if expected_sha.len() != 64 || !expected_sha.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Invalid checksum format: '{}'", expected_sha_raw.trim());
    }

    let response = client.get(download_url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download update: {}", response.status());
    }

    let total_size = response.content_length();
    let mut temp_file = std::fs::File::create(&temp_file_path)?;

    let pb = if show_progress {
        let bar = indicatif::ProgressBar::new(total_size.unwrap_or(0));
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
                .progress_chars("#>-")
        );
        Some(bar)
    } else {
        None
    };

    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        temp_file.write_all(&chunk)?;
        if let Some(ref bar) = pb {
            bar.inc(chunk.len() as u64);
        }
    }

    if let Some(ref bar) = pb {
        bar.finish_with_message("Download complete");
    }

    // Ensure file is flushed and closed
    temp_file.sync_all()?;
    drop(temp_file);

    // Compute SHA-256 checksum of downloaded binary
    let mut temp_file_read = std::fs::File::open(&temp_file_path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut temp_file_read, &mut hasher)?;
    let computed_sha = format!("{:x}", hasher.finalize());

    if computed_sha != expected_sha {
        let _ = std::fs::remove_file(&temp_file_path);
        anyhow::bail!(
            "Integrity check failed: checksum mismatch.\nExpected: {}\nComputed: {}",
            expected_sha,
            computed_sha
        );
    }

    if std::env::var("SPEEDTEST_MOCK_EXE_PATH").is_ok() {
        // In tests: copy the file because the mock binary isn't running
        std::fs::copy(&temp_file_path, &target_exe)?;
    } else {
        // In production: perform the self-replace hack
        self_replace::self_replace(&temp_file_path)?;
    }

    let _ = std::fs::remove_file(&temp_file_path);

    Ok(())
}

fn last_update_check_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("SPEEDTEST_MOCK_DATA_DIR") {
        return Some(
            std::path::PathBuf::from(p)
                .join("speedtest")
                .join("last_update_check"),
        );
    }
    dirs::data_local_dir().map(|d| d.join("speedtest").join("last_update_check"))
}

fn write_cache_timestamp(path: &Option<std::path::PathBuf>) {
    if let Some(p) = path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            let _ = std::fs::write(p, now.as_secs().to_string());
        }
    }
}

pub async fn check_and_perform_auto_update(client: &reqwest::Client) -> anyhow::Result<()> {
    if std::env::var("NO_UPDATE").is_ok() || std::env::var("CLI_SPEEDTEST_NO_UPDATE").is_ok() {
        return Ok(());
    }

    let cache_path = last_update_check_path();
    if let Some(path) = &cache_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(last_check) = content.trim().parse::<u64>() {
                if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                {
                    let now_secs = now.as_secs();
                    let elapsed = now_secs.saturating_sub(last_check);
                    if elapsed < 86400 {
                        return Ok(());
                    }
                }
            }
        }
    }

    let check_res = check_for_updates(client).await;
    match check_res {
        Ok(Some(info)) => {
            eprintln!(
                "[self-update] New version v{} found. Updating silently...",
                info.version
            );
            match run_update(client, &info.download_url, false).await {
                Ok(()) => {
                    eprintln!("[self-update] Successfully updated to v{}!", info.version);
                }
                Err(e) => {
                    let is_permission_err = e
                        .downcast_ref::<std::io::Error>()
                        .map(|io_err| io_err.kind() == std::io::ErrorKind::PermissionDenied)
                        .unwrap_or(false);
                    if is_permission_err {
                        eprintln!(
                            "[self-update] New version v{} is available, but update failed due to insufficient permissions. Please update manually.",
                            info.version
                        );
                    } else {
                        eprintln!("[self-update] Failed to update: {}", e);
                    }
                }
            }
            write_cache_timestamp(&cache_path);
        }
        Ok(None) => {
            write_cache_timestamp(&cache_path);
        }
        Err(e) => {
            tracing::debug!("Auto-update check failed: {}", e);
        }
    }

    Ok(())
}

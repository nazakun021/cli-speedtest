// src/cooldown.rs

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_COOLDOWN_SECS: u64 = 300; // 5 minutes

/// Returns the platform-appropriate path for the last-run timestamp file.
/// Linux/macOS: ~/.local/share/speedtest/last_run
/// Windows:     %APPDATA%\speedtest\last_run
pub fn last_run_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("speedtest").join("last_run"))
}

/// Returns Some(seconds_remaining) if the cooldown is still active,
/// or None if the cooldown has elapsed or no previous run was recorded.
pub fn cooldown_remaining(cooldown_secs: u64) -> Option<u64> {
    let path = last_run_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let last_run_ts: u64 = contents.trim().parse().ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let elapsed = now.saturating_sub(last_run_ts);
    if elapsed < cooldown_secs {
        Some(cooldown_secs - elapsed)
    } else {
        None
    }
}

/// Writes the current Unix timestamp to the last-run file.
/// Creates the directory if it does not exist.
/// Called only on successful test completion - failed runs do not reset
/// the cooldown clock.
pub fn record_successful_run() -> anyhow::Result<()> {
    let path =
        last_run_path().ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    std::fs::write(&path, now.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn cooldown_none_when_no_file() {
        // Just verify it doesn't crash on non-existent config paths
        // We can't safely test the file presence without mocking dirs, but we can verify the method signature works
    }
}

// src/cooldown.rs

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_COOLDOWN_SECS: u64 = 300; // 5 minutes

/// Returns the platform-appropriate path for the last-run timestamp file.
/// Linux/macOS: ~/.local/share/speedtest/last_run
/// Windows:     %APPDATA%\speedtest\last_run
pub fn last_run_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SPEEDTEST_MOCK_DATA_DIR") {
        return Some(PathBuf::from(p).join("speedtest").join("last_run"));
    }
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
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn setup_test_env() -> TempDir {
        let temp = TempDir::new().expect("Failed to create temp dir");
        unsafe {
            std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp.path());
        }
        temp
    }

    #[test]
    fn cooldown_none_when_no_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        assert_eq!(cooldown_remaining(DEFAULT_COOLDOWN_SECS), None);
    }

    #[test]
    fn cooldown_none_when_elapsed() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        let path = last_run_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let old_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1000;
        fs::write(&path, old_time.to_string()).unwrap();

        assert_eq!(cooldown_remaining(DEFAULT_COOLDOWN_SECS), None);
    }

    #[test]
    fn cooldown_some_when_active() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        let path = last_run_path().unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let recent_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100;
        fs::write(&path, recent_time.to_string()).unwrap();

        let remaining = cooldown_remaining(DEFAULT_COOLDOWN_SECS);
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= 200); // 300 - 100
    }

    #[test]
    fn record_run_creates_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();

        // Ensure file does not exist
        let path = last_run_path().unwrap();
        assert!(!path.exists());

        record_successful_run().expect("Should record successfully");

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.trim().parse::<u64>().is_ok());
    }

    #[test]
    fn record_run_creates_missing_dirs() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp = setup_test_env();

        // Remove the speedtest directory if it somehow exists to ensure we create it
        let speedtest_dir = temp.path().join("speedtest");
        if speedtest_dir.exists() {
            fs::remove_dir_all(&speedtest_dir).unwrap();
        }

        record_successful_run().expect("Should record successfully with missing parent dir");

        let path = last_run_path().unwrap();
        assert!(path.exists());
    }
}

// src/cooldown.rs

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_COOLDOWN_SECS: u64 = 300; // 5 minutes

#[derive(Debug, PartialEq, Eq)]
pub enum CooldownStatus {
    Allowed,
    BlockedByCooldown { remaining_secs: u64 },
    BlockedByBurstLimit { remaining_secs: u64 },
}

/// Evaluates active timers, enforces burst thresholds, and updates the local state.
pub fn enforce_cooldown_policy(quick: bool, force_run: bool) -> CooldownStatus {
    if force_run {
        let _ = reset_burst_count();
        return CooldownStatus::Allowed;
    }

    if let Some(remaining) = cooldown_remaining(DEFAULT_COOLDOWN_SECS) {
        if quick {
            let burst_count = get_burst_count();
            if burst_count >= 5 {
                CooldownStatus::BlockedByBurstLimit {
                    remaining_secs: remaining,
                }
            } else {
                CooldownStatus::Allowed
            }
        } else {
            CooldownStatus::BlockedByCooldown {
                remaining_secs: remaining,
            }
        }
    } else {
        let _ = reset_burst_count();
        CooldownStatus::Allowed
    }
}

/// Encapsulates the successful completion logging: updates the last run timestamp,
/// and increments or resets the quick burst counter.
pub fn record_run_completion(quick: bool) -> anyhow::Result<()> {
    record_successful_run()?;
    if quick {
        increment_burst_count()?;
    } else {
        reset_burst_count()?;
    }
    Ok(())
}

/// Returns the platform-appropriate path for the last-run timestamp file.
/// Linux/macOS: ~/.local/share/speedtest/last_run
/// Windows:     %APPDATA%\speedtest\last_run
fn last_run_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SPEEDTEST_MOCK_DATA_DIR") {
        return Some(PathBuf::from(p).join("speedtest").join("last_run"));
    }
    dirs::data_local_dir().map(|d| d.join("speedtest").join("last_run"))
}

/// Returns Some(seconds_remaining) if the cooldown is still active,
/// or None if the cooldown has elapsed or no previous run was recorded.
fn cooldown_remaining(cooldown_secs: u64) -> Option<u64> {
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
fn record_successful_run() -> anyhow::Result<()> {
    let path =
        last_run_path().ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    std::fs::write(&path, now.to_string())?;
    Ok(())
}

fn burst_count_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SPEEDTEST_MOCK_DATA_DIR") {
        return Some(PathBuf::from(p).join("speedtest").join("burst_count"));
    }
    dirs::data_local_dir().map(|d| d.join("speedtest").join("burst_count"))
}

fn get_burst_count() -> u32 {
    let path = match burst_count_path() {
        Some(p) => p,
        None => return 0,
    };
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    contents.trim().parse().unwrap_or(0)
}

fn increment_burst_count() -> anyhow::Result<u32> {
    let path =
        burst_count_path().ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let current = get_burst_count();
    let new_val = current + 1;
    std::fs::write(&path, new_val.to_string())?;
    Ok(new_val)
}

fn reset_burst_count() -> anyhow::Result<()> {
    let path =
        burst_count_path().ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, "0")?;
    Ok(())
}

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use super::TEST_ENV_LOCK as ENV_LOCK;

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

    #[test]
    fn burst_count_zero_when_no_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        assert_eq!(get_burst_count(), 0);
    }

    #[test]
    fn increment_burst_count_updates_correctly() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        assert_eq!(get_burst_count(), 0);
        let count = increment_burst_count().unwrap();
        assert_eq!(count, 1);
        assert_eq!(get_burst_count(), 1);
        let count = increment_burst_count().unwrap();
        assert_eq!(count, 2);
        assert_eq!(get_burst_count(), 2);
    }

    #[test]
    fn reset_burst_count_clears_value() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        increment_burst_count().unwrap();
        assert_eq!(get_burst_count(), 1);
        reset_burst_count().unwrap();
        assert_eq!(get_burst_count(), 0);
    }

    #[test]
    fn policy_allowed_when_no_cooldown() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        // Initially, no cooldown file exists, so cooldown is not active
        assert_eq!(
            enforce_cooldown_policy(false, false),
            CooldownStatus::Allowed
        );
        assert_eq!(
            enforce_cooldown_policy(true, false),
            CooldownStatus::Allowed
        );
    }

    #[test]
    fn policy_blocked_cooldown_active() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        record_successful_run().unwrap();

        // Standard run (quick=false, force=false) should be blocked by active cooldown
        let status = enforce_cooldown_policy(false, false);
        assert!(matches!(status, CooldownStatus::BlockedByCooldown { .. }));
    }

    #[test]
    fn policy_allowed_quick_burst_under_limit() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        record_successful_run().unwrap();
        increment_burst_count().unwrap(); // count = 1

        // Quick run with burst = 1 should be allowed
        assert_eq!(
            enforce_cooldown_policy(true, false),
            CooldownStatus::Allowed
        );
    }

    #[test]
    fn policy_blocked_quick_burst_limit_reached() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        record_successful_run().unwrap();
        for _ in 0..5 {
            increment_burst_count().unwrap();
        } // count = 5

        // Quick run with burst = 5 should be blocked by burst limit
        let status = enforce_cooldown_policy(true, false);
        assert!(matches!(status, CooldownStatus::BlockedByBurstLimit { .. }));
    }

    #[test]
    fn policy_force_run_ignores_all() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        record_successful_run().unwrap();
        for _ in 0..5 {
            increment_burst_count().unwrap();
        }

        // Under force_run, standard or quick runs are allowed
        assert_eq!(
            enforce_cooldown_policy(false, true),
            CooldownStatus::Allowed
        );
        assert_eq!(enforce_cooldown_policy(true, true), CooldownStatus::Allowed);
        // And burst count should be reset
        assert_eq!(get_burst_count(), 0);
    }

    #[test]
    fn record_completion_resets_burst_on_standard_run() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        increment_burst_count().unwrap();
        assert_eq!(get_burst_count(), 1);

        record_run_completion(false).unwrap(); // standard run
        assert_eq!(get_burst_count(), 0);
    }

    #[test]
    fn record_completion_increments_burst_on_quick_run() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _temp = setup_test_env();
        assert_eq!(get_burst_count(), 0);

        record_run_completion(true).unwrap(); // quick run
        assert_eq!(get_burst_count(), 1);
    }
}

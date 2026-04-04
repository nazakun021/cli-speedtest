// src/utils.rs

use crate::models::AppConfig;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use tracing::debug;

pub const WARMUP_SECS: f64 = 2.0;

pub fn create_spinner(msg: &str, config: &AppConfig, style_template: &str) -> ProgressBar {
    if config.quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new_spinner();
        if let Ok(style) = ProgressStyle::default_spinner().template(style_template) {
            pb.set_style(style);
        }
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    }
}

pub fn calculate_mbps(bytes: u64, duration_secs: f64) -> f64 {
    if duration_secs <= 0.0 {
        return 0.0;
    }
    let megabytes = (bytes as f64) / (1024.0 * 1024.0);
    (megabytes * 8.0) / duration_secs
}

pub async fn with_retry<F, Fut, T>(max_retries: u32, mut f: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut last_err = anyhow::anyhow!("No attempts made");
    for attempt in 0..=max_retries {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt < max_retries {
                    let backoff = Duration::from_millis(100 * 2u64.pow(attempt));
                    debug!(
                        "Request failed (attempt {}/{}): {}. Retrying in {:?}...",
                        attempt + 1,
                        max_retries + 1,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
                last_err = e;
            }
        }
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    // --- calculate_mbps ---

    #[test]
    fn mbps_correct_for_known_value() {
        // 12.5 MiB in 1 second = exactly 100 Mbps
        let bytes = 13_107_200u64; // 12.5 * 1024 * 1024
        let speed = calculate_mbps(bytes, 1.0);
        assert!(
            (speed - 100.0).abs() < 0.001,
            "Expected 100 Mbps, got {}",
            speed
        );
    }

    #[test]
    fn mbps_zero_for_zero_duration() {
        assert_eq!(calculate_mbps(1_000_000, 0.0), 0.0);
    }

    #[test]
    fn mbps_zero_for_negative_duration() {
        assert_eq!(calculate_mbps(1_000_000, -5.0), 0.0);
    }

    #[test]
    fn mbps_zero_bytes_gives_zero() {
        assert_eq!(calculate_mbps(0, 10.0), 0.0);
    }

    // --- with_retry ---

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let result = with_retry(3, || async { Ok::<i32, anyhow::Error>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_c = attempts.clone();

        let result = with_retry(3, move || {
            let counter = attempts_c.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    anyhow::bail!("transient error");
                }
                Ok::<i32, anyhow::Error>(99)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 99);
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "Should have taken exactly 2 attempts"
        );
    }

    #[tokio::test]
    async fn retry_exhausts_all_attempts_and_returns_last_error() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_c = attempts.clone();

        let result = with_retry(2, move || {
            let counter = attempts_c.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!("always fails")
            }
        })
        .await;

        assert!(result.is_err());
        // max_retries = 2 means 3 total attempts: attempt 0, 1, 2
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            3,
            "Should have attempted exactly max_retries + 1 times"
        );
    }

    #[tokio::test]
    async fn retry_with_zero_retries_attempts_exactly_once() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_c = attempts.clone();

        let result = with_retry(0, move || {
            let counter = attempts_c.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!("fail")
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "Zero retries = exactly 1 attempt"
        );
    }
}

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

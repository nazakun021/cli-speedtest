// src/models.rs

use serde::Serialize;

/// Shared config object — replaces quiet: bool prop-drilling (AppConfig already exists)
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub quiet: bool,
}

/// All parameters the core `run()` function needs, decoupled from clap's `Args`.
/// This is what allows integration tests to call `run()` without constructing CLI args.
#[derive(Debug, Clone)]
pub struct RunArgs {
    pub server_url: String,
    pub duration_secs: u64,
    pub connections: Option<usize>,
    pub ping_count: u32,
    pub no_download: bool,
    pub no_upload: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct PingStats {
    pub min_ms: u128,
    pub max_ms: u128,
    pub avg_ms: f64,
    pub jitter_ms: f64,
    pub packet_loss_pct: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct SpeedTestResult {
    pub timestamp: String,
    pub version: String,
    pub server_name: String,
    pub ping: PingStats,
    // CHANGED: Option<f64> so skipped tests serialize as null / are absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_mbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_mbps: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Server {
    pub name: String,
    pub base_url: String,
}

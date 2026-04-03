use serde::Serialize;

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
    pub download_mbps: f64,
    pub upload_mbps: f64,
}

#[derive(Debug, Clone)]
pub struct Server {
    pub name: String,
    pub base_url: String,
}

use bytes::Bytes;
use clap::Parser;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rand::RngCore;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::debug;

const WARMUP_SECS: f64 = 2.0;
const MAX_RETRIES: u32 = 3;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// A blazing fast CLI Speedtest written in Rust
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Duration of the download/upload tests in seconds
    #[arg(short, long, default_value_t = 10)]
    duration: u64,

    /// Output results as JSON (suppresses all visual UI)
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Enable debug logging for troubleshooting
    #[arg(long, default_value_t = false)]
    debug: bool,

    /// Number of pings to send for latency/jitter measurement
    #[arg(long, default_value_t = 20)]
    ping_count: u32,
}

#[derive(Serialize)]
struct PingStats {
    min_ms: u128,
    max_ms: u128,
    avg_ms: f64,
    jitter_ms: f64,
    packet_loss_pct: f64,
}

#[derive(Debug, Clone)]
struct Server {
    name: String,
    base_url: String,
}

// 1. Define our JSON output structure
#[derive(Serialize)]
struct SpeedTestResult {
    server_name: String,
    ping: PingStats,
    download_mbps: f64,
    upload_mbps: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let log_level = if args.debug { "debug" } else { "error" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_writer(std::io::stderr)
        .init();

    debug!("Application started with args: {:?}", args);

    let client = Client::builder()
        .user_agent("rust-speedtest/0.1.0")
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    tokio::select! {
        res = run_app(args.clone(), client) => {
            match res {
                Ok(result) => {
                    if args.json {
                        let json_out = serde_json::to_string_pretty(&result)?;
                        println!("{}", json_out);
                    }
                }
                Err(e) => {
                    if args.json {
                        println!(r#"{{"error": "{}"}}"#, e);
                    } else {
                        eprintln!("❌ Error: {}", e);
                    }
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            if args.json {
                println!(r#"{{"error": "aborted_by_user"}}"#);
            } else {
                print!("\r\x1b[2K\x1b[?25h");
                println!("⚠️  Speedtest aborted by user.");
            }
            std::process::exit(130);
        }
    }

    Ok(())
}

async fn run_app(args: Args, client: Client) -> anyhow::Result<SpeedTestResult> {
    let quiet = args.json;

    if args.duration <= WARMUP_SECS as u64 {
        anyhow::bail!(
            "Duration must be greater than {} seconds (warm-up period). Got: {}s",
            WARMUP_SECS,
            args.duration
        );
    }

    if !quiet {
        println!("🚀 Starting Rust Speedtest...\n");
    }

    // Single server — Cloudflare anycast routes to the nearest edge automatically
    let server = Server {
        name: "Cloudflare".into(),
        base_url: "https://speed.cloudflare.com".into(),
    };

    if !quiet {
        println!("🔍 Using server: {}\n", server.name);
    }

    // --- Ping / Jitter / Packet Loss ---
    let ping_stats = test_ping_stats(&client, &server.base_url, args.ping_count, quiet).await?;

    // --- Download ---
    let down_speed = test_download(&client, &server.base_url, args.duration, quiet).await?;

    // --- Upload ---
    let up_speed = test_upload(&client, &server.base_url, args.duration, quiet).await?;

    // --- Summary ---
    if !quiet {
        println!();
        println!("╔══════════════════════════════════════╗");
        println!("║           📊 Test Summary            ║");
        println!("╠══════════════════════════════════════╣");
        println!("║  Server     : {:<23}║", server.name);
        println!("╠══════════════════════════════════════╣");
        println!("║  Ping       : {:<20} ms ║", format!("{:.1}", ping_stats.avg_ms));
        println!("║  Jitter     : {:<20} ms ║", format!("{:.2}", ping_stats.jitter_ms));
        println!("║  Min Ping   : {:<20} ms ║", ping_stats.min_ms);
        println!("║  Max Ping   : {:<20} ms ║", ping_stats.max_ms);
        println!("║  Packet Loss: {:<19} %  ║", format!("{:.1}", ping_stats.packet_loss_pct));
        println!("╠══════════════════════════════════════╣");
        println!("║  Download   : {:<18} Mbps ║", format!("{:.2}", down_speed));
        println!("║  Upload     : {:<18} Mbps ║", format!("{:.2}", up_speed));
        println!("╚══════════════════════════════════════╝");
    }

    Ok(SpeedTestResult {
        server_name: server.name,
        ping: ping_stats,
        download_mbps: down_speed,
        upload_mbps: up_speed,
    })
}

// 4. Helper to cleanly build hidden vs visible progress bars
fn create_spinner(msg: &str, quiet: bool, style_template: &str) -> ProgressBar {
    if quiet {
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

fn calculate_mbps(bytes: u64, duration_secs: f64) -> f64 {
    if duration_secs <= 0.0 {
        return 0.0;
    }
    let megabytes = (bytes as f64) / (1024.0 * 1024.0);
    (megabytes * 8.0) / duration_secs
}

/// Retries an async operation up to `max_retries` times with exponential backoff.
/// Delays: 100 ms, 200 ms, 400 ms — then gives up and surfaces the last error.
async fn with_retry<F, Fut, T>(max_retries: u32, mut f: F) -> anyhow::Result<T>
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

/// Sends `count` sequential HEAD requests and computes min, max, avg ping,
/// jitter (mean absolute deviation between consecutive samples), and packet loss.
async fn test_ping_stats(
    client: &Client,
    base_url: &str,
    count: u32,
    quiet: bool,
) -> anyhow::Result<PingStats> {
    let pb = create_spinner(
        "Measuring latency & jitter...",
        quiet,
        "{spinner:.cyan} {msg}",
    );

    let url = format!("{}/cdn-cgi/trace", base_url);
    let mut samples: Vec<u128> = Vec::with_capacity(count as usize);
    let mut lost: u32 = 0;

    for _ in 0..count {
        let start = Instant::now();
        // 2s timeout per ping — anything longer counts as lost
        match timeout(Duration::from_secs(2), client.head(&url).send()).await {
            Ok(Ok(_)) => samples.push(start.elapsed().as_millis()),
            _ => lost += 1,
        }
        // Small gap between pings, like real ping tools do
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    pb.finish_and_clear();

    if samples.is_empty() {
        anyhow::bail!("All ping attempts failed — server unreachable");
    }

    let min_ms = *samples.iter().min().unwrap();
    let max_ms = *samples.iter().max().unwrap();
    let avg_ms = samples.iter().sum::<u128>() as f64 / samples.len() as f64;

    // Jitter = mean of absolute differences between consecutive samples (RFC 3550 style)
    let jitter_ms = if samples.len() > 1 {
        let diffs: Vec<f64> = samples
            .windows(2)
            .map(|w| (w[1] as f64 - w[0] as f64).abs())
            .collect();
        diffs.iter().sum::<f64>() / diffs.len() as f64
    } else {
        0.0
    };

    let packet_loss_pct = (lost as f64 / count as f64) * 100.0;

    if !quiet {
        println!(
            "📡 Ping: {:.1} ms avg  |  Jitter: {:.2} ms  |  Loss: {:.1}%\n",
            avg_ms, jitter_ms, packet_loss_pct
        );
    }

    Ok(PingStats {
        min_ms,
        max_ms,
        avg_ms,
        jitter_ms,
        packet_loss_pct,
    })
}

async fn test_download(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    quiet: bool,
) -> anyhow::Result<f64> {
    let num_connections = 8;
    let chunk_size_bytes = 50 * 1024 * 1024;
    let total_downloaded = Arc::new(AtomicU64::new(0));

    let pb = create_spinner(
        "Downloading...",
        quiet,
        "{spinner:.green} [{elapsed_precise}] Downloading... {bytes} total ({bytes_per_sec})",
    );

    let mut tasks = vec![];
    let start = Instant::now();

    for _ in 0..num_connections {
        let client = client.clone();
        let pb = pb.clone();
        let total_downloaded = total_downloaded.clone();
        let url = format!("{}/__down?bytes={}", base_url, chunk_size_bytes);

        let task = tokio::spawn(async move {
            let download_logic = async {
                loop {
                    let res = with_retry(MAX_RETRIES, || async {
                        let r = client.get(&url).send().await?;
                        if !r.status().is_success() {
                            anyhow::bail!("Download request failed with status: {}", r.status());
                        }
                        Ok(r)
                    }).await?;
                    let mut stream = res.bytes_stream();
                    while let Some(item) = stream.next().await {
                        let chunk = item?;
                        let len = chunk.len() as u64;
                        // Always update progress bar so the user sees live activity
                        pb.inc(len);
                        // Only count bytes after warm-up to exclude TCP slow-start ramp
                        if start.elapsed().as_secs_f64() >= WARMUP_SECS {
                            total_downloaded.fetch_add(len, Ordering::Relaxed);
                        }
                    }
                }
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            };
            let _ = timeout(Duration::from_secs(duration_secs), download_logic).await;
            Ok::<(), anyhow::Error>(())
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await??;
    }

    let total_duration = start.elapsed().as_secs_f64();
    pb.finish_and_clear();

    // Subtract warm-up from the denominator so Mbps reflects only the plateau
    let effective_duration = (total_duration - WARMUP_SECS).max(0.0);
    Ok(calculate_mbps(
        total_downloaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

async fn test_upload(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    quiet: bool,
) -> anyhow::Result<f64> {
    let num_connections = 4;
    let chunk_size = 2 * 1024 * 1024;
    let total_uploaded = Arc::new(AtomicU64::new(0));
    let pb = create_spinner(
        "Uploading (random data)...",
        quiet,
        "{spinner:.red} [{elapsed_precise}] Uploading (random data)... {bytes} total ({bytes_per_sec})",
    );

    let mut tasks = vec![];
    let start = Instant::now();

    for _ in 0..num_connections {
        let client = client.clone();
        let pb = pb.clone();
        let total_uploaded = total_uploaded.clone();
        let url = format!("{}/__up", base_url);

        let task = tokio::spawn(async move {
            let upload_logic = async {
                let mut raw_payload = vec![0u8; chunk_size];
                rand::thread_rng().fill_bytes(&mut raw_payload);
                let payload = Bytes::from(raw_payload);

                loop {
                    let _ = with_retry(MAX_RETRIES, || async {
                        let r = client
                            .post(url.clone())
                            .body(payload.clone())
                            .send()
                            .await?;
                        if !r.status().is_success() {
                            anyhow::bail!("Upload request failed with status: {}", r.status());
                        }
                        Ok(r)
                    }).await?;
                    let len = payload.len() as u64;
                    // Always update progress bar so the user sees live activity
                    pb.inc(len);
                    // Only count bytes after warm-up to exclude TCP slow-start ramp
                    if start.elapsed().as_secs_f64() >= WARMUP_SECS {
                        total_uploaded.fetch_add(len, Ordering::Relaxed);
                    }
                }
                #[allow(unreachable_code)]
                Ok::<(), anyhow::Error>(())
            };
            let _ = timeout(Duration::from_secs(duration_secs), upload_logic).await;
            Ok::<(), anyhow::Error>(())
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await??;
    }

    let total_duration = start.elapsed().as_secs_f64();
    pb.finish_and_clear();

    // Subtract warm-up from the denominator so Mbps reflects only the plateau
    let effective_duration = (total_duration - WARMUP_SECS).max(0.0);
    Ok(calculate_mbps(
        total_uploaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_mbps() {
        let bytes = 13_107_200;
        let speed = calculate_mbps(bytes, 1.0);
        assert!((speed - 100.0).abs() < 0.001, "Speed was {}", speed);
    }

    #[test]
    fn test_calculate_mbps_zero_duration() {
        let speed = calculate_mbps(1000, 0.0);
        assert_eq!(speed, 0.0);
    }
}

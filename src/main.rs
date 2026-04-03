use clap::Parser;
use reqwest::Client;
use std::time::Duration;
use tracing::debug;
use chrono::Utc;

mod models;
mod utils;
mod client;

use models::{Server, SpeedTestResult};
use client::{test_ping_stats, test_download, test_upload};

const WARMUP_SECS: f64 = 2.0;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// A blazing fast CLI Speedtest written in Rust
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Duration of the download/upload tests in seconds
    #[arg(short, long, default_value_t = 10)]
    duration: u64,

    /// Number of parallel connections for testing (default: 8 for down, 4 for up)
    #[arg(short, long)]
    connections: Option<usize>,

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
    let down_connections = args.connections.unwrap_or(8);
    let down_speed = test_download(
        &client,
        &server.base_url,
        args.duration,
        down_connections,
        quiet,
    ).await?;

    // --- Upload ---
    let up_connections = args.connections.unwrap_or(4);
    let up_speed = test_upload(
        &client,
        &server.base_url,
        args.duration,
        up_connections,
        quiet,
    ).await?;

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
        timestamp: Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        server_name: server.name,
        ping: ping_stats,
        download_mbps: down_speed,
        upload_mbps: up_speed,
    })
}

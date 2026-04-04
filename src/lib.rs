// src/lib.rs

pub mod client;
pub mod models;
pub mod utils;

use chrono::Utc;
use models::{AppConfig, RunArgs, Server, SpeedTestResult};
use reqwest::Client;
use std::sync::Arc;
use utils::WARMUP_SECS;

const DEFAULT_SERVER_URL: &str = "https://speed.cloudflare.com";

/// Core application logic — fully decoupled from clap so integration tests can
/// call it directly with a mockito server URL via `RunArgs::server_url`.
pub async fn run(
    args: RunArgs,
    config: Arc<AppConfig>,
    client: Client,
) -> anyhow::Result<SpeedTestResult> {
    if args.duration_secs <= WARMUP_SECS as u64 {
        anyhow::bail!(
            "Duration must be greater than {} seconds (warm-up period). Got: {}s",
            WARMUP_SECS,
            args.duration_secs
        );
    }

    if args.ping_count == 0 {
        anyhow::bail!("--ping-count must be at least 1");
    }

    // Derive display name: if the URL is the default, label it "Cloudflare";
    // otherwise show the URL itself so users know which custom server was used.
    let server = Server {
        name: if args.server_url == DEFAULT_SERVER_URL {
            "Cloudflare".into()
        } else {
            args.server_url.clone()
        },
        base_url: args.server_url.clone(),
    };

    if !config.quiet {
        println!("🔍 Using server: {}\n", server.name);
    }

    // --- Ping / Jitter / Packet Loss ---
    let ping_stats = client::test_ping_stats(
        &client,
        &server.base_url,
        args.ping_count,
        Arc::clone(&config),
    )
    .await?;

    // --- Download (skipped if --no-download) ---
    let down_speed: Option<f64> = if args.no_download {
        if !config.quiet {
            println!("⬇️  Download: skipped\n");
        }
        None
    } else {
        let conns = args.connections.unwrap_or(8);
        let speed = client::test_download(
            &client,
            &server.base_url,
            args.duration_secs,
            conns,
            Arc::clone(&config),
        )
        .await?;
        if !config.quiet {
            println!("⬇️  Download Speed: {:.2} Mbps\n", speed);
        }
        Some(speed)
    };

    // --- Upload (skipped if --no-upload) ---
    let up_speed: Option<f64> = if args.no_upload {
        if !config.quiet {
            println!("⬆️  Upload: skipped\n");
        }
        None
    } else {
        let conns = args.connections.unwrap_or(4);
        let speed = client::test_upload(
            &client,
            &server.base_url,
            args.duration_secs,
            conns,
            Arc::clone(&config),
        )
        .await?;
        if !config.quiet {
            println!("⬆️  Upload Speed: {:.2} Mbps\n", speed);
        }
        Some(speed)
    };

    // --- Summary box ---
    if !config.quiet {
        println!("╔══════════════════════════════════════╗");
        println!("║           📊 Test Summary            ║");
        println!("╠══════════════════════════════════════╣");
        println!("║  Server     : {:<23}║", server.name);
        println!("╠══════════════════════════════════════╣");
        println!(
            "║  Ping       : {:<20} ms ║",
            format!("{:.1}", ping_stats.avg_ms)
        );
        println!(
            "║  Jitter     : {:<20} ms ║",
            format!("{:.2}", ping_stats.jitter_ms)
        );
        println!("║  Min Ping   : {:<20} ms ║", ping_stats.min_ms);
        println!("║  Max Ping   : {:<20} ms ║", ping_stats.max_ms);
        println!(
            "║  Packet Loss: {:<19} %  ║",
            format!("{:.1}", ping_stats.packet_loss_pct)
        );
        println!("╠══════════════════════════════════════╣");
        match down_speed {
            Some(s) => println!("║  Download   : {:<18} Mbps ║", format!("{:.2}", s)),
            None => println!("║  Download   : {:<23}║", "skipped"),
        }
        match up_speed {
            Some(s) => println!("║  Upload     : {:<18} Mbps ║", format!("{:.2}", s)),
            None => println!("║  Upload     : {:<23}║", "skipped"),
        }
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

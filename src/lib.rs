// src/lib.rs

pub mod client;
pub mod cooldown;
pub mod menu;
pub mod models;
pub mod theme;
pub mod utils;

use chrono::Utc;
use models::{AppConfig, RunArgs, Server, SpeedTestResult};
use reqwest::Client;
use std::sync::Arc;
use utils::{NonRetryableError, WARMUP_SECS};

const DEFAULT_SERVER_URL: &str = "https://speed.cloudflare.com";

async fn run_with_fallback_concurrency<F, Fut>(
    initial_conns: usize,
    config: Arc<AppConfig>,
    test_name: &str,
    mut f: F,
) -> anyhow::Result<f64>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<f64>>,
{
    match f(initial_conns).await {
        Ok(speed) => Ok(speed),
        Err(e) => {
            let is_rate_limit = e.downcast_ref::<NonRetryableError>().is_some();
            if is_rate_limit && initial_conns > 1 {
                if !config.quiet {
                    eprintln!(
                        "⚠️  {} rate limited at {} connections — retrying \
                         with 1 connection…",
                        test_name, initial_conns
                    );
                }
                f(1).await
            } else {
                Err(e)
            }
        }
    }
}

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
        let conns = args.connections.unwrap_or(4);
        let config_clone = Arc::clone(&config);
        let speed = run_with_fallback_concurrency(conns, config_clone, "Download", |c| {
            client::test_download(
                &client,
                &server.base_url,
                args.duration_secs,
                c,
                Arc::clone(&config),
            )
        })
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
        let conns = args.connections.unwrap_or(2);
        let config_clone = Arc::clone(&config);
        let speed = run_with_fallback_concurrency(conns, config_clone, "Upload", |c| {
            client::test_upload(
                &client,
                &server.base_url,
                args.duration_secs,
                c,
                Arc::clone(&config),
            )
        })
        .await?;
        if !config.quiet {
            println!("⬆️  Upload Speed: {:.2} Mbps\n", speed);
        }
        Some(speed)
    };

    // --- Summary box ---
    if !config.quiet {
        let term_cols = console::Term::stdout().size().1 as usize;
        let box_width = term_cols.saturating_sub(4).clamp(44, 60);
        let inner_width = box_width - 2;

        println!("╔{}╗", "═".repeat(inner_width));
        println!("║{:^width$}║", "📊 Test Summary", width = inner_width);
        println!("╠{}╣", "═".repeat(inner_width));

        // Server Row
        let server_label = "  Server     : ";
        let server_val_width = inner_width - server_label.len() - 1;
        let truncated_server = theme::truncate_to(&server.name, server_val_width);
        println!(
            "║{}{} ║",
            server_label,
            theme::pad_to(&truncated_server, server_val_width),
        );

        println!("╠{}╣", "═".repeat(inner_width));

        // Ping Stats Rows
        let labels = [
            (
                "  Ping       : ",
                theme::color_ping(ping_stats.avg_ms, &config),
            ),
            (
                "  Jitter     : ",
                theme::color_jitter(ping_stats.jitter_ms, &config),
            ),
            ("  Min Ping   : ", format!("{} ms", ping_stats.min_ms)),
            ("  Max Ping   : ", format!("{} ms", ping_stats.max_ms)),
            (
                "  Packet Loss: ",
                theme::color_loss(ping_stats.packet_loss_pct, &config),
            ),
        ];

        for (label, val) in labels {
            let val_width = inner_width - label.len() - 1;
            println!("║{}{} ║", label, theme::pad_to(&val, val_width));
        }

        println!("╠{}╣", "═".repeat(inner_width));

        // Download Row
        match down_speed {
            Some(s) => {
                let label = "  Download   : ";
                let speed_str = theme::color_speed(s, &config);
                let rating = theme::speed_rating(s, &config);
                let combined = format!("{}  {}", speed_str, rating);
                let val_width = inner_width - label.len() - 1;
                println!("║{}{} ║", label, theme::pad_to(&combined, val_width));
            }
            None => {
                let label = "  Download   : ";
                let val_width = inner_width - label.len() - 1;
                println!("║{}{} ║", label, theme::pad_to("skipped", val_width));
            }
        }

        // Upload Row
        match up_speed {
            Some(s) => {
                let label = "  Upload     : ";
                let speed_str = theme::color_speed(s, &config);
                let rating = theme::speed_rating(s, &config);
                let combined = format!("{}  {}", speed_str, rating);
                let val_width = inner_width - label.len() - 1;
                println!("║{}{} ║", label, theme::pad_to(&combined, val_width));
            }
            None => {
                let label = "  Upload     : ";
                let val_width = inner_width - label.len() - 1;
                println!("║{}{} ║", label, theme::pad_to("skipped", val_width));
            }
        }

        println!("╚{}╝", "═".repeat(inner_width));
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

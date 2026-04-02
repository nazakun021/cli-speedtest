use bytes::Bytes;
use clap::Parser;
use futures::future::join_all;
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
    ping_ms: u128,
    download_mbps: f64,
    upload_mbps: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize the logger. If --debug is passed, show debug logs. Otherwise, hide them.
    let log_level = if args.debug { "debug" } else { "error" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_writer(std::io::stderr) // Write logs to stderr so it doesn't break JSON stdout
        .init();

    debug!("Application started with args: {:?}", args);

    // Setting a User-Agent is good practice and prevents many CDNs from blocking requests
    let client = Client::builder()
        .user_agent("rust-speedtest/0.1.0")
        .build()?;

    // We use tokio::select! to race our application logic against a Ctrl+C signal
    tokio::select! {
        // Branch 1: The normal application execution
        res = run_app(args.clone(), client) => {
            match res {
                Ok(result) => {
                    // 2. If --json is passed, print ONLY the JSON string
                    if args.json {
                        let json_out = serde_json::to_string_pretty(&result)?;
                        println!("{}", json_out);
                    }
                }
                Err(e) => {
                    // Output errors in JSON format if requested
                    if args.json {
                        println!(r#"{{"error": "{}"}}"#, e);
                    } else {
                        eprintln!("❌ Error: {}", e);
                    }
                }
            }
        }

        // Branch 2: The user presses Ctrl+C
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

// Your existing main logic simply moves in here
async fn run_app(args: Args, client: Client) -> anyhow::Result<SpeedTestResult> {
    let quiet = args.json;

    if !quiet {
        println!("🚀 Starting Rust Speedtest...\n");
    }

    let server_pool = vec![
        Server {
            name: "Cloudflare (Global)".into(),
            base_url: "https://speed.cloudflare.com".into(),
        },
        Server {
            name: "Cloudflare (Alternative)".into(),
            base_url: "https://speed.cloudflare.com".into(),
        },
    ];

    let best_server = find_best_server(&client, server_pool, quiet).await?;

    let ping_latency = test_ping(&client, &best_server.base_url, quiet).await?;
    if !quiet {
        println!("📡 Ping: {} ms\n", ping_latency);
    }

    let down_speed = test_download(&client, &best_server.base_url, args.duration, quiet).await?;
    if !quiet {
        println!("⬇️  Download Speed: {:.2} Mbps\n", down_speed);
    }

    let up_speed = test_upload(&client, &best_server.base_url, args.duration, quiet).await?;
    if !quiet {
        println!("⬆️  Upload Speed: {:.2} Mbps\n", up_speed);
        println!("--------------------------------------");
        println!("🏁 Test Complete!");
        println!("--------------------------------------");
    }
    Ok(SpeedTestResult {
        server_name: best_server.name,
        ping_ms: ping_latency,
        download_mbps: down_speed,
        upload_mbps: up_speed,
    })
}

async fn find_best_server(
    client: &Client,
    servers: Vec<Server>,
    quiet: bool,
) -> anyhow::Result<Server> {
    if !quiet {
        println!("🔍 Finding best server...");
    }

    let mut ping_tasks = vec![];

    for server in servers {
        let client = client.clone();
        let server_node = server.clone();

        // Task: Measure latency for this specific server
        let task = tokio::spawn(async move {
            let start = Instant::now();
            let url = format!("{}/cdn-cgi/trace", server_node.base_url);
            let res = client.head(&url).send().await;

            match res {
                Ok(_) => {
                    let latency = start.elapsed().as_millis();
                    Ok::<(u128, Server), anyhow::Error>((latency, server_node))
                }
                Err(e) => Err(anyhow::anyhow!("Server {} failed: {}", server_node.name, e)),
            }
        });
        ping_tasks.push(task);
    }

    // Wait for all pings to return
    let results = join_all(ping_tasks).await;

    // Filter out errors and find the one with the minimum latency
    let best = results
        .into_iter()
        .filter_map(|res| res.ok().and_then(|inner| inner.ok()))
        .min_by_key(|(latency, _)| *latency);

    match best {
        Some((latency, server)) => {
            if !quiet {
                println!("✅ Selected {} ({} ms)", server.name, latency);
            }
            Ok(server)
        }
        None => Err(anyhow::anyhow!("No servers were reachable")),
    }
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

async fn test_ping(client: &Client, base_url: &str, quiet: bool) -> anyhow::Result<u128> {
    let pb = create_spinner("Measuring latency...", quiet, "{spinner:.cyan} {msg}");

    let start = Instant::now();
    // Use the dynamic URL
    let url = format!("{}/cdn-cgi/trace", base_url);
    client.get(&url).send().await?;
    let duration = start.elapsed();

    pb.finish_and_clear();
    Ok(duration.as_millis())
}

async fn test_download(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    quiet: bool,
) -> anyhow::Result<f64> {
    let num_connections = 8;
    // We request 50MB per connection at a time to ensure the server doesn't reject the size
    let chunk_size_bytes = 50 * 1024 * 1024;

    // We use an Atomic counter to thread-safely tally the total bytes downloaded
    let total_downloaded = Arc::new(AtomicU64::new(0));

    // Because we don't know the final size, we switch from a ProgressBar to a Spinner
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
                    let res = client.get(&url).send().await?;
                    if !res.status().is_success() {
                        anyhow::bail!("Download request failed with status: {}", res.status());
                    }

                    let mut stream = res.bytes_stream();
                    while let Some(item) = stream.next().await {
                        let chunk = item?;
                        let len = chunk.len() as u64;
                        pb.inc(len);
                        total_downloaded.fetch_add(len, Ordering::Relaxed);
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

    // Wait for all Tokio tasks to finish
    for task in tasks {
        task.await??; // This will now correctly propagate errors if any inner task failed
    }

    // Stop the timer and clear the progress bar
    let duration = start.elapsed().as_secs_f64();
    pb.finish_and_clear();

    // Calculate final Mbps using our pure function
    let speed_mbps = calculate_mbps(total_downloaded.load(Ordering::Relaxed), duration);

    Ok(speed_mbps)
}

async fn test_upload(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    quiet: bool,
) -> anyhow::Result<f64> {
    let num_connections = 4;
    let chunk_size = 2 * 1024 * 1024; // 2MB chunks
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

                // Convert the Vec to a cheap-to-clone Bytes object
                let payload = Bytes::from(raw_payload);

                loop {
                    let res = client
                        .post(url.clone())
                        .body(payload.clone())
                        .send()
                        .await?;
                    if !res.status().is_success() {
                        anyhow::bail!("Upload request failed with status: {}", res.status());
                    }

                    let len = payload.len() as u64;
                    pb.inc(len);
                    total_uploaded.fetch_add(len, Ordering::Relaxed);
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
        let _ = task.await?;
    }

    let duration = start.elapsed().as_secs_f64();
    pb.finish_and_clear();

    let speed_mbps = calculate_mbps(total_uploaded.load(Ordering::Relaxed), duration);

    Ok(speed_mbps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_mbps() {
        // If we download 12.5 MiB in 1 second, that's exactly 100 Mbps
        // 12.5 * 1024 * 1024 = 13,107,200 bytes
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

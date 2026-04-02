use bytes::Bytes;
use clap::Parser;
use futures::future::join_all;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rand::RngCore;
use reqwest::Client;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// A blazing fast CLI Speedtest written in Rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Duration of the download/upload tests in seconds
    #[arg(short, long, default_value_t = 10)]
    duration: u64,
}

#[derive(Debug, Clone)]
struct Server {
    name: String,
    base_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Setting a User-Agent is good practice and prevents many CDNs from blocking requests
    let client = Client::builder()
        .user_agent("rust-speedtest/0.1.0")
        .build()?;

    // We use tokio::select! to race our application logic against a Ctrl+C signal
    tokio::select! {
        // Branch 1: The normal application execution
        res = run_app(args, client) => {
            // If run_app finishes normally (or with an error), return its result
            res?;
        }

        // Branch 2: The user presses Ctrl+C
        _ = tokio::signal::ctrl_c() => {
            // 1. \r\x1b[2K clears the current terminal line (removing broken progress bars)
            // 2. \x1b[?25h is the ANSI escape code to un-hide the terminal cursor!
            print!("\r\x1b[2K\x1b[?25h");
            println!("⚠️  Speedtest aborted by user.");

            // Force exit so background tasks don't accidentally print more frames
            std::process::exit(130);
        }
    }

    Ok(())
}

// Your existing main logic simply moves in here
async fn run_app(args: Args, client: Client) -> anyhow::Result<()> {
    println!("🚀 Starting Rust Speedtest...\n");

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

    let best_server = find_best_server(&client, server_pool).await?;
    // Use best_server.url for your download/upload tests!

    // 1. PING TEST
    let ping_latency = test_ping(&client, &best_server.base_url).await?;
    println!("📡 Ping: {} ms\n", ping_latency);

    // 2. DOWNLOAD TEST
    let down_speed = test_download(&client, &best_server.base_url, args.duration).await?;
    println!("⬇️  Download Speed: {:.2} Mbps\n", down_speed);

    // 3. UPLOAD TEST
    let up_speed = test_upload(&client, &best_server.base_url, args.duration).await?;
    println!("⬆️  Upload Speed: {:.2} Mbps\n", up_speed);

    println!("--------------------------------------");
    println!("🏁 Test Complete!");
    println!("--------------------------------------");

    Ok(())
}

async fn test_ping(client: &Client, base_url: &str) -> anyhow::Result<u128> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_message("Measuring latency...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let start = Instant::now();
    // Use the dynamic URL
    let url = format!("{}/cdn-cgi/trace", base_url);
    client.get(&url).send().await?;
    let duration = start.elapsed();

    spinner.finish_and_clear();
    Ok(duration.as_millis())
}

async fn test_download(client: &Client, base_url: &str, duration_secs: u64) -> anyhow::Result<f64> {
    let num_connections = 8;
    // We request 50MB per connection at a time to ensure the server doesn't reject the size
    let chunk_size_bytes = 50 * 1024 * 1024;

    // We use an Atomic counter to thread-safely tally the total bytes downloaded
    let total_downloaded = Arc::new(AtomicU64::new(0));

    // Because we don't know the final size, we switch from a ProgressBar to a Spinner
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner().template(
        "{spinner:.green} [{elapsed_precise}] Downloading... {bytes} total ({bytes_per_sec})",
    )?);
    pb.enable_steady_tick(Duration::from_millis(100));

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

    // Calculate final Mbps based on exactly how many bytes we managed to grab
    let final_bytes = total_downloaded.load(Ordering::Relaxed) as f64;
    let final_megabytes = final_bytes / (1024.0 * 1024.0);
    let speed_mbps = (final_megabytes * 8.0) / duration;

    Ok(speed_mbps)
}

async fn test_upload(client: &Client, base_url: &str, duration_secs: u64) -> anyhow::Result<f64> {
    let num_connections = 4;
    let chunk_size = 2 * 1024 * 1024; // 2MB chunks
    let total_uploaded = Arc::new(AtomicU64::new(0));

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.red} [{elapsed_precise}] Uploading (random data)... {bytes} total ({bytes_per_sec})")?);
    pb.enable_steady_tick(Duration::from_millis(100));

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

    let final_bytes = total_uploaded.load(Ordering::Relaxed) as f64;
    let final_megabytes = final_bytes / (1024.0 * 1024.0);
    let speed_mbps = (final_megabytes * 8.0) / duration;

    Ok(speed_mbps)
}

async fn find_best_server(client: &Client, servers: Vec<Server>) -> anyhow::Result<Server> {
    println!("🔍 Finding best server...");

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
            println!("✅ Selected {} ({} ms)", server.name, latency);
            Ok(server)
        }
        None => Err(anyhow::anyhow!("No servers were reachable")),
    }
}

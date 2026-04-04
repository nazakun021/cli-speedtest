// src/client.rs

use bytes::Bytes;
use futures_util::StreamExt;
use rand::RngCore;
use reqwest::Client;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio_util::sync::CancellationToken;

use crate::models::{AppConfig, PingStats};
use crate::utils::{WARMUP_SECS, calculate_mbps, create_spinner, with_retry};

pub async fn test_ping_stats(
    client: &Client,
    base_url: &str,
    count: u32,
    config: Arc<AppConfig>,
) -> anyhow::Result<PingStats> {
    let pb = create_spinner(
        "Measuring latency & jitter...",
        &config,
        "{spinner:.cyan} {msg}",
    );

    let url = format!("{}/cdn-cgi/trace", base_url);
    let mut samples: Vec<u128> = Vec::with_capacity(count as usize);
    let mut lost: u32 = 0;

    for _ in 0..count {
        let start = Instant::now();
        match tokio::time::timeout(Duration::from_secs(2), client.head(&url).send()).await {
            Ok(Ok(_)) => samples.push(start.elapsed().as_millis()),
            _ => lost += 1,
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    pb.finish_and_clear();

    if samples.is_empty() {
        anyhow::bail!("All ping attempts failed — server unreachable");
    }

    let min_ms = *samples.iter().min().unwrap();
    let max_ms = *samples.iter().max().unwrap();
    let avg_ms = samples.iter().sum::<u128>() as f64 / samples.len() as f64;

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

    if !config.quiet {
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

pub async fn test_download(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    num_connections: usize,
    config: Arc<AppConfig>,
) -> anyhow::Result<f64> {
    let chunk_size_bytes = 50 * 1024 * 1024;
    let total_downloaded = Arc::new(AtomicU64::new(0));

    let pb = create_spinner(
        "Downloading...",
        &config,
        "{spinner:.green} [{elapsed_precise}] Downloading... {bytes} total ({bytes_per_sec})",
    );

    let token = CancellationToken::new();
    let barrier = Arc::new(Barrier::new(num_connections));
    let shared_start: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let mut tasks = vec![];

    for _ in 0..num_connections {
        let client = client.clone();
        let pb = pb.clone();
        let total_downloaded = total_downloaded.clone();
        let url = format!("{}/__down?bytes={}", base_url, chunk_size_bytes);
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();
        let token = token.clone();

        let task = tokio::spawn(async move {
            barrier.wait().await;
            let start = *shared_start.get_or_init(Instant::now);

            // Outer loop: request a new chunk file when the previous one finishes.
            'request: loop {
                if token.is_cancelled() {
                    break;
                }

                let res = match with_retry(3, || async {
                    let r = client.get(&url).send().await?;
                    if !r.status().is_success() {
                        anyhow::bail!("Download failed with status: {}", r.status());
                    }
                    Ok(r)
                })
                .await
                {
                    Ok(r) => r,
                    Err(e) => return Err(e),
                };

                let mut stream = res.bytes_stream();

                // Inner loop: drain the stream, but yield to the token on every chunk.
                loop {
                    tokio::select! {
                        biased; // check cancellation first to avoid polling a dead stream
                        _ = token.cancelled() => break 'request,
                        item = stream.next() => {
                            match item {
                                Some(Ok(chunk)) => {
                                    let len = chunk.len() as u64;
                                    pb.inc(len);
                                    if start.elapsed().as_secs_f64() >= WARMUP_SECS {
                                        total_downloaded.fetch_add(len, Ordering::Relaxed);
                                    }
                                }
                                Some(Err(e)) => return Err(e.into()),
                                // Stream exhausted — loop back and request the next chunk
                                None => break,
                            }
                        }
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
            // No #[allow(unreachable_code)] needed — every exit path is explicit
        });

        tasks.push(task);
    }

    // Sleep for the test window, then signal all workers to stop cleanly.
    tokio::time::sleep(Duration::from_secs(duration_secs)).await;
    token.cancel();

    for task in tasks {
        task.await??;
    }

    pb.finish_and_clear();

    let start = shared_start.get().copied().unwrap_or_else(Instant::now);
    let effective_duration = (start.elapsed().as_secs_f64() - WARMUP_SECS).max(0.0);
    Ok(calculate_mbps(
        total_downloaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

pub async fn test_upload(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    num_connections: usize,
    config: Arc<AppConfig>,
) -> anyhow::Result<f64> {
    let chunk_size = 2 * 1024 * 1024;
    let total_uploaded = Arc::new(AtomicU64::new(0));

    let pb = create_spinner(
        "Uploading...",
        &config,
        "{spinner:.red} [{elapsed_precise}] Uploading... {bytes} total ({bytes_per_sec})",
    );

    let token = CancellationToken::new();
    let barrier = Arc::new(Barrier::new(num_connections));
    let shared_start: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let mut tasks = vec![];

    for _ in 0..num_connections {
        let client = client.clone();
        let pb = pb.clone();
        let total_uploaded = total_uploaded.clone();
        let url = format!("{}/__up", base_url);
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();
        let token = token.clone();

        let task = tokio::spawn(async move {
            barrier.wait().await;
            let start = *shared_start.get_or_init(Instant::now);

            // Generate the payload once per connection and reuse it across requests.
            let mut raw_payload = vec![0u8; chunk_size];
            rand::rng().fill_bytes(&mut raw_payload);
            let payload = Bytes::from(raw_payload);

            loop {
                if token.is_cancelled() {
                    break;
                }

                match with_retry(3, || async {
                    let r = client
                        .post(url.clone())
                        .body(payload.clone())
                        .send()
                        .await?;
                    if !r.status().is_success() {
                        anyhow::bail!("Upload failed with status: {}", r.status());
                    }
                    Ok(r)
                })
                .await
                {
                    Ok(_) => {
                        let len = payload.len() as u64;
                        pb.inc(len);
                        if start.elapsed().as_secs_f64() >= WARMUP_SECS {
                            total_uploaded.fetch_add(len, Ordering::Relaxed);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }

            Ok::<(), anyhow::Error>(())
            // No #[allow(unreachable_code)] needed — token drives the exit
        });

        tasks.push(task);
    }

    tokio::time::sleep(Duration::from_secs(duration_secs)).await;
    token.cancel();

    for task in tasks {
        task.await??;
    }

    pb.finish_and_clear();

    let start = shared_start.get().copied().unwrap_or_else(Instant::now);
    let effective_duration = (start.elapsed().as_secs_f64() - WARMUP_SECS).max(0.0);
    Ok(calculate_mbps(
        total_uploaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

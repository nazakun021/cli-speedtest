use bytes::Bytes;
use futures_util::StreamExt;
use rand::RngCore;
use reqwest::Client;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::timeout;

use crate::models::PingStats;
use crate::utils::{calculate_mbps, create_spinner, with_retry};

const WARMUP_SECS: f64 = 2.0;

/// Sends `count` sequential HEAD requests and computes min, max, avg ping,
/// jitter (mean absolute deviation between consecutive samples), and packet loss.
pub async fn test_ping_stats(
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

pub async fn test_download(
    client: &Client,
    base_url: &str,
    duration_secs: u64,
    num_connections: usize,
    quiet: bool,
) -> anyhow::Result<f64> {
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
                    let res = with_retry(3, || async {
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
                        pb.inc(len);
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

    let effective_duration = (total_duration - WARMUP_SECS).max(0.0);
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
    quiet: bool,
) -> anyhow::Result<f64> {
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
                    let _ = with_retry(3, || async {
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
                    pb.inc(len);
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

    let effective_duration = (total_duration - WARMUP_SECS).max(0.0);
    Ok(calculate_mbps(
        total_uploaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

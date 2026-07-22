// src/client.rs

use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::HumanBytes;
use rand::{Rng, RngCore};
use reqwest::Client;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Barrier, mpsc};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::models::{AppConfig, PingStats};
use crate::theme;
use crate::utils::{
    LOW_SPEED_THRESHOLD_MBPS, LOW_SPEED_TIMEOUT, NonRetryableError, calculate_mbps, create_spinner,
    with_retry,
};

// src/client.rs - shared helper used in both test_download and test_upload
fn check_status(r: &reqwest::Response) -> anyhow::Result<()> {
    match r.status() {
        s if s.is_success() => Ok(()),

        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            let (wait_secs, source) = r
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|s| (s, "server says"))
                .unwrap_or((900, "estimated - no Retry-After header"));

            Err(anyhow::Error::new(NonRetryableError(anyhow::anyhow!(
                "You've been rate-limited by Cloudflare. \
                 Please wait {} minutes ({}).\n\n\
                 Alternatives:\n  \
                 - Use a custom server:  cli-speedtest --server <URL>\n  \
                 - Run ping only:        cli-speedtest --no-download --no-upload\n  \
                 - Force immediate run:  cli-speedtest --force-run",
                wait_secs / 60,
                source
            ))))
        }

        reqwest::StatusCode::FORBIDDEN => {
            Err(anyhow::Error::new(NonRetryableError(anyhow::anyhow!(
                "Cloudflare returned 403 Forbidden. Your IP may have \
                 triggered Bot Fight Mode. Wait 15 minutes or switch \
                 servers with: speedtest --server <URL>"
            ))))
        }

        s => anyhow::bail!("Request failed with status: {}", s),
    }
}

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
            Ok(Ok(response)) if response.status().is_success() => {
                samples.push(start.elapsed().as_millis())
            }
            _ => lost += 1,
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    pb.finish_and_clear();

    if samples.is_empty() {
        anyhow::bail!("All ping attempts failed - server unreachable");
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
            "Ping: {} avg  |  Jitter: {}  |  Loss: {}\n",
            theme::color_ping(avg_ms, &config),
            theme::color_jitter(jitter_ms, &config),
            theme::color_loss(packet_loss_pct, &config)
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
    warmup_secs: f64,
    config: Arc<AppConfig>,
) -> anyhow::Result<f64> {
    let chunk_size_bytes = 50 * 1024 * 1024;
    let total_downloaded = Arc::new(AtomicU64::new(0));

    let pb = create_spinner(
        "Downloading...",
        &config,
        "{spinner:.green} [{elapsed_precise}] {msg}",
    );

    let token = CancellationToken::new();
    let barrier = Arc::new(Barrier::new(num_connections + 1)); // +1 for the display task
    let shared_start: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let mut tasks = JoinSet::new();
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();

    // Worker tasks
    for _ in 0..num_connections {
        let client = client.clone();
        let total_downloaded = total_downloaded.clone();
        let url = format!("{}/__down?bytes={}", base_url, chunk_size_bytes);
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();
        let token = token.clone();
        let progress_tx = progress_tx.clone();

        tasks.spawn(async move {
            barrier.wait().await;
            let start = *shared_start.get_or_init(Instant::now);

            'request: loop {
                if token.is_cancelled() {
                    break;
                }

                let res = match with_retry(3, || async {
                    let r = client.get(&url).send().await?;
                    check_status(&r)?;
                    Ok(r)
                })
                .await
                {
                    Ok(r) => r,
                    Err(e) => return Err(e),
                };

                let mut stream = res.bytes_stream();

                loop {
                    tokio::select! {
                        biased;
                        _ = token.cancelled() => break 'request,
                        item = stream.next() => {
                            match item {
                                Some(Ok(chunk)) => {
                                    let len = chunk.len() as u64;
                                    let _ = progress_tx.send(len);
                                    if start.elapsed().as_secs_f64() >= warmup_secs {
                                        total_downloaded.fetch_add(len, Ordering::Relaxed);
                                    }
                                }
                                Some(Err(e)) => return Err(e.into()),
                                None => break,
                            }
                        }
                    }
                }

                let jitter_ms = rand::rng().random_range(50u64..=150);
                tokio::time::sleep(std::time::Duration::from_millis(jitter_ms)).await;
            }

            Ok::<(), anyhow::Error>(())
        });
    }
    drop(progress_tx);

    // Display task
    let display_task = {
        let pb = pb.clone();
        let total_downloaded = total_downloaded.clone();
        let token = token.clone();
        let config = config.clone();
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();

        tokio::spawn(async move {
            barrier.wait().await;
            let mut prev_bytes = 0;
            let mut prev_instant = Instant::now();
            let mut low_speed_since: Option<Instant> = None;
            let mut progress_channel_open = true;

            loop {
                tokio::select! {
                    _ = token.cancelled() => break Ok(()),
                    progress = progress_rx.recv(), if progress_channel_open => {
                        match progress {
                            Some(bytes) => pb.inc(bytes),
                            None => progress_channel_open = false,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {
                        let now_bytes = total_downloaded.load(Ordering::Relaxed);
                        let delta = now_bytes.saturating_sub(prev_bytes);
                        let elapsed = prev_instant.elapsed().as_secs_f64();
                        let speed = calculate_mbps(delta, elapsed);

                        let start_elapsed = shared_start.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);

                        if speed < LOW_SPEED_THRESHOLD_MBPS && start_elapsed > warmup_secs {
                            if let Some(since) = low_speed_since {
                                if since.elapsed() > LOW_SPEED_TIMEOUT {
                                    return Err(anyhow::anyhow!(
                                        "Connection too slow (below {:.2} Mbps for {}s). \
                                         Aborting test.",
                                        LOW_SPEED_THRESHOLD_MBPS,
                                        LOW_SPEED_TIMEOUT.as_secs()
                                    ));
                                }
                            } else {
                                low_speed_since = Some(Instant::now());
                            }
                        } else {
                            low_speed_since = None;
                        }

                        let speed_str = if speed == 0.0 && now_bytes == 0 {
                            "↓  --.- Mbps".to_string()
                        } else {
                            format!("↓  {}", theme::color_speed(speed, &config))
                        };

                        pb.set_message(format!(
                            "{}    {} total",
                            speed_str,
                            HumanBytes(now_bytes)
                        ));

                        prev_bytes = now_bytes;
                        prev_instant = Instant::now();
                    }
                }
            }
        })
    };

    let mut display_handle = display_task;
    let measurement_result = tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(duration_secs)) => Ok(()),
        task_result = tasks.join_next() => match task_result {
            Some(Ok(Err(error))) => Err(error),
            Some(Ok(Ok(()))) => Err(anyhow::anyhow!("Download worker ended before the test completed")),
            Some(Err(error)) => Err(error.into()),
            None => Err(anyhow::anyhow!("No download workers were started")),
        },
        display_result = &mut display_handle => match display_result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(())) => Err(anyhow::anyhow!("Download display ended before the test completed")),
            Err(error) => Err(error.into()),
        },
    };

    token.cancel();
    tasks.shutdown().await;

    if let Err(error) = measurement_result {
        display_handle.abort();
        pb.finish_and_clear();
        return Err(error);
    }

    display_handle.await??;

    pb.finish_and_clear();

    let start = shared_start.get().copied().unwrap_or_else(Instant::now);
    let effective_duration = (start.elapsed().as_secs_f64() - warmup_secs).max(0.0);
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
    warmup_secs: f64,
    config: Arc<AppConfig>,
) -> anyhow::Result<f64> {
    let chunk_size = 2 * 1024 * 1024;
    let total_uploaded = Arc::new(AtomicU64::new(0));

    let pb = create_spinner(
        "Uploading...",
        &config,
        "{spinner:.red} [{elapsed_precise}] {msg}",
    );

    let token = CancellationToken::new();
    let barrier = Arc::new(Barrier::new(num_connections + 1));
    let shared_start: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let mut tasks = JoinSet::new();
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();

    // Worker tasks
    for _ in 0..num_connections {
        let client = client.clone();
        let total_uploaded = total_uploaded.clone();
        let url = format!("{}/__up", base_url);
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();
        let token = token.clone();
        let progress_tx = progress_tx.clone();

        tasks.spawn(async move {
            barrier.wait().await;
            let start = *shared_start.get_or_init(Instant::now);

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
                    check_status(&r)?;
                    Ok(r)
                })
                .await
                {
                    Ok(_) => {
                        let len = payload.len() as u64;
                        let _ = progress_tx.send(len);
                        if start.elapsed().as_secs_f64() >= warmup_secs {
                            total_uploaded.fetch_add(len, Ordering::Relaxed);
                        }
                    }
                    Err(e) => return Err(e),
                }

                let jitter_ms = rand::rng().random_range(50u64..=150);
                tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
            }

            Ok::<(), anyhow::Error>(())
        });
    }
    drop(progress_tx);

    // Display task
    let display_task = {
        let pb = pb.clone();
        let total_uploaded = total_uploaded.clone();
        let token = token.clone();
        let config = config.clone();
        let barrier = barrier.clone();
        let shared_start = shared_start.clone();

        tokio::spawn(async move {
            barrier.wait().await;
            let mut prev_bytes = 0;
            let mut prev_instant = Instant::now();
            let mut low_speed_since: Option<Instant> = None;
            let mut progress_channel_open = true;

            loop {
                tokio::select! {
                    _ = token.cancelled() => break Ok(()),
                    progress = progress_rx.recv(), if progress_channel_open => {
                        match progress {
                            Some(bytes) => pb.inc(bytes),
                            None => progress_channel_open = false,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {
                        let now_bytes = total_uploaded.load(Ordering::Relaxed);
                        let delta = now_bytes.saturating_sub(prev_bytes);
                        let elapsed = prev_instant.elapsed().as_secs_f64();
                        let speed = calculate_mbps(delta, elapsed);

                        let start_elapsed = shared_start.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);

                        if speed < LOW_SPEED_THRESHOLD_MBPS && start_elapsed > warmup_secs {
                            if let Some(since) = low_speed_since {
                                if since.elapsed() > LOW_SPEED_TIMEOUT {
                                    return Err(anyhow::anyhow!(
                                        "Connection too slow (below {:.2} Mbps for {}s). \
                                         Aborting test.",
                                        LOW_SPEED_THRESHOLD_MBPS,
                                        LOW_SPEED_TIMEOUT.as_secs()
                                    ));
                                }
                            } else {
                                low_speed_since = Some(Instant::now());
                            }
                        } else {
                            low_speed_since = None;
                        }

                        let speed_str = if speed == 0.0 && now_bytes == 0 {
                            "↑  --.- Mbps".to_string()
                        } else {
                            format!("↑  {}", theme::color_speed(speed, &config))
                        };

                        pb.set_message(format!(
                            "{}    {} total",
                            speed_str,
                            HumanBytes(now_bytes)
                        ));

                        prev_bytes = now_bytes;
                        prev_instant = Instant::now();
                    }
                }
            }
        })
    };

    let mut display_handle = display_task;
    let measurement_result = tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(duration_secs)) => Ok(()),
        task_result = tasks.join_next() => match task_result {
            Some(Ok(Err(error))) => Err(error),
            Some(Ok(Ok(()))) => Err(anyhow::anyhow!("Upload worker ended before the test completed")),
            Some(Err(error)) => Err(error.into()),
            None => Err(anyhow::anyhow!("No upload workers were started")),
        },
        display_result = &mut display_handle => match display_result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(())) => Err(anyhow::anyhow!("Upload display ended before the test completed")),
            Err(error) => Err(error.into()),
        },
    };

    token.cancel();
    tasks.shutdown().await;

    if let Err(error) = measurement_result {
        display_handle.abort();
        pb.finish_and_clear();
        return Err(error);
    }

    display_handle.await??;

    pb.finish_and_clear();

    let start = shared_start.get().copied().unwrap_or_else(Instant::now);
    let effective_duration = (start.elapsed().as_secs_f64() - warmup_secs).max(0.0);
    Ok(calculate_mbps(
        total_uploaded.load(Ordering::Relaxed),
        effective_duration,
    ))
}

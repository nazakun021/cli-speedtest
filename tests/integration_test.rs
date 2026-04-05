// tests/integration_test.rs
//
// Integration tests use mockito to spin up a local HTTP server — no real network
// required. Each test constructs RunArgs pointing at the mock server URL, then
// calls cli_speedtest::run() or the individual client functions directly.
//
// Timing note: tests that exercise test_download / test_upload will take at least
// `duration_secs` (3s) because the CancellationToken sleep drives the test window.
// This is expected and correct behaviour for integration tests.

use mockito::Matcher;
use reqwest::Client;
use cli_speedtest::{
    client::{test_download, test_ping_stats, test_upload},
    models::{AppConfig, RunArgs},
    utils::calculate_mbps,
};
use std::sync::Arc;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn quiet_config() -> Arc<AppConfig> {
    Arc::new(AppConfig {
        quiet: true,
        color: false,
    })
}

fn test_client() -> Client {
    Client::builder()
        .user_agent("rust-speedtest-test/0.0.0")
        .build()
        .expect("Failed to build reqwest client")
}

// Minimum duration that passes the warm-up validation (WARMUP_SECS = 2.0).
const TEST_DURATION_SECS: u64 = 3;

// ── calculate_mbps (pure, no I/O) ────────────────────────────────────────────

#[test]
fn calculate_mbps_known_value() {
    // 12.5 MiB/s = 100 Mbps
    let speed = calculate_mbps(13_107_200, 1.0);
    assert!((speed - 100.0).abs() < 0.001);
}

#[test]
fn calculate_mbps_zero_duration_returns_zero() {
    assert_eq!(calculate_mbps(999_999, 0.0), 0.0);
}

// ── test_ping_stats ───────────────────────────────────────────────────────────

#[tokio::test]
async fn ping_stats_returns_valid_measurements() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(3)
        .create_async()
        .await;

    let stats = test_ping_stats(&test_client(), &server.url(), 3, quiet_config())
        .await
        .expect("ping_stats should succeed");

    assert!(stats.min_ms <= stats.max_ms, "min must be ≤ max");
    assert!(stats.avg_ms >= stats.min_ms as f64, "avg must be ≥ min");
    assert!(
        stats.packet_loss_pct >= 0.0 && stats.packet_loss_pct <= 100.0,
        "packet loss must be 0–100%"
    );
    assert!(stats.jitter_ms >= 0.0, "jitter must be non-negative");

    _mock.assert_async().await;
}

#[tokio::test]
async fn ping_stats_single_probe_has_zero_jitter() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .create_async()
        .await;

    let stats = test_ping_stats(&test_client(), &server.url(), 1, quiet_config())
        .await
        .expect("single-probe ping should succeed");

    assert_eq!(
        stats.jitter_ms, 0.0,
        "jitter is undefined with one sample — must be 0"
    );
    assert_eq!(stats.min_ms, stats.max_ms, "min == max with one sample");
}

// ── test_download ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn download_returns_non_negative_speed() {
    let mut server = mockito::Server::new_async().await;
    // Return a small body — the client loops until the token fires, not until
    // it has downloaded the full requested size.
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(200)
        .with_body(vec![0u8; 64 * 1024]) // 64 KB per response
        .expect_at_least(1)
        .create_async()
        .await;

    let speed = test_download(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1, // single connection keeps the test deterministic
        quiet_config(),
    )
    .await
    .expect("download should not error");

    assert!(speed >= 0.0, "speed must be non-negative, got {}", speed);
}

// ── test_upload ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn upload_returns_non_negative_speed() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/__up")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    let speed = test_upload(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1,
        quiet_config(),
    )
    .await
    .expect("upload should not error");

    assert!(speed >= 0.0, "speed must be non-negative, got {}", speed);
}

// ── --no-download / --no-upload ───────────────────────────────────────────────

#[tokio::test]
async fn no_download_flag_skips_download_and_returns_none() {
    let mut server = mockito::Server::new_async().await;

    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    // Explicitly expect zero download requests
    let download_mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(200)
        .expect(0)
        .create_async()
        .await;

    let _upload = server
        .mock("POST", "/__up")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    let result = cli_speedtest::run(
        RunArgs {
            server_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: true, // <-- flag under test
            no_upload: false,
        },
        quiet_config(),
        test_client(),
    )
    .await
    .expect("run() should succeed");

    assert!(
        result.download_mbps.is_none(),
        "download_mbps must be None when --no-download is set"
    );
    assert!(
        result.upload_mbps.is_some(),
        "upload_mbps must be Some when --no-upload is not set"
    );

    download_mock.assert_async().await; // verifies 0 calls to /__down
}

#[tokio::test]
async fn no_upload_flag_skips_upload_and_returns_none() {
    let mut server = mockito::Server::new_async().await;

    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    let _download = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(200)
        .with_body(vec![0u8; 64 * 1024])
        .expect_at_least(1)
        .create_async()
        .await;

    // Explicitly expect zero upload requests
    let upload_mock = server
        .mock("POST", "/__up")
        .with_status(200)
        .expect(0)
        .create_async()
        .await;

    let result = cli_speedtest::run(
        RunArgs {
            server_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: false,
            no_upload: true, // <-- flag under test
        },
        quiet_config(),
        test_client(),
    )
    .await
    .expect("run() should succeed");

    assert!(
        result.download_mbps.is_some(),
        "download_mbps must be Some when --no-download is not set"
    );
    assert!(
        result.upload_mbps.is_none(),
        "upload_mbps must be None when --no-upload is set"
    );

    upload_mock.assert_async().await; // verifies 0 calls to /__up
}

#[tokio::test]
async fn both_no_flags_skips_both_tests() {
    let mut server = mockito::Server::new_async().await;

    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    let result = cli_speedtest::run(
        RunArgs {
            server_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: true,
            no_upload: true,
        },
        quiet_config(),
        test_client(),
    )
    .await
    .expect("run() should succeed even with both tests skipped");

    assert!(result.download_mbps.is_none());
    assert!(result.upload_mbps.is_none());
    // Ping always runs — result must still be present
    assert!(result.ping.avg_ms >= 0.0);
}

// ── --server (custom URL) ─────────────────────────────────────────────────────

#[tokio::test]
async fn custom_server_url_is_used_and_reflected_in_result() {
    let mut server = mockito::Server::new_async().await;

    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;
    let _down = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(200)
        .with_body(vec![0u8; 64 * 1024])
        .create_async()
        .await;
    let _up = server
        .mock("POST", "/__up")
        .with_status(200)
        .create_async()
        .await;

    let custom_url = server.url(); // e.g. "http://127.0.0.1:PORT"

    let result = cli_speedtest::run(
        RunArgs {
            server_url: custom_url.clone(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: false,
            no_upload: false,
        },
        quiet_config(),
        test_client(),
    )
    .await
    .expect("run() should succeed with a custom server URL");

    // Non-default URL → server_name should be the URL itself, not "Cloudflare"
    assert_ne!(
        result.server_name, "Cloudflare",
        "Custom server should not be labelled as Cloudflare"
    );
    assert_eq!(
        result.server_name, custom_url,
        "server_name should equal the custom URL passed in"
    );
}

// ── Validation edge cases ─────────────────────────────────────────────────────

#[tokio::test]
async fn duration_equal_to_warmup_is_rejected() {
    // WARMUP_SECS = 2.0, so duration = 2 must fail validation
    let result = cli_speedtest::run(
        RunArgs {
            server_url: "https://speed.cloudflare.com".into(),
            duration_secs: 2,
            connections: None,
            ping_count: 1,
            no_download: false,
            no_upload: false,
        },
        quiet_config(),
        test_client(),
    )
    .await;

    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("warm-up"),
        "Error message should mention warm-up period"
    );
}

#[tokio::test]
async fn zero_ping_count_is_rejected() {
    let result = cli_speedtest::run(
        RunArgs {
            server_url: "https://speed.cloudflare.com".into(),
            duration_secs: 10,
            connections: None,
            ping_count: 0, // invalid
            no_download: false,
            no_upload: false,
        },
        quiet_config(),
        test_client(),
    )
    .await;

    assert!(result.is_err());
    assert!(
        result.unwrap_err().to_string().contains("ping-count"),
        "Error message should mention --ping-count"
    );
}

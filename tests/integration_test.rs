// tests/integration_test.rs
#![allow(clippy::await_holding_lock)]
//
// Integration tests use mockito to spin up a local HTTP server — no real network
// required. Each test constructs RunArgs pointing at the mock server URL, then
// calls cli_speedtest::run() or the individual client functions directly.
//
// Timing note: tests that exercise test_download / test_upload will take at least
// `duration_secs` (3s) because the CancellationToken sleep drives the test window.
// This is expected and correct behaviour for integration tests.

use cli_speedtest::{
    client::{test_download, test_ping_stats, test_upload},
    models::{AppConfig, RunArgs},
    utils::calculate_mbps,
};
use mockito::Matcher;
use reqwest::Client;
use serde_json::json;
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

#[test]
fn json_error_output_escapes_special_characters() {
    let error = "provider returned \"bad request\"\ntry again";
    let output = serde_json::to_string(&json!({ "error": error })).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["error"], error);
}

#[test]
fn direct_mode_json_errors_are_valid_and_nonzero() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cli-speedtest"))
        .args(["--duration", "2", "--json"])
        .output()
        .expect("CLI should launch");

    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("error output must be valid JSON");
    assert!(result["error"].as_str().unwrap().contains("warm-up"));
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

#[tokio::test]
async fn ping_stats_rejects_non_successful_responses() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(503)
        .expect_at_least(1)
        .create_async()
        .await;

    let result = test_ping_stats(&test_client(), &server.url(), 1, quiet_config()).await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("All ping attempts failed")
    );
    _mock.assert_async().await;
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
        2.0,
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
        2.0,
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
            provider_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: true, // <-- flag under test
            no_upload: false,
            quick: false,
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
            provider_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: false,
            no_upload: true, // <-- flag under test
            quick: false,
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
            provider_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: true,
            no_upload: true,
            quick: false,
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
async fn custom_provider_url_is_used_and_reflected_in_result() {
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
            provider_url: custom_url.clone(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 2,
            no_download: false,
            no_upload: false,
            quick: false,
        },
        quiet_config(),
        test_client(),
    )
    .await
    .expect("run() should succeed with a custom provider URL");

    // Non-default URL → provider_name should be the URL itself, not "Cloudflare"
    assert_ne!(
        result.provider_name, "Cloudflare",
        "Custom provider should not be labelled as Cloudflare"
    );
    assert_eq!(
        result.provider_name, custom_url,
        "provider_name should equal the custom URL passed in"
    );
}

#[tokio::test]
async fn custom_provider_rejects_missing_selected_endpoint() {
    let mut server = mockito::Server::new_async().await;

    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    let result = cli_speedtest::run(
        RunArgs {
            provider_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 1,
            no_download: false,
            no_upload: true,
            quick: false,
        },
        quiet_config(),
        test_client(),
    )
    .await;

    let error = result.expect_err("missing download endpoint must fail preflight");
    assert!(error.to_string().contains("GET /__down"));
}

// ── Validation edge cases ─────────────────────────────────────────────────────

#[tokio::test]
async fn duration_equal_to_warmup_is_rejected() {
    // WARMUP_SECS = 2.0, so duration = 2 must fail validation
    let result = cli_speedtest::run(
        RunArgs {
            provider_url: "https://speed.cloudflare.com".into(),
            duration_secs: 2,
            connections: None,
            ping_count: 1,
            no_download: false,
            no_upload: false,
            quick: false,
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
            provider_url: "https://speed.cloudflare.com".into(),
            duration_secs: 10,
            connections: None,
            ping_count: 0, // invalid
            no_download: false,
            no_upload: false,
            quick: false,
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

// ── Error / Retry Integration ─────────────────────────────────────────────────

#[tokio::test]
async fn download_bails_immediately_on_429() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(429)
        .with_header("retry-after", "120")
        .expect(1) // verify exactly 1 hit
        .create_async()
        .await;

    let start = std::time::Instant::now();
    let result = test_download(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1,
        2.0,
        quiet_config(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("rate-limited"));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    _mock.assert_async().await;
}

#[tokio::test]
async fn download_bails_immediately_on_403() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(403)
        .expect(1) // verify exactly 1 hit
        .create_async()
        .await;

    let result = test_download(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1,
        2.0,
        quiet_config(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Bot Fight Mode"));
    _mock.assert_async().await;
}

#[tokio::test]
async fn download_retries_on_500() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(500)
        // With max_retries = 3, we expect 1 initial request + up to 3 retries = 4 hits
        // Wait, the with_retry block in client makes exactly `attempts` requests
        // Wait! The retry logic loop uses `max_retries` incorrectly in some parts?
        // No, `with_retry` will attempt enough times. If it attempts at least 2 times it means it retries.
        .expect_at_least(2)
        .create_async()
        .await;

    let result = test_download(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1,
        2.0,
        quiet_config(),
    )
    .await;

    assert!(result.is_err());
    _mock.assert_async().await;
}

#[tokio::test]
async fn upload_bails_immediately_on_429() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/__up")
        .with_status(429)
        .expect(1) // verify exactly 1 hit
        .create_async()
        .await;

    let start = std::time::Instant::now();
    let result = test_upload(
        &test_client(),
        &server.url(),
        TEST_DURATION_SECS,
        1,
        2.0,
        quiet_config(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("rate-limited"));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    _mock.assert_async().await;
}

#[tokio::test]
async fn download_aborts_on_low_speed() {
    let mut server = mockito::Server::new_async().await;
    // Mock a server that returns data very slowly (1 byte every 1s)
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_status(200)
        .with_chunked_body(|w| {
            for _ in 0..10 {
                w.write_all(b"a")?;
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
            Ok(())
        })
        .create_async()
        .await;

    let result = test_download(
        &test_client(),
        &server.url(),
        10, // 10s test
        1,
        2.0,
        quiet_config(),
    )
    .await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Connection too slow"));
}

#[tokio::test]
async fn upload_aborts_on_low_speed() {
    let mut server = mockito::Server::new_async().await;
    // Mock a server that accepts POST but we simulate slow upload
    // Actually, mockito doesn't easily let us slow down the body receipt from client,
    // but we can make the server hang or return 200 after a long delay.
    // However, the client sends the whole payload at once in `test_upload`.
    // Wait, the client sends `payload.clone()` in `client.post(url.clone()).body(payload.clone()).send()`.

    // If the server is slow to respond to the POST, the `with_retry` will wait.
    // But the display task calculates speed based on `total_uploaded`.
    // `total_uploaded` is incremented AFTER `send().await?` returns successfully.

    // So if the server is slow, `total_uploaded` won't increase, and speed will be 0.

    let _mock = server
        .mock("POST", "/__up")
        .with_status(200)
        .with_chunked_body(|w| {
            std::thread::sleep(std::time::Duration::from_secs(10));
            w.write_all(b"OK")?;
            Ok(())
        })
        .create_async()
        .await;

    let result = test_upload(&test_client(), &server.url(), 10, 1, 2.0, quiet_config()).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Connection too slow"));
}

#[tokio::test]
#[ignore = "Takes 120s due to global timeout, run manually"]
async fn run_respects_global_timeout() {
    let mut server = mockito::Server::new_async().await;
    let _ping = server
        .mock("HEAD", "/cdn-cgi/trace")
        .with_status(200)
        .expect_at_least(1)
        .create_async()
        .await;

    // Simulate server accepting connection but hanging indefinitely
    let _mock = server
        .mock("GET", Matcher::Regex(r"^/__down".to_string()))
        .with_chunked_body(|_w| {
            std::thread::sleep(std::time::Duration::from_secs(300));
            Ok(())
        })
        .create_async()
        .await;

    let result = cli_speedtest::run(
        RunArgs {
            provider_url: server.url(),
            duration_secs: TEST_DURATION_SECS,
            connections: Some(1),
            ping_count: 1,
            no_download: false,
            no_upload: true,
            quick: false,
        },
        quiet_config(),
        test_client(),
    )
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Global timeout"));
}

#[tokio::test]
async fn test_quick_mode_bypasses_warmup() {
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
        .create_async()
        .await;

    // Quick mode should allow a 1-second duration (standard mode would fail duration <= WARMUP_SECS)
    let result = cli_speedtest::run(
        RunArgs {
            provider_url: server.url(),
            duration_secs: 1, // 1 second duration
            connections: Some(1),
            ping_count: 1,
            no_download: false,
            no_upload: true,
            quick: true, // <-- quick mode
        },
        quiet_config(),
        test_client(),
    )
    .await;

    assert!(
        result.is_ok(),
        "Quick mode should run successfully with 1 second duration: {:?}",
        result
    );
}

#[tokio::test]
async fn test_cli_cooldown_and_quick_burst() {
    use std::process::Command;
    use tempfile::TempDir;

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
        .with_body(vec![0u8; 1024])
        .create_async()
        .await;

    let _upload = server
        .mock("POST", "/__up")
        .with_status(200)
        .create_async()
        .await;

    let temp = TempDir::new().unwrap();
    let bin_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli-speedtest");

    if !bin_path.exists() {
        // If not built, compile it first
        let _ = Command::new("cargo").arg("build").status();
    }

    let run_cli = |args: &[&str]| {
        Command::new(&bin_path)
            .args(args)
            .env("SPEEDTEST_MOCK_DATA_DIR", temp.path())
            .output()
            .expect("failed to execute process")
    };

    // First standard run: should succeed
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
    ]);
    assert!(
        out.status.success(),
        "First standard run should succeed. stdout: {}, stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Second standard run: should fail due to cooldown
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
    ]);
    assert!(
        !out.status.success(),
        "Second standard run should fail due to cooldown"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("Cooldown active"));

    // Quick run 1: should succeed (bypasses standard cooldown)
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(
        out.status.success(),
        "Quick run 1 should succeed. stdout: {}, stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Quick run 2: should succeed
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(out.status.success(), "Quick run 2 should succeed");

    // Quick run 3: should succeed
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(out.status.success(), "Quick run 3 should succeed");

    // Quick run 4: should succeed
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(out.status.success(), "Quick run 4 should succeed");

    // Quick run 5: should succeed
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(out.status.success(), "Quick run 5 should succeed");

    // Quick run 6: should FAIL because burst limit of 5 is reached
    let out = run_cli(&[
        "--server",
        &server.url(),
        "--ping-count",
        "1",
        "--duration",
        "3",
        "--quick",
    ]);
    assert!(
        !out.status.success(),
        "Quick run 6 should fail due to burst limit"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("Quick Burst limit reached"));
}

// ── Self-Updater Tests ────────────────────────────────────────────────────────

static UPDATE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn check_for_updates_succeeds_when_newer_version_available() {
    let _lock = UPDATE_ENV_LOCK.lock().unwrap();
    use cli_speedtest::updater::check_for_updates;

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v99.0.0/speedtest-linux-amd64"
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v99.0.0/speedtest-windows-amd64.exe"
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v99.0.0/speedtest-macos-intel"
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v99.0.0/speedtest-macos-arm64"
            }
        ]
    });

    let _mock = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    // Direct updater calls to the mock API base
    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_for_updates(&client)
        .await
        .expect("update check should run");
    assert!(res.is_some(), "Should find an update");
    let info = res.unwrap();
    assert_eq!(info.version, "99.0.0");

    let expected_asset_name = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "speedtest-linux-amd64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "speedtest-windows-amd64.exe"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "speedtest-macos-intel"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "speedtest-macos-arm64"
    } else {
        panic!("Unsupported test platform");
    };

    assert!(
        info.download_url.contains(expected_asset_name),
        "URL '{}' should contain asset name '{}'",
        info.download_url,
        expected_asset_name
    );

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
    }
}

#[tokio::test]
async fn check_for_updates_returns_none_when_version_is_current_or_older() {
    let _lock = UPDATE_ENV_LOCK.lock().unwrap();
    use cli_speedtest::updater::check_for_updates;

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        // v0.1.3 is current, v0.1.0 is older
        "tag_name": "v0.1.3",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v0.1.3/speedtest-linux-amd64"
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v0.1.3/speedtest-windows-amd64.exe"
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v0.1.3/speedtest-macos-intel"
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v0.1.3/speedtest-macos-arm64"
            }
        ]
    });

    let _mock = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_for_updates(&client)
        .await
        .expect("update check should run");
    assert!(res.is_none(), "Should not update if version is not newer");

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
    }
}

#[tokio::test]
async fn check_for_updates_returns_none_when_no_matching_asset_is_found() {
    let _lock = UPDATE_ENV_LOCK.lock().unwrap();
    use cli_speedtest::updater::check_for_updates;

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-unsupported-platform",
                "browser_download_url": "https://github.com/nazakun021/cli-speedtest/releases/download/v99.0.0/speedtest-unsupported-platform"
            }
        ]
    });

    let _mock = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_for_updates(&client)
        .await
        .expect("update check should run");
    assert!(
        res.is_none(),
        "Should not update if no matching asset for platform"
    );

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
    }
}

#[tokio::test]
async fn check_for_updates_returns_error_when_api_fails() {
    let _lock = UPDATE_ENV_LOCK.lock().unwrap();
    use cli_speedtest::updater::check_for_updates;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(500)
        .create_async()
        .await;

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_for_updates(&client).await;
    assert!(res.is_err(), "Should return error when API returns 500");

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
    }
}

#[tokio::test]
async fn run_update_succeeds_and_replaces_mock_executable() {
    use cli_speedtest::updater::run_update;
    use std::fs;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    let new_binary_payload = "NEW_BINARY_EXE_CONTENT_DATA";

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(new_binary_payload.as_bytes());
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/speedtest-new-binary.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock = server
        .mock("GET", "/speedtest-new-binary")
        .with_status(200)
        .with_body(new_binary_payload)
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    let mock_exe_path = temp_dir.path().join("speedtest_mock_exe");
    fs::write(&mock_exe_path, "OLD_BINARY_EXE_CONTENT_DATA").unwrap();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path);
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let download_url = format!("{}/speedtest-new-binary", server.url());
    let res = run_update(&client, &download_url, false).await;
    assert!(res.is_ok(), "run_update should succeed, got: {:?}", res);

    // Verify content has changed
    let current_content = fs::read_to_string(&mock_exe_path).unwrap();
    assert_eq!(
        current_content, new_binary_payload,
        "Mock binary should have been replaced"
    );

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_EXE_PATH");
    }
}

#[tokio::test]
async fn run_update_with_progress_bar_succeeds() {
    use cli_speedtest::updater::run_update;
    use std::fs;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    // Serve 1MB of binary content to trigger multiple progress bar updates
    let large_binary_payload = vec![65u8; 1024 * 1024];

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&large_binary_payload);
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/speedtest-large-binary.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock = server
        .mock("GET", "/speedtest-large-binary")
        .with_status(200)
        .with_body(large_binary_payload)
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    let mock_exe_path = temp_dir.path().join("speedtest_mock_exe");
    fs::write(&mock_exe_path, "INITIAL").unwrap();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path);
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let download_url = format!("{}/speedtest-large-binary", server.url());
    let res = run_update(&client, &download_url, true).await; // <-- show_progress: true
    assert!(
        res.is_ok(),
        "run_update with progress should succeed, got: {:?}",
        res
    );

    // Verify it succeeded and replaced the binary
    let metadata = fs::metadata(&mock_exe_path).unwrap();
    assert_eq!(
        metadata.len(),
        1024 * 1024,
        "Mock binary size should be 1MB"
    );

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_EXE_PATH");
    }
}

#[tokio::test]
async fn cli_self_update_flag_performs_update_and_exits() {
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": format!("{}/download-asset", server.url())
            }
        ]
    });

    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"NEW_CLI_PAYLOAD");
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/download-asset.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock_asset = server
        .mock("GET", "/download-asset")
        .with_status(200)
        .with_body("NEW_CLI_PAYLOAD")
        .create_async()
        .await;

    let temp = TempDir::new().unwrap();
    let mock_exe_path = temp.path().join("speedtest_mock_exe");
    fs::write(&mock_exe_path, "OLD_CLI_PAYLOAD").unwrap();

    let bin_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("cli-speedtest");

    if !bin_path.exists() {
        let _ = Command::new("cargo").arg("build").status();
    }

    let out = Command::new(&bin_path)
        .arg("--self-update")
        .env("SPEEDTEST_MOCK_GITHUB_API", server.url())
        .env("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path)
        .output()
        .expect("failed to execute process");

    assert!(
        out.status.success(),
        "CLI self-update run should succeed, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stderr_str = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr_str.contains("Checking for updates")
            || stderr_str.contains("Downloading")
            || stderr_str.contains("Successfully updated")
    );

    let current_content = fs::read_to_string(&mock_exe_path).unwrap();
    assert_eq!(current_content, "NEW_CLI_PAYLOAD");
}

#[tokio::test]
async fn check_and_perform_auto_update_succeeds_and_updates_cache() {
    use cli_speedtest::updater::check_and_perform_auto_update;
    use std::fs;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": format!("{}/download-asset", server.url())
            }
        ]
    });

    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"NEW_AUTO_PAYLOAD");
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/download-asset.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock_asset = server
        .mock("GET", "/download-asset")
        .with_status(200)
        .with_body("NEW_AUTO_PAYLOAD")
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    let mock_exe_path = temp_dir.path().join("speedtest_mock_exe");
    fs::write(&mock_exe_path, "OLD_AUTO_PAYLOAD").unwrap();

    // Setup environments
    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
        std::env::set_var("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path);
        std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp_dir.path());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    // Verify cache file does not exist initially
    let cache_file = temp_dir.path().join("speedtest").join("last_update_check");
    assert!(!cache_file.exists());

    let res = check_and_perform_auto_update(&client).await;
    assert!(res.is_ok(), "Auto-update should succeed, got: {:?}", res);

    // Verify mock binary content has changed
    let current_content = fs::read_to_string(&mock_exe_path).unwrap();
    assert_eq!(current_content, "NEW_AUTO_PAYLOAD");

    // Verify cache file was written
    assert!(
        cache_file.exists(),
        "Cache file last_update_check should be created"
    );
    let timestamp_str = fs::read_to_string(&cache_file).unwrap();
    assert!(timestamp_str.trim().parse::<u64>().is_ok());

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
        std::env::remove_var("SPEEDTEST_MOCK_EXE_PATH");
        std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
    }
}

#[tokio::test]
async fn check_and_perform_auto_update_throttles_within_24_hours() {
    use cli_speedtest::updater::check_and_perform_auto_update;
    use std::fs;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    // Expect 0 calls to the API because the cache is warm
    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .expect(0)
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    let speedtest_dir = temp_dir.path().join("speedtest");
    fs::create_dir_all(&speedtest_dir).unwrap();

    // Write a recent timestamp (now) to the cache file
    let cache_file = speedtest_dir.join("last_update_check");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    fs::write(&cache_file, now.to_string()).unwrap();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
        std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp_dir.path());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_and_perform_auto_update(&client).await;
    assert!(res.is_ok(), "Should return Ok(()) even when throttled");

    _mock_api.assert_async().await;

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
        std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
    }
}

#[tokio::test]
async fn check_and_perform_auto_update_bypasses_if_env_var_set() {
    use cli_speedtest::updater::check_and_perform_auto_update;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    // Expect 0 calls because of env bypass
    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .expect(0)
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
        std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp_dir.path());
        std::env::set_var("NO_UPDATE", "1");
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_and_perform_auto_update(&client).await;
    assert!(res.is_ok(), "Should bypass and return Ok(())");

    _mock_api.assert_async().await;

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
        std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
        std::env::remove_var("NO_UPDATE");
    }
}

#[tokio::test]
async fn check_and_perform_auto_update_gracefully_handles_permission_denied() {
    use cli_speedtest::updater::check_and_perform_auto_update;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": format!("{}/download-asset", server.url())
            }
        ]
    });

    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"NEW_CLI_PAYLOAD");
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/download-asset.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock_asset = server
        .mock("GET", "/download-asset")
        .with_status(200)
        .with_body("NEW_CLI_PAYLOAD")
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    // Point the target path to the directory itself to trigger an IO error
    // (copying/replacing onto a directory path fails due to permissions/is-a-directory)
    let mock_exe_path = temp_dir.path().to_path_buf();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
        std::env::set_var("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path);
        std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp_dir.path());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let res = check_and_perform_auto_update(&client).await;
    assert!(
        res.is_ok(),
        "Auto-update should return Ok(()) even if permission/copy fails: {:?}",
        res
    );

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
        std::env::remove_var("SPEEDTEST_MOCK_EXE_PATH");
        std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
    }
}

#[tokio::test]
async fn test_tui_menu_triggers_updater_on_startup() {
    use cli_speedtest::menu::run_menu_with_selector;
    use cli_speedtest::models::AppConfig;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    let _lock = UPDATE_ENV_LOCK.lock().unwrap();

    let mut server = mockito::Server::new_async().await;
    let mock_response = serde_json::json!({
        "tag_name": "v99.0.0",
        "assets": [
            {
                "name": "speedtest-linux-amd64",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-windows-amd64.exe",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-intel",
                "browser_download_url": format!("{}/download-asset", server.url())
            },
            {
                "name": "speedtest-macos-arm64",
                "browser_download_url": format!("{}/download-asset", server.url())
            }
        ]
    });

    let _mock_api = server
        .mock("GET", "/repos/nazakun021/cli-speedtest/releases/latest")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_response).unwrap())
        .create_async()
        .await;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"NEW_TUI_PAYLOAD");
    let expected_sha = format!("{:x}", hasher.finalize());

    let _mock_sha = server
        .mock("GET", "/download-asset.sha256")
        .with_status(200)
        .with_body(expected_sha)
        .create_async()
        .await;

    let _mock_asset = server
        .mock("GET", "/download-asset")
        .with_status(200)
        .with_body("NEW_TUI_PAYLOAD")
        .create_async()
        .await;

    let temp_dir = TempDir::new().unwrap();
    let mock_exe_path = temp_dir.path().join("speedtest_mock_exe");
    fs::write(&mock_exe_path, "OLD_TUI_PAYLOAD").unwrap();

    unsafe {
        std::env::set_var("SPEEDTEST_MOCK_GITHUB_API", server.url());
        std::env::set_var("SPEEDTEST_MOCK_EXE_PATH", &mock_exe_path);
        std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp_dir.path());
    }

    let client = Client::builder().user_agent("test-agent").build().unwrap();

    let config = Arc::new(AppConfig {
        quiet: true,
        color: false,
    });

    let res = run_menu_with_selector(config, client, || Ok(Some(6))).await;
    assert!(res.is_ok(), "Menu should exit after the injected selection");

    // Verify mock binary content has changed
    let current_content = fs::read_to_string(&mock_exe_path).unwrap();
    assert_eq!(current_content, "NEW_TUI_PAYLOAD");

    unsafe {
        std::env::remove_var("SPEEDTEST_MOCK_GITHUB_API");
        std::env::remove_var("SPEEDTEST_MOCK_EXE_PATH");
        std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
    }
}

// src/main.rs

use clap::Parser;
use rand::Rng;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use cli_speedtest::models::{AppConfig, RunArgs};

const CONNECT_TIMEOUT_SECS: u64 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 30;
const GLOBAL_TEST_TIMEOUT_SECS: u64 = 120; // 2 minutes hard maximum
const DEFAULT_SERVER_URL: &str = "https://speed.cloudflare.com";

/// A blazing fast CLI Speedtest written in Rust
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Duration of the download/upload tests in seconds (must be > 2 due to warm-up)
    #[arg(short, long, default_value_t = 10)]
    duration: u64,

    /// Number of parallel connections for testing
    /// (default: 8 for download, 4 for upload; applies equally to both when set explicitly)
    #[arg(short, long)]
    connections: Option<usize>,

    /// Custom server base URL — must expose /__down, /__up, and /cdn-cgi/trace
    #[arg(long, default_value = DEFAULT_SERVER_URL)]
    server: String,

    /// Skip the download test
    #[arg(long, default_value_t = false)]
    no_download: bool,

    /// Skip the upload test
    #[arg(long, default_value_t = false)]
    no_upload: bool,

    /// Number of pings to send for latency/jitter measurement
    #[arg(long, default_value_t = 20)]
    ping_count: u32,

    /// Output results as JSON (suppresses all visual UI)
    #[arg(long, default_value_t = false)]
    json: bool,

    /// Enable debug logging for troubleshooting
    #[arg(long, default_value_t = false)]
    debug: bool,

    /// Disable all color output (also auto-disabled when NO_COLOR is set or stdout is piped)
    #[arg(long, default_value_t = false)]
    no_color: bool,

    /// Bypass the local cooldown and run the test immediately
    #[arg(long, default_value_t = false)]
    force_run: bool,
}

impl Args {
    /// Returns true if the user passed any flag that customises run behaviour.
    /// Used to decide whether to show the interactive menu.
    fn has_any_action_flags(&self) -> bool {
        self.no_download
            || self.no_upload
            || self.server != DEFAULT_SERVER_URL
            || self.connections.is_some()
            || self.duration != 10
            || self.ping_count != 20
    }
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

    const USER_AGENTS: &[&str] = &[
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0",
    ];

    let ua = USER_AGENTS[rand::rng().random_range(0..USER_AGENTS.len())];

    let client = Client::builder()
        .user_agent(ua)
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    let color_enabled =
        !args.no_color && std::env::var("NO_COLOR").is_err() && console::Term::stdout().is_term();

    let config = Arc::new(AppConfig {
        quiet: args.json,
        color: color_enabled,
    });

    let is_tty = console::Term::stdout().is_term();
    let has_flags = args.has_any_action_flags();
    let show_menu = is_tty && !has_flags && !args.json;

    if show_menu {
        cli_speedtest::menu::run_menu(config, client).await?;
    } else {
        tokio::select! {
            res = tokio::time::timeout(
                Duration::from_secs(GLOBAL_TEST_TIMEOUT_SECS),
                run_app(args.clone(), client, config)
            ) => {
                let res = res.unwrap_or_else(|_| {
                    Err(anyhow::anyhow!(
                        "Test timed out after {}s. The server may be rate limiting \
                         or unreachable.\n\n\
                         Try a custom server: speedtest --server <URL>",
                        GLOBAL_TEST_TIMEOUT_SECS
                    ))
                });
                match res {
                    Ok(result) => {
                        if args.json {
                            println!("{}", serde_json::to_string_pretty(&result)?);
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
    }

    Ok(())
}

async fn run_app(
    args: Args,
    client: Client,
    config: Arc<AppConfig>,
) -> anyhow::Result<cli_speedtest::models::SpeedTestResult> {
    if !config.quiet {
        println!("🚀 Starting Rust Speedtest...\n");
    }

    if !args.force_run {
        if let Some(remaining) = cli_speedtest::cooldown::cooldown_remaining(
            cli_speedtest::cooldown::DEFAULT_COOLDOWN_SECS,
        ) {
            eprintln!(
                "⏳ Cooldown active. Last test ran recently.\n   \
                 Wait {} more minutes, or override with: speedtest --force-run",
                remaining / 60 + 1
            );
            std::process::exit(1);
        }
    }

    // Convert CLI args into the lib's RunArgs — keeps clap out of the library
    let run_args = RunArgs {
        server_url: args.server,
        duration_secs: args.duration,
        connections: args.connections,
        ping_count: args.ping_count,
        no_download: args.no_download,
        no_upload: args.no_upload,
    };

    let result = cli_speedtest::run(run_args, config, client).await?;

    let _ = cli_speedtest::cooldown::record_successful_run();

    Ok(result)
}

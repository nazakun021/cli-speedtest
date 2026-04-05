// src/menu.rs

use crate::models::{AppConfig, MenuSettings, RunArgs};
use crate::theme::{pad_to, speed_rating};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use reqwest::Client;
use std::sync::Arc;

const DEFAULT_SERVER_URL: &str = "https://speed.cloudflare.com";

const ASCII_ART: &str = r#"
 ██████╗██╗     ██╗    ███████╗██████╗ ███████╗███████╗██████╗ ████████╗███████╗███████╗████████╗
██╔════╝██║     ██║    ██╔════╝██╔══██╗██╔════╝██╔════╝██╔══██╗╚══██╔══╝██╔════╝██╔════╝╚══██╔══╝
██║     ██║     ██║    ███████╗██████╔╝█████╗  █████╗  ██║  ██║   ██║   █████╗  ███████╗   ██║
██║     ██║     ██║    ╚════██║██╔═══╝ ██╔══╝  ██╔══╝  ██║  ██║   ██║   ██╔══╝  ╚════██║   ██║
╚██████╗███████╗██║    ███████║██║     ███████╗███████╗██████╔╝   ██║   ███████╗███████║   ██║
 ╚═════╝╚══════╝╚═╝    ╚══════╝╚═╝     ╚══════╝╚══════╝╚═════╝    ╚═╝   ╚══════╝╚══════╝   ╚═╝
"#;

const ASCII_ART_COMPACT: &str = "  CLI SPEEDTEST  •  v0.1.0";

pub async fn run_menu(config: Arc<AppConfig>, client: Client) -> anyhow::Result<()> {
    let mut settings = MenuSettings::default();

    loop {
        print_welcome(&config);

        let options = &[
            "🚀  Start Full Speed Test",
            "📡  Quick Ping Only",
            "⚙️   Settings",
            "📋  View Commands",
            "❓  Help",
            "🚪  Exit",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Main Menu")
            .items(options)
            .default(0)
            .interact_opt()?;

        match selection {
            Some(0) => run_full_test(&settings, &config, &client).await?,
            Some(1) => run_quick_ping(&settings, &config, &client).await?,
            Some(2) => show_settings(&mut settings, &config)?,
            Some(3) => show_commands(&config),
            Some(4) => show_help(&config),
            Some(5) | None => {
                clear_screen();
                break;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn print_welcome(_config: &AppConfig) {
    clear_screen();
    let term_width = console::Term::stdout().size().1 as usize;

    if term_width >= 95 {
        println!("{}", ASCII_ART);
    } else {
        println!("\n{}\n", ASCII_ART_COMPACT);
    }

    println!("  A blazing fast network speed tester — written in Rust");
    println!(
        "  v{}  •  Cloudflare backend  •  github.com/nazakun021/cli-speedtest\n",
        env!("CARGO_PKG_VERSION")
    );
}

async fn run_full_test(
    settings: &MenuSettings,
    config: &AppConfig,
    client: &Client,
) -> anyhow::Result<()> {
    clear_screen();
    let run_args = RunArgs::from(settings);
    let app_config = Arc::new(AppConfig {
        quiet: config.quiet,
        color: settings.color,
    });

    crate::run(run_args, app_config, client.clone()).await?;

    println!("\n  Press Enter to return to menu…");
    wait_for_enter();
    Ok(())
}

async fn run_quick_ping(
    settings: &MenuSettings,
    config: &AppConfig,
    client: &Client,
) -> anyhow::Result<()> {
    clear_screen();
    println!("📡 Running Quick Ping...\n");

    let app_config = Arc::new(AppConfig {
        quiet: config.quiet,
        color: settings.color,
    });

    crate::client::test_ping_stats(client, DEFAULT_SERVER_URL, settings.ping_count, app_config)
        .await?;

    println!("\n  Press Enter to return to menu…");
    wait_for_enter();
    Ok(())
}

fn show_settings(settings: &mut MenuSettings, _config: &AppConfig) -> anyhow::Result<()> {
    loop {
        clear_screen();
        println!("  ⚙️  Settings\n");
        println!("  ───────────────────────────────");

        let options = &[
            format!("Test Duration        : {}s", settings.duration_secs),
            format!("Parallel Connections : {}", settings.connections),
            format!("Ping Probe Count     : {}", settings.ping_count),
            format!(
                "Color Output         : {}",
                if settings.color { "On" } else { "Off" }
            ),
            "↩  Back to Main Menu".to_string(),
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(options)
            .default(4)
            .interact_opt()?;

        match selection {
            Some(0) => {
                let durations = &[5, 10, 15, 20, 30];
                let idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select Duration")
                    .items(durations)
                    .default(1)
                    .interact()?;
                settings.duration_secs = durations[idx] as u64;
            }
            Some(1) => {
                let connections = &[2, 4, 6, 8, 12, 16];
                let idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select Parallel Connections")
                    .items(connections)
                    .default(3)
                    .interact()?;
                settings.connections = connections[idx];
            }
            Some(2) => {
                let counts = &[5, 10, 20, 30, 50];
                let idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Select Ping Count")
                    .items(counts)
                    .default(2)
                    .interact()?;
                settings.ping_count = counts[idx];
            }
            Some(3) => {
                let idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Color Output")
                    .items(&["On", "Off"])
                    .default(if settings.color { 0 } else { 1 })
                    .interact()?;
                settings.color = idx == 0;
            }
            Some(4) | None => break,
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn show_commands(_config: &AppConfig) {
    clear_screen();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  📋  Available Commands                                  │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  -d, --duration <SECS>       Test duration (default: 10) │");
    println!("  │  -c, --connections <N>       Parallel connections         │");
    println!("  │      --server <URL>          Custom server base URL       │");
    println!("  │      --no-download           Skip download test           │");
    println!("  │      --no-upload             Skip upload test             │");
    println!("  │      --ping-count <N>        Ping probes (default: 20)    │");
    println!("  │      --json                  Output results as JSON       │");
    println!("  │      --no-color              Disable color output         │");
    println!("  │      --debug                 Enable debug logging         │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  Example:  cli-speedtest --duration 20 --connections 12  │");
    println!("  │  Example:  cli-speedtest --json | jq .download_mbps      │");
    println!("  └─────────────────────────────────────────────────────────┘");
    println!("\n  Press Enter to return…");
    wait_for_enter();
}

fn show_help(config: &AppConfig) {
    clear_screen();
    let mock_conf = AppConfig {
        quiet: false,
        color: config.color,
    };

    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  ❓  Interpreting Your Results                           │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  SPEED                                                   │");
    println!(
        "  │    ≥ 500 Mbps  {} Excellent — fiber / high-end cable",
        pad_to("", 0)
    );
    println!("  │    100–499     Great     — HD streaming, fast downloads  │");
    println!("  │     25–99      Good      — video calls, light streaming  │");
    println!("  │      5–24      Fair      — basic browsing, email         │");
    println!("  │       < 5      Poor      — may struggle with modern web  │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  PING                                                    │");
    println!("  │    ≤  20 ms   Excellent — real-time gaming, VoIP         │");
    println!("  │    21–80 ms   Good      — video calls, general use       │");
    println!("  │    > 80 ms    High      — noticeable in latency-sensitive │");
    println!("  │               applications                               │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  JITTER  (variation in ping)                             │");
    println!("  │    ≤  5 ms   Stable — voice/video calls unaffected       │");
    println!("  │    6–20 ms   Moderate — occasional stutter possible      │");
    println!("  │    > 20 ms   Unstable — real-time apps will be impacted  │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  PACKET LOSS                                             │");
    println!("  │    0.0%      Ideal — no retransmission overhead          │");
    println!("  │    > 0.0%    Lossy — investigate ISP or local network    │");
    println!("  └─────────────────────────────────────────────────────────┘");

    // Re-rendering with actual ratings to show colors if enabled
    clear_screen();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  ❓  Interpreting Your Results                           │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  SPEED                                                   │");
    println!(
        "  │    ≥ 500 Mbps  {} — fiber / high-end cable",
        pad_to(&speed_rating(500.0, &mock_conf), 10)
    );
    println!(
        "  │    100–499     {} — HD streaming, fast downloads  ",
        pad_to(&speed_rating(100.0, &mock_conf), 10)
    );
    println!(
        "  │     25–99      {} — video calls, light streaming  ",
        pad_to(&speed_rating(25.0, &mock_conf), 10)
    );
    println!(
        "  │      5–24      {} — basic browsing, email         ",
        pad_to(&speed_rating(5.0, &mock_conf), 10)
    );
    println!(
        "  │       < 5      {} — may struggle with modern web  ",
        pad_to(&speed_rating(0.0, &mock_conf), 10)
    );
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  PING                                                    │");
    println!("  │    ≤  20 ms   Excellent — real-time gaming, VoIP         │");
    println!("  │    21–80 ms   Good      — video calls, general use       │");
    println!("  │    > 80 ms    High      — noticeable in latency-sensitive │");
    println!("  │               applications                               │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  JITTER  (variation in ping)                             │");
    println!("  │    ≤  5 ms   Stable — voice/video calls unaffected       │");
    println!("  │    6–20 ms   Moderate — occasional stutter possible      │");
    println!("  │    > 20 ms   Unstable — real-time apps will be impacted  │");
    println!("  ├─────────────────────────────────────────────────────────┤");
    println!("  │  PACKET LOSS                                             │");
    println!("  │    0.0%      Ideal — no retransmission overhead          │");
    println!("  │    > 0.0%    Lossy — investigate ISP or local network    │");
    println!("  └─────────────────────────────────────────────────────────┘");

    println!("\n  Press Enter to return…");
    wait_for_enter();
}

fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

fn wait_for_enter() {
    use std::io::{self, BufRead};
    let mut _line = String::new();
    let _ = io::stdin().lock().read_line(&mut _line);
}

impl From<&MenuSettings> for RunArgs {
    fn from(s: &MenuSettings) -> Self {
        RunArgs {
            server_url: DEFAULT_SERVER_URL.to_string(),
            duration_secs: s.duration_secs,
            connections: Some(s.connections),
            ping_count: s.ping_count,
            no_download: false,
            no_upload: false,
        }
    }
}

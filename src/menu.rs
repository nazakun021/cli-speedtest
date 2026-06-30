// src/menu.rs

use crate::models::{AppConfig, MenuSettings, RunArgs};
use crate::theme::{pad_to, speed_rating};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use reqwest::Client;
use std::sync::Arc;

const DEFAULT_PROVIDER_URL: &str = "https://speed.cloudflare.com";

const ASCII_ART: &str = r#"
 ██████╗██╗     ██╗    ███████╗██████╗ ███████╗███████╗██████╗ ████████╗███████╗███████╗████████╗
██╔════╝██║     ██║    ██╔════╝██╔══██╗██╔════╝██╔════╝██╔══██╗╚══██╔══╝██╔════╝██╔════╝╚══██╔══╝
██║     ██║     ██║    ███████╗██████╔╝█████╗  █████╗  ██║  ██║   ██║   █████╗  ███████╗   ██║
██║     ██║     ██║    ╚════██║██╔═══╝ ██╔══╝  ██╔══╝  ██║  ██║   ██║   ██╔══╝  ╚════██║   ██║
╚██████╗███████╗██║    ███████║██║     ███████╗███████╗██████╔╝   ██║   ███████╗███████║   ██║
 ╚═════╝╚══════╝╚═╝    ╚══════╝╚═╝     ╚══════╝╚══════╝╚═════╝    ╚═╝   ╚══════╝╚══════╝   ╚═╝
"#;

const ASCII_ART_COMPACT: &str = "  CLI SPEEDTEST  -  v0.1.0";

pub async fn run_menu(config: Arc<AppConfig>, client: Client) -> anyhow::Result<()> {
    let mut settings = MenuSettings::default();

    // Check/prompt for updates on startup before entering TUI loop
    let _ = crate::updater::check_and_perform_auto_update(&client).await;

    loop {
        print_welcome(&config);

        let options = &[
            "Start Full Speed Test",
            "Start Quick Speed Test",
            "Quick Ping Only",
            "Settings",
            "View Commands",
            "Help",
            "Exit",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Main Menu")
            .items(options)
            .default(0)
            .interact_opt()?;

        match selection {
            Some(0) => run_full_test(&settings, &config, &client).await?,
            Some(1) => run_quick_test(&settings, &config, &client).await?,
            Some(2) => run_quick_ping(&settings, &config, &client).await?,
            Some(3) => show_settings(&mut settings, &config)?,
            Some(4) => show_commands(&config),
            Some(5) => show_help(&config),
            Some(6) | None => {
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

    println!("  A blazing fast network speed tester - written in Rust");
    println!(
        "  v{}  -  Cloudflare backend  -  github.com/nazakun021/cli-speedtest\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn check_cooldown_and_confirm(quick: bool) -> anyhow::Result<bool> {
    use crate::cooldown::{CooldownStatus, enforce_cooldown_policy};

    match enforce_cooldown_policy(quick, false) {
        CooldownStatus::Allowed => Ok(true),
        CooldownStatus::BlockedByCooldown { remaining_secs } => {
            let confirm = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Cooldown is active ({}s remaining). Force run?",
                    remaining_secs
                ))
                .default(false)
                .interact()?;
            if confirm {
                let _ = enforce_cooldown_policy(quick, true);
                Ok(true)
            } else {
                Ok(false)
            }
        }
        CooldownStatus::BlockedByBurstLimit { remaining_secs } => {
            let confirm = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Quick Burst limit reached ({}s remaining). Force run?",
                    remaining_secs
                ))
                .default(false)
                .interact()?;
            if confirm {
                let _ = enforce_cooldown_policy(quick, true);
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

async fn run_full_test(
    settings: &MenuSettings,
    config: &AppConfig,
    client: &Client,
) -> anyhow::Result<()> {
    clear_screen();
    if !check_cooldown_and_confirm(settings.quick)? {
        return Ok(());
    }

    clear_screen();
    let run_args = RunArgs::from(settings);
    let app_config = Arc::new(AppConfig {
        quiet: config.quiet,
        color: settings.color,
    });

    crate::run(run_args, app_config, client.clone()).await?;

    println!("\n  Press Enter to return to menu...");
    wait_for_enter();
    Ok(())
}

async fn run_quick_test(
    settings: &MenuSettings,
    config: &AppConfig,
    client: &Client,
) -> anyhow::Result<()> {
    clear_screen();
    if !check_cooldown_and_confirm(true)? {
        return Ok(());
    }

    clear_screen();
    println!("Running Quick Speed Test...\n");

    let mut run_args = RunArgs::from(settings);
    run_args.quick = true;

    let app_config = Arc::new(AppConfig {
        quiet: config.quiet,
        color: settings.color,
    });

    crate::run(run_args, app_config, client.clone()).await?;

    println!("\n  Press Enter to return to menu...");
    wait_for_enter();
    Ok(())
}

async fn run_quick_ping(
    settings: &MenuSettings,
    config: &AppConfig,
    client: &Client,
) -> anyhow::Result<()> {
    clear_screen();
    println!("Running Quick Ping...\n");

    let app_config = Arc::new(AppConfig {
        quiet: config.quiet,
        color: settings.color,
    });

    crate::client::test_ping_stats(
        client,
        DEFAULT_PROVIDER_URL,
        settings.ping_count,
        app_config,
    )
    .await?;

    println!("\n  Press Enter to return to menu...");
    wait_for_enter();
    Ok(())
}

fn show_settings(settings: &mut MenuSettings, _config: &AppConfig) -> anyhow::Result<()> {
    loop {
        clear_screen();
        println!("  Settings\n");
        println!("  ───────────────────────────────");

        let options = &[
            format!("Test Duration        : {}s", settings.duration_secs),
            format!("Parallel Connections : {}", settings.connections),
            format!("Ping Probe Count     : {}", settings.ping_count),
            format!(
                "Color Output         : {}",
                if settings.color { "On" } else { "Off" }
            ),
            format!(
                "Default Test Mode    : {}",
                if settings.quick { "Quick" } else { "Integrity" }
            ),
            "<- Back to Main Menu".to_string(),
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(options)
            .default(5)
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
            Some(4) => {
                let idx = Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Default Test Mode")
                    .items(&["Integrity (Standard)", "Quick (Warm-up bypassed)"])
                    .default(if settings.quick { 1 } else { 0 })
                    .interact()?;
                settings.quick = idx == 1;
            }
            Some(5) | None => break,
            _ => unreachable!(),
        }
    }
    Ok(())
}

fn show_commands(_config: &AppConfig) {
    clear_screen();
    let w = 58;
    let inner_w = w - 2;
    println!("  ┌{}┐", "─".repeat(w));
    println!("  │ {} │", pad_to("Available Commands", inner_w));
    println!("  ├{}┤", "─".repeat(w));
    println!(
        "  │ {} │",
        pad_to(
            "-d, --duration <SECS>       Test duration (default: 10)",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to("-c, --connections <N>       Parallel connections", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to(
            "    --server <URL>          Custom provider base URL",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to("    --no-download           Skip download test", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to("    --no-upload             Skip upload test", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to(
            "    --ping-count <N>        Ping probes (default: 20)",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            "    --json                  Output results as JSON",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to("    --no-color              Disable color output", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to("    --debug                 Enable debug logging", inner_w)
    );
    println!("  ├{}┤", "─".repeat(w));
    println!(
        "  │ {} │",
        pad_to(
            "Example:  cli-speedtest --duration 20 --connections 12",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            "Example:  cli-speedtest --json | jq .download_mbps",
            inner_w
        )
    );
    println!("  └{}┘", "─".repeat(w));
    println!("\n  Press Enter to return...");
    wait_for_enter();
}

fn show_help(config: &AppConfig) {
    clear_screen();
    let mock_conf = AppConfig {
        quiet: false,
        color: config.color,
    };

    let w = 58;
    let inner_w = w - 2;
    println!("  ┌{}┐", "─".repeat(w));
    println!("  │ {} │", pad_to("Interpreting Your Results", inner_w));
    println!("  ├{}┤", "─".repeat(w));
    println!("  │ {} │", pad_to("SPEED", inner_w));
    println!(
        "  │ {} │",
        pad_to(
            &format!(
                "  ≥ 500 Mbps  {} — fiber / high-end cable",
                speed_rating(500.0, &mock_conf)
            ),
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            &format!(
                "  100–499     {} — HD streaming, fast downloads",
                speed_rating(100.0, &mock_conf)
            ),
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            &format!(
                "   25–99      {} — video calls, light streaming",
                speed_rating(25.0, &mock_conf)
            ),
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            &format!(
                "    5–24      {} — basic browsing, email",
                speed_rating(5.0, &mock_conf)
            ),
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            &format!(
                "     < 5      {} — may struggle with modern web",
                speed_rating(0.0, &mock_conf)
            ),
            inner_w
        )
    );
    println!("  ├{}┤", "─".repeat(w));
    println!("  │ {} │", pad_to("PING", inner_w));
    println!(
        "  │ {} │",
        pad_to("  ≤  20 ms   Excellent — real-time gaming, VoIP", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to("  21–80 ms   Good      — video calls, general use", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to(
            "  > 80 ms    High      — noticeable in latency-sensitive",
            inner_w
        )
    );
    println!("  │ {} │", pad_to("             applications", inner_w));
    println!("  ├{}┤", "─".repeat(w));
    println!("  │ {} │", pad_to("JITTER  (variation in ping)", inner_w));
    println!(
        "  │ {} │",
        pad_to("  ≤  5 ms   Stable — voice/video calls unaffected", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to(
            "  6–20 ms   Moderate — occasional stutter possible",
            inner_w
        )
    );
    println!(
        "  │ {} │",
        pad_to(
            "  > 20 ms   Unstable — real-time apps will be impacted",
            inner_w
        )
    );
    println!("  ├{}┤", "─".repeat(w));
    println!("  │ {} │", pad_to("PACKET LOSS", inner_w));
    println!(
        "  │ {} │",
        pad_to("  0.0%      Ideal — no retransmission overhead", inner_w)
    );
    println!(
        "  │ {} │",
        pad_to(
            "  > 0.0%    Lossy — investigate ISP or local network",
            inner_w
        )
    );
    println!("  └{}┘", "─".repeat(w));

    println!("\n  Press Enter to return...");
    wait_for_enter();
}

fn clear_screen() {
    print!("\x1b[2J\x1b[H");
}

fn wait_for_enter() {
    if cfg!(test) {
        return;
    }
    use std::io::{self, BufRead};
    let mut _line = String::new();
    let _ = io::stdin().lock().read_line(&mut _line);
}

impl From<&MenuSettings> for RunArgs {
    fn from(s: &MenuSettings) -> Self {
        RunArgs {
            provider_url: DEFAULT_PROVIDER_URL.to_string(),
            duration_secs: s.duration_secs,
            connections: Some(s.connections),
            ping_count: s.ping_count,
            no_download: false,
            no_upload: false,
            quick: s.quick,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_settings_to_run_args_mapping() {
        let settings = MenuSettings {
            quick: true,
            ..Default::default()
        };

        let run_args = RunArgs::from(&settings);
        assert!(run_args.quick, "quick flag should be mapped from settings");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_run_full_test_prompts_on_active_cooldown() {
        let _guard = crate::cooldown::TEST_ENV_LOCK.lock().unwrap();

        let temp = tempfile::TempDir::new().unwrap();
        unsafe {
            std::env::set_var("SPEEDTEST_MOCK_DATA_DIR", temp.path());
        }

        // Record standard run completion to trigger cooldown block
        let _ = crate::cooldown::record_run_completion(false);

        let settings = MenuSettings::default();
        let config = AppConfig {
            quiet: true,
            color: false,
        };
        let client = reqwest::Client::new();

        // Attempting to run should prompt user via dialoguer, which fails/errors in non-TTY test
        let res = run_full_test(&settings, &config, &client).await;
        assert!(
            res.is_err(),
            "Should return error due to TTY prompt block, got: {:?}",
            res
        );

        unsafe {
            std::env::remove_var("SPEEDTEST_MOCK_DATA_DIR");
        }
    }
}

# Phase 2.5 Specification: Interactive Menu & Box Rendering Fix
**Project:** `cli-speedtest`
**Phase:** 2.5 (between Phase 2 polish and Phase 3 features)
**Status:** ✅ Completed
**Depends on:** Phase 2 (`theme.rs`, `AppConfig.color`, `owo-colors`)

---

## Overview

This phase adds two distinct improvements:

1. **Box rendering fix** — the summary box border collapses on the right side
   whenever colored values are present. This is a measurement bug, not a visual
   preference.
2. **Interactive main menu** — an ASCII art welcome screen with an arrow-key
   navigable menu that wraps the existing `run()` function. The menu is
   TTY-only; all existing CLI flag behaviour is completely unchanged.

---

## Root Cause: Why the Box Breaks

The screenshot shows `║` characters floating disconnected from the right border.
The cause is that Rust's `{:<N}` format padding measures **byte length**, not
**visible character width**. An ANSI-colored string like
`\x1b[32m401.74 Mbps\x1b[0m` is 22 bytes but only 11 visible characters.
When inserted into a `{:<23}` field, Rust pads it to 23 *bytes* — but the
terminal renders it 12 columns short, pushing the right border inward.

```
Expected:  ║  Download   :      401.74 Mbps  Great      ║
Actual:    ║  Download   :      401.74 Mbps  Great      ║  ← right border drifts
```

The fix is a `visible_len()` helper that strips ANSI escape sequences before
measuring, then manually constructs the padding string to the correct visible
width. `console::strip_ansi_codes()` already exists in a dependency you have.

---

## New Dependencies

Add to `[dependencies]` in `Cargo.toml`:

```toml
dialoguer = "0.11"   # arrow-key Select menu; internally uses `console` (already a dep)
```

`dialoguer` 0.11 requires no new transitive dependencies beyond `console`, which
`indicatif` already pulls in.

---

## File Changes Summary

```
src/
├── client.rs        (unchanged)
├── lib.rs           (unchanged — run() stays public and flag-driven)
├── main.rs          ← CHANGED: TTY check, route to menu or direct run
├── menu.rs          ← NEW: ASCII art, menu loop, Settings state
├── models.rs        ← CHANGED: MenuSettings struct added
├── theme.rs         ← CHANGED: visible_len() + pad_to() helpers; box renderer
└── utils.rs         (unchanged)

tests/
└── integration_test.rs   (unchanged — tests call run() directly, unaffected)
```

---

## Part 1 — Box Rendering Fix

### 1.1  New helpers in `src/theme.rs`

```rust
/// Returns the visible (printed) length of a string by stripping ANSI codes first.
/// Uses console::strip_ansi_codes which handles all standard SGR sequences.
pub fn visible_len(s: &str) -> usize {
    console::strip_ansi_codes(s).chars().count()
}

/// Right-pads `s` with spaces so its *visible* width equals `width`.
/// If the visible length already meets or exceeds `width`, returns `s` unchanged.
pub fn pad_to(s: &str, width: usize) -> String {
    let vlen = visible_len(s);
    if vlen >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - vlen))
    }
}
```

### 1.2  Updated box renderer in `src/lib.rs`

Replace every `format!("{:<N}", colored_value)` call inside the summary box with
`pad_to(colored_value, N)`. The box width remains dynamic per the P2-5 spec
(clamped between 44 and 60 visible columns).

**Before (broken):**
```rust
println!("║  Download   : {:<18} Mbps ║", format!("{:.2}", down_speed));
```

**After (correct):**
```rust
let speed_str = color_speed(down_speed, &config);  // may contain ANSI codes
let rated_str = format!("{} {}", speed_str, speed_rating(down_speed, &config));
println!("║  Download   : {} ║", pad_to(&rated_str, field_width));
```

Where `field_width` is derived from the dynamic box width so the right border
always lands in the correct column regardless of color state.

### 1.3  Acceptance test

```bash
# Both of these must produce a box with aligned right borders:
cargo run -- --ping-count 3 --duration 4          # color on
cargo run -- --ping-count 3 --duration 4 --no-color   # color off
NO_COLOR=1 cargo run -- --ping-count 3 --duration 4   # color off via env
```

---

## Part 2 — Interactive Main Menu

### 2.1  Entry-point routing in `src/main.rs`

At the start of `main()`, after building the `Client` and `AppConfig`, check
whether to show the menu or run directly:

```rust
let is_tty     = console::Term::stdout().is_term();
let has_flags  = args.has_any_action_flags();  // see §2.2
let show_menu  = is_tty && !has_flags && !args.json;

if show_menu {
    menu::run_menu(config, client).await?;
} else {
    // existing direct-run path — completely unchanged
    run_app(args, client).await?;
}
```

**Rule:** if *any* action flag is present (`--no-download`, `--no-upload`,
`--server`, `--connections`, `--duration`, `--ping-count`), the menu is skipped
and the tool behaves exactly as it does today. This preserves 100% backward
compatibility for scripting.

### 2.2  `has_any_action_flags()` on `Args`

```rust
impl Args {
    /// Returns true if the user passed any flag that customises run behaviour.
    /// Used to decide whether to show the interactive menu.
    fn has_any_action_flags(&self) -> bool {
        self.no_download
            || self.no_upload
            || self.server != DEFAULT_SERVER_URL
            || self.connections.is_some()
            || self.duration != 10          // 10 is the clap default_value_t
            || self.ping_count != 20        // 20 is the clap default_value_t
    }
}
```

### 2.3  ASCII art welcome screen

The art is a `const &str` baked directly into `menu.rs` — no font files, no
runtime rendering, no extra crates. It is printed once on menu entry and cleared
before each test run.

```
 ██████╗██╗     ██╗    ███████╗██████╗ ███████╗███████╗██████╗ ████████╗███████╗███████╗████████╗
██╔════╝██║     ██║    ██╔════╝██╔══██╗██╔════╝██╔════╝██╔══██╗╚══██╔══╝██╔════╝██╔════╝╚══██╔══╝
██║     ██║     ██║    ███████╗██████╔╝█████╗  █████╗  ██║  ██║   ██║   █████╗  ███████╗   ██║
██║     ██║     ██║    ╚════██║██╔═══╝ ██╔══╝  ██╔══╝  ██║  ██║   ██║   ██╔══╝  ╚════██║   ██║
╚██████╗███████╗██║    ███████║██║     ███████╗███████╗██████╔╝   ██║   ███████╗███████║   ██║
 ╚═════╝╚══════╝╚═╝    ╚══════╝╚═╝     ╚══════╝╚══════╝╚═════╝    ╚═╝   ╚══════╝╚══════╝   ╚═╝
```

Beneath the art, a subtitle line and version badge:

```
  A blazing fast network speed tester — written in Rust
  v0.1.0  •  Cloudflare backend  •  github.com/nazakun021/cli-speedtest
```

The full art + subtitle is stored in `menu.rs`:

```rust
const ASCII_ART: &str = r#"
 ██████╗██╗     ██╗    ███████╗██████╗ ███████╗███████╗██████╗ ████████╗███████╗███████╗████████╗
██╔════╝██║     ██║    ██╔════╝██╔══██╗██╔════╝██╔════╝██╔══██╗╚══██╔══╝██╔════╝██╔════╝╚══██╔══╝
██║     ██║     ██║    ███████╗██████╔╝█████╗  █████╗  ██║  ██║   ██║   █████╗  ███████╗   ██║
██║     ██║     ██║    ╚════██║██╔═══╝ ██╔══╝  ██╔══╝  ██║  ██║   ██║   ██╔══╝  ╚════██║   ██║
╚██████╗███████╗██║    ███████║██║     ███████╗███████╗██████╔╝   ██║   ███████╗███████║   ██║
 ╚═════╝╚══════╝╚═╝    ╚══════╝╚═╝     ╚══════╝╚══════╝╚═════╝    ╚═╝   ╚══════╝╚══════╝   ╚═╝
"#;
```

If the terminal is narrower than the art (< 95 columns), fall back to a compact
single-line title instead:

```
  CLI SPEEDTEST  •  v0.1.0
```

Width check uses `console::Term::stdout().size().1`.

### 2.4  Menu options

The `dialoguer::Select` widget renders a styled list. The user navigates with
`↑`/`↓` arrow keys (or `k`/`j`) and confirms with `Enter`. `Esc` or `q` exits.

**Menu items:**

```
  🚀  Start Full Speed Test
  📡  Quick Ping Only
  ⚙️   Settings
  📋  View Commands
  ❓  Help
  ──────────────────
  🚪  Exit
```

| Option | Behaviour |
|---|---|
| **Start Full Speed Test** | Clears screen, runs `run()` with current `MenuSettings`, shows results, then prompts "Press Enter to return to menu…" |
| **Quick Ping Only** | Runs `test_ping_stats()` directly (no download/upload), shows a compact ping result, returns to menu |
| **Settings** | Opens a settings submenu (§2.5); returns to main menu when done |
| **View Commands** | Prints a formatted reference of all CLI flags (§2.6), waits for Enter |
| **Help** | Prints an interpretation guide for results (§2.7), waits for Enter |
| **Exit** | Clears the welcome art and exits with code 0 |

The separator line between Help and Exit is a non-selectable cosmetic item
rendered via `dialoguer`'s item theming.

### 2.5  Settings submenu

Settings let the user configure the run parameters interactively without needing
to know the CLI flags. Changes persist for the duration of the session (stored in
`MenuSettings` in memory; never written to disk).

```
  ⚙️  Settings
  ───────────────────────────────
  Test Duration     : 10s    [← current value shown]
  Parallel Connections : 8 down / 4 up
  Ping Probe Count  : 20
  Color Output      : On
  ───────────────────────────────
  ↩  Back to Main Menu
```

Each setting uses `dialoguer::Select` to pick from a preset list:

| Setting | Options |
|---|---|
| Test Duration | 5s, 10s (default), 15s, 20s, 30s |
| Parallel Connections | 2, 4, 6, 8 (default down), 12, 16 |
| Ping Probe Count | 5, 10, 20 (default), 30, 50 |
| Color Output | On (default), Off |

`MenuSettings` is stored in `menu.rs` and converted to `RunArgs` before each
test run:

```rust
// src/models.rs — new struct
#[derive(Debug, Clone)]
pub struct MenuSettings {
    pub duration_secs: u64,     // default: 10
    pub connections: usize,     // default: 8 (down), 4 (up)
    pub ping_count: u32,        // default: 20
    pub color: bool,            // default: true
}

impl Default for MenuSettings {
    fn default() -> Self {
        Self {
            duration_secs: 10,
            connections: 8,
            ping_count: 20,
            color: true,
        }
    }
}
```

### 2.6  View Commands screen

A formatted, human-readable reference of every CLI flag. Shown in the terminal,
waits for Enter to return. Uses `pad_to()` for alignment so it looks correct
with or without color.

```
  ┌─────────────────────────────────────────────────────────┐
  │  📋  Available Commands                                  │
  ├─────────────────────────────────────────────────────────┤
  │  -d, --duration <SECS>       Test duration (default: 10) │
  │  -c, --connections <N>       Parallel connections         │
  │      --server <URL>          Custom server base URL       │
  │      --no-download           Skip download test           │
  │      --no-upload             Skip upload test             │
  │      --ping-count <N>        Ping probes (default: 20)    │
  │      --json                  Output results as JSON       │
  │      --no-color              Disable color output         │
  │      --debug                 Enable debug logging         │
  ├─────────────────────────────────────────────────────────┤
  │  Example:  cli-speedtest --duration 20 --connections 12  │
  │  Example:  cli-speedtest --json | jq .download_mbps      │
  └─────────────────────────────────────────────────────────┘

  Press Enter to return…
```

### 2.7  Help screen

Explains how to interpret results. No interactivity beyond "press Enter". Uses
the same `pad_to()` box renderer as the summary.

```
  ┌─────────────────────────────────────────────────────────┐
  │  ❓  Interpreting Your Results                           │
  ├─────────────────────────────────────────────────────────┤
  │  SPEED                                                   │
  │    ≥ 500 Mbps  Excellent — fiber / high-end cable        │
  │    100–499     Great     — HD streaming, fast downloads  │
  │     25–99      Good      — video calls, light streaming  │
  │      5–24      Fair      — basic browsing, email         │
  │       < 5      Poor      — may struggle with modern web  │
  ├─────────────────────────────────────────────────────────┤
  │  PING                                                    │
  │    ≤  20 ms   Excellent — real-time gaming, VoIP         │
  │    21–80 ms   Good      — video calls, general use       │
  │    > 80 ms    High      — noticeable in latency-sensitive │
  │               applications                               │
  ├─────────────────────────────────────────────────────────┤
  │  JITTER  (variation in ping)                             │
  │    ≤  5 ms   Stable — voice/video calls unaffected       │
  │    6–20 ms   Moderate — occasional stutter possible      │
  │    > 20 ms   Unstable — real-time apps will be impacted  │
  ├─────────────────────────────────────────────────────────┤
  │  PACKET LOSS                                             │
  │    0.0%      Ideal — no retransmission overhead          │
  │    > 0.0%    Lossy — investigate ISP or local network    │
  └─────────────────────────────────────────────────────────┘

  Press Enter to return…
```

---

## Part 3 — `src/menu.rs` Structure

```rust
// src/menu.rs

use crate::models::{AppConfig, MenuSettings, RunArgs};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use reqwest::Client;
use std::sync::Arc;

const DEFAULT_SERVER_URL: &str = "https://speed.cloudflare.com";
const ASCII_ART: &str = r#"..."#;          // full art from §2.3
const ASCII_ART_COMPACT: &str = "  CLI SPEEDTEST";

pub async fn run_menu(config: Arc<AppConfig>, client: Client) -> anyhow::Result<()> {
    let mut settings = MenuSettings::default();

    loop {
        // Clear screen, print art, print menu
        print_welcome(&config);

        let choice = show_main_menu(&config)?;

        match choice {
            0 => run_full_test(&settings, &config, &client).await?,
            1 => run_quick_ping(&settings, &config, &client).await?,
            2 => show_settings(&mut settings, &config)?,
            3 => show_commands(&config),
            4 => show_help(&config),
            5 => { clear_screen(); break; }
            _ => unreachable!(),
        }
    }

    Ok(())
}

fn print_welcome(config: &AppConfig) { /* clears screen, prints ASCII art + subtitle */ }
fn show_main_menu(config: &AppConfig) -> anyhow::Result<usize> { /* dialoguer::Select */ }
async fn run_full_test(settings: &MenuSettings, config: &AppConfig, client: &Client) -> anyhow::Result<()> { /* calls crate::run() */ }
async fn run_quick_ping(settings: &MenuSettings, config: &AppConfig, client: &Client) -> anyhow::Result<()> { /* calls client::test_ping_stats() */ }
fn show_settings(settings: &mut MenuSettings, config: &AppConfig) -> anyhow::Result<()> { /* settings submenu */ }
fn show_commands(config: &AppConfig) { /* prints commands box, waits for Enter */ }
fn show_help(config: &AppConfig) { /* prints help box, waits for Enter */ }
fn clear_screen() { print!("\x1b[2J\x1b[H"); }
fn wait_for_enter() { /* reads a single Enter keystroke */ }
```

### Menu settings → RunArgs conversion

```rust
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
```

---

## Routing Decision Tree

```
main() starts
    │
    ├─ stdout is NOT a TTY?          ──→  direct run (existing path, unchanged)
    ├─ --json flag set?              ──→  direct run
    ├─ any action flag set?          ──→  direct run
    │
    └─ interactive TTY, no flags    ──→  menu::run_menu()
                                              │
                                              ├─ Start Full Test ──→ crate::run() ──→ results ──→ "press Enter" ──→ menu
                                              ├─ Quick Ping      ──→ test_ping_stats() ──→ "press Enter" ──→ menu
                                              ├─ Settings        ──→ settings submenu ──→ menu
                                              ├─ View Commands   ──→ commands box ──→ "press Enter" ──→ menu
                                              ├─ Help            ──→ help box ──→ "press Enter" ──→ menu
                                              └─ Exit            ──→ clear screen ──→ exit(0)
```

---

## Implementation Order

```
1. Box fix (theme.rs: visible_len, pad_to, updated box renderer)
      — standalone, no new deps, verifiable immediately

2. models.rs: add MenuSettings struct + Default impl
      — needed by menu.rs

3. menu.rs: skeleton + ASCII art + print_welcome()
      — visible immediately, no behaviour yet

4. menu.rs: show_main_menu() with dialoguer::Select
      — requires dialoguer dep to be added

5. menu.rs: run_full_test() and run_quick_ping()
      — wires menu to existing run() and test_ping_stats()

6. menu.rs: show_settings() submenu
      — purely UI, no measurement logic

7. menu.rs: show_commands() and show_help() screens
      — purely informational, uses pad_to() box renderer

8. main.rs: TTY check + routing logic
      — last step; everything else must work before this is wired up
```

---

## Testing Requirements

### Unit tests (`src/theme.rs`)

| Test | Asserts |
|---|---|
| `visible_len_plain_string` | `visible_len("hello")` == 5 |
| `visible_len_colored_string` | `visible_len("\x1b[32m401.74\x1b[0m")` == 6 |
| `pad_to_short_string_pads_correctly` | `pad_to("hi", 5)` == `"hi   "` |
| `pad_to_colored_string_pads_to_visible_width` | visible_len of result == 10 |
| `pad_to_already_at_width_unchanged` | `pad_to("hello", 5)` == `"hello"` |
| `pad_to_over_width_unchanged` | `pad_to("toolong", 4)` == `"toolong"` |

### Unit tests (`src/menu.rs` or `tests/menu_test.rs`)

| Test | Asserts |
|---|---|
| `menu_settings_default_values` | All fields match documented defaults |
| `menu_settings_converts_to_run_args` | `RunArgs::from(&MenuSettings::default())` has correct fields |
| `has_any_action_flags_false_for_defaults` | Default `Args` returns false |
| `has_any_action_flags_true_when_no_download` | Returns true |
| `has_any_action_flags_true_when_custom_server` | Returns true |

### Manual acceptance tests

```bash
# 1. Menu appears in normal interactive terminal
cargo run

# 2. Menu does NOT appear with any flag
cargo run -- --duration 5
cargo run -- --no-download
cargo run -- --json
cargo run -- --server https://speed.cloudflare.com

# 3. Box alignment — right border must be flush with color on and off
cargo run   # select "Start Full Speed Test", inspect summary box
NO_COLOR=1 cargo run   # same, verify no drift

# 4. Narrow terminal — resize to ~80 cols, run full test
#    Box must not overflow or wrap

# 5. Settings persist within session
#    Change duration to 5s in Settings, run full test — test must complete in ~5s

# 6. Quick Ping Only — must not trigger any download/upload requests
#    Verify by running with --debug and checking stderr

# 7. Exit option — must clear welcome art and return cleanly to shell prompt

# 8. ASCII art fallback — resize terminal to < 95 columns
#    Compact title must appear instead of full block art
```

---

## Definition of Done

- [x] `visible_len()` and `pad_to()` exist in `theme.rs`
- [x] Summary box right border is flush with color on, off, and piped
- [x] `MenuSettings` struct exists in `models.rs` with `Default` impl
- [x] `From<&MenuSettings> for RunArgs` is implemented
- [x] `menu.rs` exists with all 7 functions implemented
- [x] ASCII art displays at ≥ 95 columns; compact title at < 95 columns
- [x] All 6 menu options are navigable with arrow keys and Enter
- [x] Settings submenu persists changes for the session
- [x] Menu is completely bypassed when any action flag is set or stdout is non-TTY
- [x] All unit tests in the testing requirements table pass
- [x] All 8 manual acceptance tests pass
- [x] `cargo clippy -- -D warnings` passes
- [x] `cargo fmt --check` passes
- [x] Phase 2.5 items added to `TODO.md` and marked ✅ on completion

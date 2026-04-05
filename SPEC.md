# Phase 2 Specification: Visual Polish & UX
**Project:** `cli-speedtest`
**Phase:** 2 of 3
**Status:** ✅ Completed
**Depends on:** Phase 1 (all items ✅ complete)

---

## Overview

Phase 2 transforms the tool from a correct, well-architected speedtest into one
that *feels* professional. The five items in this phase are not cosmetic luxuries
— for a CLI speedtest specifically, live speed feedback and clear color-coded
results are part of the core user contract. Users who run `fast.com`'s CLI or
`speedtest-cli` expect to watch a number climb in real time and get an instant
color verdict. This phase delivers that.

**Goals:**
- Make every number on screen instantly interpretable without reading labels
- Show live rolling speed during transfers, not just a byte counter
- Degrade gracefully to plain text when piped, redirected, or `NO_COLOR` is set
- Handle terminals of any width without wrapping or truncation

**Non-goals for Phase 2:**
- No new measurement features (those are Phase 3)
- No persistent history or export (Phase 3)
- No changes to the JSON output schema (already stable)

---

## New Dependencies

Add to `[dependencies]` in `Cargo.toml`:

```toml
owo-colors = "3"
console    = "0.15"
```

| Crate | Why this one |
|---|---|
| `owo-colors` | Zero-cost color abstraction with built-in `if_supports_color()` — handles `NO_COLOR` env var and non-TTY stdout automatically without any manual checks |
| `console` | Terminal size query (`Term::stdout().size()`), TTY detection, and ANSI stripping; already a transitive dep of `indicatif` so compile cost is near zero |

No other new dependencies are required. All five items in this phase are
implemented using these two crates plus the existing `indicatif` and `tokio`.

---

## Architecture Changes

### New file: `src/theme.rs`

A single module owns all presentation logic: color rules, rating labels, and
box-drawing helpers. Nothing outside `theme.rs` should import `owo-colors`
directly — all color decisions go through this module so thresholds and palette
changes require edits in exactly one place.

```
src/
├── client.rs       (unchanged interface, updated spinner format — P2-2)
├── lib.rs          (passes Arc<AppConfig> with new `color` field)
├── main.rs         (reads --no-color flag, populates AppConfig)
├── models.rs       (AppConfig gains `color: bool`)
├── theme.rs        ← NEW (all color/rating/box logic lives here)
└── utils.rs        (WARMUP_SECS, calculate_mbps, with_retry — unchanged)
```

### `AppConfig` gains one field

```rust
// src/models.rs
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub quiet: bool,
    pub color: bool,   // NEW — false when --no-color, NO_COLOR set, or non-TTY
}
```

`color` is determined once at startup in `main.rs` and never re-evaluated. All
rendering paths already receive `Arc<AppConfig>`, so the field is available
everywhere with zero additional propagation.

---

## Item Specifications

---

### P2-1 — Semantic Color System

**Priority:** Medium
**Touches:** `src/theme.rs` (new), `src/lib.rs` (summary box), `src/client.rs` (ping println)
**Blocked by:** Nothing

#### Behavior

Every numeric result displayed to the user is colored according to a fixed
threshold table. The color is chosen by value, not by position or context — the
same Mbps number gets the same color whether it appears in the spinner, the
progress update, or the final summary.

#### Threshold Tables

**Speed (Mbps) — applies to download and upload:**

| Range | Color | Rationale |
|---|---|---|
| ≥ 100 Mbps | Green | Sufficient for 4K streaming, large file transfers |
| 25–99 Mbps | Yellow | Adequate for most tasks; room for improvement |
| < 25 Mbps | Red | Likely to cause friction for modern workloads |

**Ping (ms) — applies to `avg_ms`:**

| Range | Color |
|---|---|
| ≤ 20 ms | Green |
| 21–80 ms | Yellow |
| > 80 ms | Red |

**Jitter (ms):**

| Range | Color |
|---|---|
| ≤ 5 ms | Green |
| 6–20 ms | Yellow |
| > 20 ms | Red |

**Packet loss (%):**

| Value | Color |
|---|---|
| 0.0% | Green |
| > 0.0% | Red |

#### `theme.rs` Public API

```rust
/// Returns the value formatted and ANSI-colored for speed (Mbps).
/// Returns plain string if config.color is false.
pub fn color_speed(mbps: f64, config: &AppConfig) -> String

/// Returns the value formatted and ANSI-colored for ping (ms).
pub fn color_ping(ms: f64, config: &AppConfig) -> String

/// Returns the value formatted and ANSI-colored for jitter (ms).
pub fn color_jitter(ms: f64, config: &AppConfig) -> String

/// Returns the value formatted and ANSI-colored for packet loss (%).
pub fn color_loss(pct: f64, config: &AppConfig) -> String
```

Each function checks `config.color` before applying any ANSI codes. When
`config.color` is false, the return value is the bare formatted number with no
escape sequences — safe to pipe or redirect.

#### Where Applied

| Location | Before | After |
|---|---|---|
| Summary box — Download | `{:.2} Mbps` plain | `color_speed(down, config)` |
| Summary box — Upload | `{:.2} Mbps` plain | `color_speed(up, config)` |
| Summary box — Ping avg | plain | `color_ping(avg_ms, config)` |
| Summary box — Jitter | plain | `color_jitter(jitter_ms, config)` |
| Summary box — Packet loss | plain | `color_loss(pct, config)` |
| Ping println in `test_ping_stats` | plain | `color_ping` + `color_jitter` + `color_loss` |

---

### P2-2 — Live Rolling-Speed Display

**Priority:** High (most visible UX improvement)
**Touches:** `src/client.rs` (`test_download`, `test_upload`), `src/theme.rs`
**Blocked by:** P2-1 (needs `color_speed` for the live display string)

#### Behavior

During download and upload, the spinner message updates every 250 ms with the
*current instantaneous speed* computed from the bytes transferred in the last
interval, not the cumulative average. This matches the behavior users expect from
professional speedtest tools.

#### Display Format

```
⠸ [00:00:05] ↓  412.7 Mbps   237 MB total
⠸ [00:00:05] ↑   98.3 Mbps    61 MB total
```

The Mbps value is color-coded via `color_speed` (green/yellow/red per P2-1
thresholds). The `↓` / `↑` arrow is fixed per function — it does not change
color.

#### Implementation

A dedicated display task is spawned alongside the worker tasks inside
`test_download` and `test_upload`. It holds a reference to the same
`Arc<AtomicU64>` byte counter the workers write to.

```
┌─ spawner (test_download / test_upload)
│
├── worker task ×N     — write bytes to Arc<AtomicU64>
│
└── display task ×1    — reads AtomicU64 every 250ms
                         diffs against previous reading
                         calls pb.set_message(format!(...))
                         exits when CancellationToken fires
```

The display task shares the same `CancellationToken` as the worker tasks, so
it stops automatically at the end of the test window without any additional
shutdown logic.

**Sampling logic (pseudocode):**

```
prev_bytes  = 0
prev_instant = Instant::now()

loop {
    select! {
        _ = token.cancelled() => break,
        _ = sleep(250ms) => {
            now_bytes = total_bytes.load(Relaxed)
            delta     = now_bytes - prev_bytes
            elapsed   = prev_instant.elapsed().as_secs_f64()
            speed     = calculate_mbps(delta, elapsed)

            pb.set_message(format!(
                "{arrow} {speed}   {total} total",
                arrow = direction_arrow,
                speed = color_speed(speed, config),
                total = HumanBytes(now_bytes),
            ))

            prev_bytes   = now_bytes
            prev_instant = Instant::now()
        }
    }
}
```

#### `indicatif` Spinner Template

```rust
"{spinner:.green} [{elapsed_precise}] {msg}"
```

The `{msg}` field carries the entire `↓ 412.7 Mbps   237 MB total` string,
updated by the display task. Remove `{bytes}` and `{bytes_per_sec}` from the
template — they show cumulative stats and will conflict visually with the
rolling display.

#### Edge Cases

| Scenario | Handling |
|---|---|
| First 250ms interval returns 0 bytes | Show `↓ --.- Mbps` placeholder — no division by zero |
| Warm-up window (first 2s) | Display task still runs and shows speed; the counter exclusion is only for the final calculation, not the live display |
| `config.quiet = true` | `create_spinner` returns `ProgressBar::hidden()`; display task's `pb.set_message()` calls are no-ops — no behavior change needed |

---

### P2-3 — Speed Rating Label in Summary

**Priority:** Low
**Touches:** `src/theme.rs`, `src/lib.rs` (summary box rendering)
**Blocked by:** P2-1 (needs color infrastructure)

#### Behavior

The summary box appends a short human-readable verdict next to each speed value.
The label uses the same color as the value itself.

```
╠══════════════════════════════════════╣
║  Download   :      412.70 Mbps  Excellent ║
║  Upload     :       98.30 Mbps      Great ║
```

#### Rating Scale

| Speed (Mbps) | Label |
|---|---|
| ≥ 500 | `Excellent` |
| 100–499 | `Great` |
| 25–99 | `Good` |
| 5–24 | `Fair` |
| < 5 | `Poor` |

#### `theme.rs` Addition

```rust
/// Returns a short rating label for a given Mbps value.
/// The label is colored to match color_speed() for the same value.
pub fn speed_rating(mbps: f64, config: &AppConfig) -> &'static str

/// Convenience: returns "{color_speed}  {color_rating}" as a single string.
pub fn speed_with_rating(mbps: f64, config: &AppConfig) -> String
```

#### Skipped Tests

When `download_mbps` or `upload_mbps` is `None` (i.e. `--no-download` or
`--no-upload` was passed), the row shows `skipped` in grey — no rating label.

---

### P2-4 — `--no-color` Flag + `NO_COLOR` / Non-TTY Compliance

**Priority:** Medium
**Touches:** `src/main.rs`, `src/models.rs`, `src/theme.rs`
**Blocked by:** P2-1 (theme functions must already accept `config.color`)

#### Behavior

Color is disabled automatically under three conditions, checked in order at
startup:

1. `--no-color` CLI flag is present
2. The `NO_COLOR` environment variable is set (any value, per [no-color.org](https://no-color.org))
3. Stdout is not a TTY (i.e. output is being piped or redirected)

All three checks are evaluated once in `main()` before `AppConfig` is
constructed. No runtime re-evaluation occurs.

#### `main.rs` Logic

```rust
let color_enabled = !args.no_color
    && std::env::var("NO_COLOR").is_err()
    && console::Term::stdout().is_term();

let config = Arc::new(AppConfig {
    quiet: args.json,
    color: color_enabled,
});
```

#### New CLI Flag

```rust
/// Disable all color output (also auto-disabled when NO_COLOR is set or stdout is piped)
#[arg(long, default_value_t = false)]
no_color: bool,
```

#### `theme.rs` Contract

All color helper functions already accept `&AppConfig` and check `config.color`
before applying ANSI codes (established in P2-1). P2-4 requires no changes to
`theme.rs` — it only requires that `AppConfig.color` is set correctly upstream.
This separation is intentional: the theme module has no knowledge of *why* color
is disabled, only *whether* it is.

#### Verification Checklist

These should all produce clean, ANSI-free output:

```bash
speedtest --no-color
speedtest | cat
NO_COLOR=1 speedtest
speedtest > results.txt && cat results.txt
```

---

### P2-5 — Terminal-Width-Aware Summary Box

**Priority:** Low
**Touches:** `src/lib.rs` (summary box rendering), `src/theme.rs`
**Blocked by:** P2-3 (rating labels change the minimum required box width)

#### Behavior

The summary box scales its width to the terminal at runtime. The hardcoded 38-
character width is replaced with a dynamic value derived from the current
terminal column count.

#### Width Formula

```rust
let term_cols = console::Term::stdout().size().1 as usize; // (rows, cols)
let box_width = (term_cols.saturating_sub(4)).min(60).max(44);
// min(44) ensures the widest row (speed + rating label) always fits
// max(60) caps growth so the box doesn't look absurd on 220-col terminals
```

#### Field Truncation

The server name field must handle long custom `--server` URLs gracefully. If the
server name exceeds the available field width, truncate with an ellipsis:

```rust
fn truncate_to(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
```

Apply `truncate_to` to the server name before it is inserted into the box string.

#### Fallback

`console::Term::stdout().size()` returns `(0, 0)` when stdout is not a TTY (e.g.
when piped). In that case the formula gives `box_width = 44`, which is the safe
minimum. This means `--json` mode and piped output are unaffected.

---

## Implementation Order

The items have a dependency chain that dictates the order of implementation:

```
P2-1 (color system / theme.rs)
  └── P2-2 (live speed — needs color_speed for spinner message)
  └── P2-3 (rating labels — needs color infrastructure)
        └── P2-5 (dynamic box width — needs to know max row width after ratings)
P2-4 (--no-color / NO_COLOR) — can be done any time after P2-1
```

Recommended order: **P2-1 → P2-2 → P2-4 → P2-3 → P2-5**

P2-4 is slotted before P2-3 so that color compliance is verified on the spinner
output (P2-2) before more colored surfaces are added.

---

## Testing Requirements

### Unit tests (in `src/theme.rs` or `tests/`)

| Test | What it verifies |
|---|---|
| `color_speed_green` | ≥ 100 Mbps returns string containing ANSI green code when `color: true` |
| `color_speed_plain_when_no_color` | Same input returns no ANSI codes when `color: false` |
| `speed_rating_boundaries` | Values at 5, 25, 100, 500 Mbps return correct label |
| `speed_rating_below_5` | < 5 Mbps returns `"Poor"` |
| `truncate_to_short_string` | String ≤ max returns unchanged |
| `truncate_to_long_string` | String > max returns truncated with `…` |
| `box_width_non_tty_fallback` | `term_cols = 0` yields `box_width = 44` |

### Manual acceptance tests

Run these before marking Phase 2 complete:

```bash
# 1. Full color output in a real terminal
cargo run

# 2. No ANSI codes when piped
cargo run | cat | grep -P '\x1b' && echo "FAIL: ANSI found" || echo "PASS"

# 3. No ANSI codes with NO_COLOR
NO_COLOR=1 cargo run | grep -P '\x1b' && echo "FAIL" || echo "PASS"

# 4. No ANSI codes with --no-color
cargo run -- --no-color | grep -P '\x1b' && echo "FAIL" || echo "PASS"

# 5. Narrow terminal — resize to 60 columns before running
cargo run

# 6. Long custom server name truncation
cargo run -- --server https://this-is-a-very-long-custom-server-hostname.example.com \
             --no-download --no-upload --ping-count 3
```

---

## Definition of Done

Phase 2 is complete when:

- [ ] `src/theme.rs` exists with all four color helpers and `speed_rating`
- [ ] `AppConfig` has a `color: bool` field populated correctly in `main.rs`
- [ ] All three no-color conditions (flag / `NO_COLOR` / non-TTY) strip ANSI codes
- [ ] Spinners during download and upload show rolling Mbps updated every 250 ms
- [ ] Summary box values are color-coded per threshold tables
- [ ] Summary box speed rows include a rating label
- [ ] Summary box width is dynamic and clamps between 44 and 60 columns
- [ ] All unit tests in the testing requirements table pass
- [ ] All six manual acceptance tests pass
- [ ] `cargo clippy -- -D warnings` passes with zero warnings
- [ ] `cargo fmt --check` passes
- [ ] `TODO.md` Phase 2 items are marked ✅
# CLI Speedtest TODO

The core measurement engine and CLI interface are now stable and production-ready.
All previously identified bugs and architectural issues from Phase 1 have been
resolved and verified via integration tests.

---

## ✅ Phase 1: Core Stability & Correctness (Completed)

| Category | Issue | Severity | Status |
|---|---|---|---|
| Bug | Upload errors silently dropped | 🔴 High | ✅ Fixed |
| Bug | `GET` ping inflates latency | 🟡 Medium | ✅ Fixed |
| Bug | Duplicate `WARMUP_SECS` constants | 🟡 Medium | ✅ Resolved (Moved to `utils.rs`) |
| Bug | Timer starts before task spawn | 🟡 Medium | ✅ Resolved (Synchronized via `Barrier`) |
| Bug | Deprecated `thread_rng()` usage | 🟡 Medium | ✅ Resolved (Updated to `rand 0.9` API) |
| Measurement | Single-shot ping, no jitter | 🔴 High | ✅ Fixed (Added jitter & packet loss) |
| Measurement | No TCP slow-start warm-up | 🔴 High | ✅ Fixed (Added 2s warm-up window) |
| Measurement | Hardcoded connection count | 🟡 Medium | ✅ Fixed (Added `--connections` flag) |
| Code quality | `#[allow(unreachable_code)]` usage | 🟡 Medium | ✅ Resolved (Explicit async control flow) |
| Code quality | `quiet` flag prop-drilling | 🟢 Low | ✅ Resolved (Consolidated in `AppConfig`) |
| Features | Missing `--server` flag | 🟡 Medium | ✅ Added |
| Features | Missing `--no-download/upload` flags | 🟢 Low | ✅ Added |
| Features | JSON missing timestamp/version | 🟡 Medium | ✅ Added |
| Release | `authors` email syntax malformed | 🟢 Low | ✅ Fixed |
| Release | No MSRV declared in `Cargo.toml` | 🟢 Low | ✅ Fixed (`rust-version = "1.85"`) |
| Release | Outdated `reqwest`/`rand` crates | 🟡 Medium | ✅ Fixed (Updated to latest stable) |
| Testing | No integration tests | 🟡 Medium | ✅ Added (`tests/integration_test.rs`) |

---

## 🎨 Phase 2: Visual Polish & UX (Next)

A speedtest lives or dies on the quality of its visual output. This phase focuses
on making the tool feel as polished as `fast.com` or `speedtest.net`'s CLI — while
remaining a proper Unix citizen that degrades gracefully when piped.

### P2-1 — Semantic color system  `🟡 Medium`
Add `owo-colors` and `console` to `Cargo.toml`. Define a single `theme.rs` module
with helper functions that apply consistent color rules everywhere:

| Value | Threshold | Color |
|---|---|---|
| Download / Upload speed | ≥ 100 Mbps | 🟢 Green |
| Download / Upload speed | 25–99 Mbps | 🟡 Yellow |
| Download / Upload speed | < 25 Mbps | 🔴 Red |
| Ping | ≤ 20 ms | 🟢 Green |
| Ping | 21–80 ms | 🟡 Yellow |
| Ping | > 80 ms | 🔴 Red |
| Jitter | ≤ 5 ms | 🟢 Green |
| Jitter | 6–20 ms | 🟡 Yellow |
| Jitter | > 20 ms | 🔴 Red |
| Packet loss | 0% | 🟢 Green |
| Packet loss | > 0% | 🔴 Red |

Apply these colors in the summary box and in the `--json` mode's plain-text
companion output. Colors must be absent when stdout is not a TTY or when the
`NO_COLOR` environment variable is set — `owo-colors`'s `if_supports_color()`
handles this automatically.

**New crates:** `owo-colors = "3"`, `console = "0.15"`

---

### P2-2 — Live rolling-speed display in progress spinners  `🔴 High`
The current spinner shows total bytes transferred (`{bytes}`). Users want to see
the *current speed right now*, not a cumulative total. This is the single biggest
perceived quality improvement.

**Implementation sketch:**
- Spawn a lightweight display task alongside the worker tasks in `test_download`
  and `test_upload`.
- The display task wakes every 250 ms, reads the `AtomicU64` byte counter, diffs
  it against the previous reading, and calls `pb.set_message(format!(...))` with
  the computed Mbps.
- Modify `AppConfig` to carry the `Arc<AtomicU64>` ref if needed, or pass it as
  a separate argument to the display task.

**Target spinner format:**
```
⠸ [00:00:05] ↓ 412.7 Mbps   (total: 257 MB)
⠸ [00:00:05] ↑  98.3 Mbps   (total:  61 MB)
```

---

### P2-3 — Speed rating label in summary  `🟢 Low`
Add a human-readable verdict to the summary box so users know at a glance whether
their connection is good for their use case.

```
║  Download   :           412.70 Mbps ║  ← Excellent
║  Upload     :            98.30 Mbps ║  ← Good
```

Define ratings in `theme.rs` alongside the color thresholds:

| Speed | Label |
|---|---|
| ≥ 500 Mbps | `Excellent` |
| 100–499 Mbps | `Great` |
| 25–99 Mbps | `Good` |
| 5–24 Mbps | `Fair` |
| < 5 Mbps | `Poor` |

---

### P2-4 — `--no-color` flag + `NO_COLOR` / non-TTY compliance  `🟡 Medium`
Professional CLIs follow the [NO_COLOR](https://no-color.org) convention. Add:

1. A `--no-color` CLI flag that sets `AppConfig::color: bool`.
2. Automatic detection: disable color when `NO_COLOR` is set in the environment
   or when stdout is not a TTY (detected via `console::Term::stdout().is_term()`).
3. `AppConfig` already flows through all render paths — adding a `color` field
   next to `quiet` is a one-line change; all theme helpers check it before
   applying ANSI codes.

This ensures correct output when piped: `speedtest --json | jq .` and
`speedtest | tee results.txt` should never contain ANSI escape sequences.

---

### P2-5 — Terminal-width-aware summary box  `🟢 Low`
The current box has a hardcoded width of 38 characters. On an 80-column terminal
it looks fine; on a narrow 60-column terminal it wraps; on a wide terminal it
looks small. Use `console::Term::stdout().size()` to get `(rows, cols)` at
runtime and scale the box width to `min(cols - 4, 60)`. This also makes the
server name field handle long custom URLs without truncating or overflowing.

---

## 🚀 Phase 3: Advanced Features

- [ ] **Multi-Server Selection**: Automatically find the closest server or allow
  a list of servers to be tested sequentially, then report per-server results.
- [ ] **CSV / NDJSON Record Export**: Append results to a local file (`~/.speedtest-history.ndjson`)
  for history tracking and graphing.
- [ ] **Adaptive Connection Scaling**: Automatically increase parallel connections
  if link saturation is not reached within the first few seconds.
- [ ] **Latency Histogram**: Display a mini ASCII histogram of ping distribution
  alongside min/avg/max (e.g. using `textplots`).
- [ ] **ISP / Location Metadata**: Query an IP-API to display local ISP and city
  in the summary header.
- [ ] **Better Error Reporting for Custom Servers**: Detect when a `--server` URL
  doesn't expose `/__down`, `/__up`, or `/cdn-cgi/trace` and surface a clear,
  actionable error message instead of a generic HTTP failure.

---

## 🛠️ Internal Maintenance

- [ ] **CI Pipeline**: Add a GitHub Action to run `cargo test`, `cargo fmt --check`,
  and `cargo clippy -- -D warnings` on every push and PR.
- [ ] **README Update**: Document all CLI flags, show example output (with the
  colored summary box as a screenshot), and fix the License placeholder.
- [ ] **Rustdoc**: Add doc-comments to all public functions in `lib.rs`, `client.rs`,
  and `utils.rs`. Enable `#![warn(missing_docs)]` in `lib.rs`.
- [ ] **Custom Error Types**: Replace `anyhow` bail strings with typed errors via
  `thiserror` so library consumers can match on error variants programmatically.
- [ ] **WASM Exploration**: Investigate whether the core measurement logic can
  target `wasm32-unknown-unknown` for a browser-based version.
- [ ] **Crate Split**: If the library grows, extract `speedtest-core` as a
  standalone crate with `cli-speedtest` as a thin binary wrapper.
```

---

The ordering matters: **P2-2 (live speed)** is marked high because it's the most noticeable missing feature for a speedtest specifically — users expect to watch the number climb in real-time. **P2-1 (color system)** comes first in the list because `theme.rs` is a shared dependency that P2-2 and P2-3 both build on.
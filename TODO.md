# CLI Speedtest TODO

The core measurement engine and CLI interface are now stable and production-ready.
All previously identified bugs and architectural issues from Phase 1 have been
resolved and verified via integration tests.

---

## ✅ Phase 1: Core Stability & Correctness (Completed)

| Category     | Issue                                | Severity  | Status                                    |
| ------------ | ------------------------------------ | --------- | ----------------------------------------- |
| Bug          | Upload errors silently dropped       | 🔴 High   | ✅ Fixed                                  |
| Bug          | `GET` ping inflates latency          | 🟡 Medium | ✅ Fixed                                  |
| Bug          | Duplicate `WARMUP_SECS` constants    | 🟡 Medium | ✅ Resolved (Moved to `utils.rs`)         |
| Bug          | Timer starts before task spawn       | 🟡 Medium | ✅ Resolved (Synchronized via `Barrier`)  |
| Bug          | Deprecated `thread_rng()` usage      | 🟡 Medium | ✅ Resolved (Updated to `rand 0.9` API)   |
| Measurement  | Single-shot ping, no jitter          | 🔴 High   | ✅ Fixed (Added jitter & packet loss)     |
| Measurement  | No TCP slow-start warm-up            | 🔴 High   | ✅ Fixed (Added 2s warm-up window)        |
| Measurement  | Hardcoded connection count           | 🟡 Medium | ✅ Fixed (Added `--connections` flag)     |
| Code quality | `#[allow(unreachable_code)]` usage   | 🟡 Medium | ✅ Resolved (Explicit async control flow) |
| Code quality | `quiet` flag prop-drilling           | 🟢 Low    | ✅ Resolved (Consolidated in `AppConfig`) |
| Features     | Missing `--server` flag              | 🟡 Medium | ✅ Added                                  |
| Features     | Missing `--no-download/upload` flags | 🟢 Low    | ✅ Added                                  |
| Features     | JSON missing timestamp/version       | 🟡 Medium | ✅ Added                                  |
| Release      | `authors` email syntax malformed     | 🟢 Low    | ✅ Fixed                                  |
| Release      | No MSRV declared in `Cargo.toml`     | 🟢 Low    | ✅ Fixed (`rust-version = "1.85"`)        |
| Release      | Outdated `reqwest`/`rand` crates     | 🟡 Medium | ✅ Fixed (Updated to latest stable)       |
| Testing      | No integration tests                 | 🟡 Medium | ✅ Added (`tests/integration_test.rs`)    |

---

## ✅ Phase 2: Visual Polish & UX (Completed)

| Item | Description                                          | Status   |
| ---- | ---------------------------------------------------- | -------- |
| P2-1 | Semantic color system (thresholds for Mbps/Ping)     | ✅ Fixed |
| P2-2 | Live rolling-speed display (250ms interval)          | ✅ Fixed |
| P2-3 | Speed rating labels in summary (Excellent/Great/etc) | ✅ Fixed |
| P2-4 | `--no-color` / `NO_COLOR` / non-TTY compliance       | ✅ Fixed |
| P2-5 | Terminal-width-aware dynamic summary box             | ✅ Fixed |

---

## ✅ Phase 2.5: Interactive Experience & Rendering Fix (Completed)

| Item   | Description                                       | Status   |
| ------ | ------------------------------------------------- | -------- |
| P2.5-1 | Summary box rendering fix (ANSI length awareness) | ✅ Fixed |
| P2.5-2 | Interactive main menu (TTY-only, flag-compatible) | ✅ Added |
| P2.5-3 | Interactive settings & interpretation guide       | ✅ Added |

---

## 🔴 Phase 2.6: Rate Limit & Anti-Ban Hardening (Critical — Fix Before Next Release)

These issues were discovered in real-world multi-user testing. When multiple users
run the tool in a short window, Cloudflare's endpoint returns 429/403 errors. The
current retry logic then makes the hang *worse* by retrying a rate-limited server
three times with backoff, causing tests to stall for over 60 seconds before failing.

---

### P2.6-1 — Treat 429 and 403 as fatal, not retryable  `🔴 High`

The root cause of the 60-second hang. `with_retry` currently retries all errors
including HTTP 429/403. Retrying a rate-limited server doesn't help — it just
consumes the full backoff budget before failing.

Intercept these codes immediately and surface a friendly, specific message:

```rust
reqwest::StatusCode::TOO_MANY_REQUESTS => {
    let wait = r.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(900); // default 15 minutes if header is absent
    anyhow::bail!(
        "You've been rate-limited by Cloudflare. \
         Please wait {} minutes before running the test again.\n\n\
         Alternatives:\n\
         • Use a custom server:  speedtest --server https://your-server.example.com\n\
         • Run ping only:        speedtest --no-download --no-upload",
        wait / 60
    )
}
reqwest::StatusCode::FORBIDDEN => {
    anyhow::bail!(
        "Cloudflare returned 403 Forbidden. Your IP may have triggered \
         Bot Fight Mode. Wait 15 minutes or use a custom server with \
         --server <URL>."
    )
}
```

This turns a 60-second hang into an immediate, actionable error message.

---

### P2.6-2 — Respect `Retry-After` header  `🟡 Medium`

When Cloudflare rate-limits a request it often includes a `Retry-After` header
specifying how many seconds to wait. Currently this header is ignored entirely.
The fix is already shown in P2.6-1 — read the header value on 429, default to
900s (15 minutes) if absent, and include the wait time in the error message.

---

### P2.6-3 — Reduce default connection counts  `🔴 High`

The current defaults (8 download / 4 upload) open up to 12 simultaneous
connections per user. Across multiple concurrent users this rapidly triggers
Cloudflare's rate limiter.

| Direction | Current default | New default | Rationale |
|---|---|---|---|
| Download | 8 | 4 | Still saturates most home connections up to ~1 Gbps |
| Upload | 4 | 2 | 2 connections is sufficient for accurate upload measurement |

Update both `run()` in `lib.rs` and `MenuSettings::default()` in `models.rs`.
The `--connections` flag still lets power users override upward.

---

### P2.6-4 — Auto-reduce connections on first 429  `🟡 Medium`

Instead of immediately bailing on the first 429, attempt one graceful recovery:
cancel all current workers, drop to a single connection, and retry once. This
handles transient throttling (where Cloudflare only objects to the concurrency
level, not the user's IP) without hanging indefinitely.

```
First 429 received
  → cancel all workers
  → warn: "Server throttled at N connections — retrying with 1 connection…"
  → retry the test phase with num_connections = 1
  → if still 429 → bail with friendly message from P2.6-1
```

---

### P2.6-5 — Request pacing with random jitter  `🟡 Medium`

Perfectly uniform, machine-generated traffic is a primary signal Cloudflare's
bot detection uses to identify automated scrapers. Real browser traffic is never
perfectly uniform. Adding a small random delay (50–150ms) between chunk requests
makes the traffic pattern look significantly more organic without meaningfully
affecting measurement accuracy.

```rust
// At the end of each chunk loop iteration in test_download / test_upload:
let jitter_ms = rand::rng().random_range(50u64..=150);
tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
```

No new dependency required — `random_range` is available via the existing
`rand = "0.9"` dependency.

---

### P2.6-6 — User-Agent rotation  `🟡 Medium`

The tool currently identifies itself as `rust-speedtest/0.1.0` on every request.
This is a trivially detectable bot signal. Rotating through a pool of realistic
browser User-Agent strings at startup significantly reduces the fingerprint.

```rust
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) \
     Gecko/20100101 Firefox/125.0",
];

// In main.rs when building the Client:
let ua = USER_AGENTS[rand::rng().random_range(0..USER_AGENTS.len())];
let client = Client::builder().user_agent(ua).build()?;
```

> **Note:** Using browser User-Agent strings makes the tool's traffic appear to
> originate from a real browser rather than a CLI. This is the most effective
> mitigation against Cloudflare Bot Fight Mode but is the most aggressive
> approach. Teams with concerns about impersonation can skip this item and rely
> on P2.6-3 and P2.6-5 instead, which achieve meaningful risk reduction without
> spoofing a browser identity.

---

### P2.6-7 — Local cooldown enforcement (disk-persisted)  `🟡 Medium`

Users running the tool on automated cronjobs (e.g. every minute) place an
outsized burden on Cloudflare's endpoint and will inevitably trigger rate
limiting for themselves and other users. A local cooldown prevents accidental
abuse.

**Critical requirement: the cooldown must be stored on disk**, not in memory.
An in-memory cooldown resets on every process start and does nothing to stop
a cronjob that launches a fresh process every minute.

**Implementation:**
- Store the last successful run timestamp in a platform-appropriate config
  directory via the `dirs` crate:
  - Linux/macOS: `~/.local/share/speedtest/last_run`
  - Windows: `%APPDATA%\speedtest\last_run`
- On startup, read this file. If elapsed time since the last run is less than
  `COOLDOWN_SECS` (default: 300s / 5 minutes), refuse to run and tell the
  user exactly how long to wait.
- Write the current timestamp only on *successful* test completion — a failed
  or rate-limited run should not reset the cooldown clock.
- A `--force-run` flag bypasses the cooldown for users who knowingly want to
  override it (e.g. running back-to-back tests during debugging).

```
$ speedtest
⏳ Cooldown active. Last test ran 2 minutes ago.
   Wait 3 more minutes, or override with: speedtest --force-run
```

**New dependency required:** `dirs = "5"` for cross-platform config path
resolution.

---

### P2.6-8 — Global test timeout as a safety net  `🟡 Medium`

Even after P2.6-1 is fixed, a test should never hang indefinitely. Add a
hard outer timeout around the entire `run()` call as a last-resort guard:

```rust
const GLOBAL_TEST_TIMEOUT_SECS: u64 = 120; // 2 minutes absolute maximum

tokio::time::timeout(
    Duration::from_secs(GLOBAL_TEST_TIMEOUT_SECS),
    cli_speedtest::run(run_args, config, client)
)
.await
.unwrap_or_else(|_| Err(anyhow::anyhow!(
    "Test timed out after {}s. The server may be rate limiting \
     or unreachable.\nTry: speedtest --server <custom-url>",
    GLOBAL_TEST_TIMEOUT_SECS
)))?;
```

---

### Implementation Order for Phase 2.6

```
v0.1.1 (this week — fixes the reported hang):
  P2.6-1  fatal 429/403 handling — eliminates the 60s hang
  P2.6-3  lower default connections — reduces rate limit frequency
  P2.6-8  global timeout — last-resort safety net, same PR as P2.6-1

v0.1.2 (next week — improves resilience):
  P2.6-2  Retry-After header — already partially covered in P2.6-1
  P2.6-4  auto-reduce connections on 429 — graceful single-connection retry
  P2.6-9  better error message copy with --server suggestion

v0.1.3 (anti-ban hardening):
  P2.6-5  request pacing / jitter — organic traffic pattern
  P2.6-6  User-Agent rotation — reduces bot fingerprint
  P2.6-7  local cooldown (disk-persisted) — prevents cronjob abuse
```

---

## 🚀 Phase 3: Advanced Features

- [ ] **Multi-Server Selection**: Automatically find the closest server or allow
      a list of servers to be tested sequentially, with automatic fallback if
      the primary server is rate-limited or unreachable.
- [ ] **CSV / NDJSON Record Export**: Append results to a local file
      (`~/.speedtest-history.ndjson`) for history tracking and graphing.
- [ ] **Adaptive Connection Scaling**: Automatically increase connections if
      saturation isn't reached, but back off immediately on any 429 response.
- [ ] **Latency Histogram**: Display a mini ASCII histogram of ping distribution
      alongside min/avg/max (e.g. using `textplots`).
- [ ] **ISP / Location Metadata**: Query an IP-API to display local ISP and city
      in the summary header.
- [ ] **Better Error Reporting for Custom Servers**: Detect when a `--server` URL
      doesn't expose `/__down`, `/__up`, or `/cdn-cgi/trace` and surface a clear,
      actionable error message instead of a generic HTTP failure.

---

## 🛠️ Internal Maintenance & Polish

- [x] **CI Pipeline**: GitHub Action running `cargo test`, `cargo fmt --check`,
      and `cargo clippy -- -D warnings` on every push/PR.
- [x] **Release Pipeline**: GitHub Action building binaries for Linux (amd64),
      Windows (amd64), macOS (Intel + ARM64) on version tags.
- [ ] **README Update**: Document all CLI flags, installation methods
      (`cargo install` + direct binary download), and show example output.
- [ ] **Rustdoc**: Add doc-comments to all public functions. Enable
      `#![warn(missing_docs)]` in `lib.rs`.
- [ ] **Unit Test Expansion**: Add tests covering the rate-limit short-circuit
      behaviour from P2.6-1 and the cooldown logic from P2.6-7.
- [ ] **Refine Error Handling**: Implement custom error types using `thiserror`
      for better programmatic error handling downstream.
- [ ] **WASM Support**: Explore compiling the core library to WASM for a
      browser-based version.
- [ ] **Crate Modularization**: Extract `speedtest-core` as a standalone crate
      if the library grows significantly.
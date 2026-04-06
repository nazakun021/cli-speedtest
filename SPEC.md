# Phase 2.6 Specification: Rate Limit & Anti-Ban Hardening

**Project:** `cli-speedtest`
**Phase:** 2.6
**Status:** Planning
**Target releases:** v0.1.1, v0.1.2, v0.1.3
**Depends on:** Phase 2.5 (all items ✅ complete)

---

## Background & Problem Statement

During the first real-world multi-user test of v0.1.0, two distinct failure
modes were observed:

1. **60-second hang followed by a cryptic error** — A user's test stalled for
   over a minute before failing. Root cause: `with_retry` retried a
   rate-limited endpoint three times with exponential backoff (100ms + 200ms +
   400ms), multiplied across 8 parallel download connections, before finally
   surfacing a generic HTTP error with no guidance on what to do.

2. **HTTP 400 "Too Many Requests"** — After multiple test runs in quick
   succession, Cloudflare's endpoint began rejecting requests. The tool had no
   mechanism to detect this condition, distinguish it from a network error, or
   tell the user how long to wait.

Both issues share a single root cause: **the tool is too aggressive toward a
shared public endpoint it does not control.** Phase 2.6 addresses this across
three layers — fail fast when rate-limited, reduce the likelihood of being
rate-limited, and make the tool's traffic pattern less detectable as automated.

---

## New Dependencies

```toml
# Cargo.toml — add to [dependencies]
dirs = "5"    # cross-platform config/data directory resolution (P2.6-7 only)
```

`dirs` is the only new dependency this phase introduces. All other items use
existing crates (`rand`, `reqwest`, `tokio`, `anyhow`).

---

## Files Changed

```
src/
├── client.rs     ← P2.6-1, P2.6-4, P2.6-5  (status handling, jitter, retry logic)
├── lib.rs        ← P2.6-3, P2.6-4, P2.6-8  (connection defaults, timeout wrapper)
├── main.rs       ← P2.6-6, P2.6-8           (User-Agent rotation, global timeout)
├── models.rs     ← P2.6-3, P2.6-7           (MenuSettings defaults, CooldownState)
├── utils.rs      ← P2.6-1                   (with_retry: non-retryable status list)
└── cooldown.rs   ← P2.6-7                   (NEW: disk-persisted cooldown logic)

Cargo.toml        ← P2.6-7                   (add dirs = "5")
```

---

## Release Targets

| Release    | Items                  | Goal                                  |
| ---------- | ---------------------- | ------------------------------------- |
| **v0.1.1** | P2.6-1, P2.6-3, P2.6-8 | Eliminate the reported 60s hang       |
| **v0.1.2** | P2.6-2, P2.6-4, P2.6-9 | Graceful recovery + better error copy |
| **v0.1.3** | P2.6-5, P2.6-6, P2.6-7 | Anti-ban traffic hardening            |

---

## Item Specifications

---

### P2.6-1 — Treat 429 and 403 as Fatal, Not Retryable

**Release:** v0.1.1
**Priority:** 🔴 High — this is the direct fix for the reported hang
**Touches:** `src/utils.rs`, `src/client.rs`

#### Problem

`with_retry` treats every error identically. An HTTP 429 or 403 response is
not a transient network glitch — retrying it immediately makes things worse by
consuming more of the rate-limit budget. The current flow on a 429:

```
429 received → retry after 100ms → 429 → retry after 200ms → 429 → retry
after 400ms → fail with anyhow::Error("Request failed with status: 400")
```

Per connection. With 8 download connections, the user waits through up to
`(100 + 200 + 400) × 8 = 5,600ms` of wasted backoff before seeing any output.

#### Fix: Non-retryable status sentinel

Introduce a `RateLimitError` type that `with_retry` recognises as a signal to
short-circuit without any retries:

```rust
// src/utils.rs

/// Marker error that tells with_retry to bail immediately without retrying.
/// Used for HTTP 429 / 403 where retrying is actively harmful.
#[derive(Debug)]
pub struct NonRetryableError(pub anyhow::Error);

impl std::fmt::Display for NonRetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub async fn with_retry<F, Fut, T>(max_retries: u32, mut f: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut last_err = anyhow::anyhow!("No attempts made");
    for attempt in 0..=max_retries {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                // If the closure wrapped the error as NonRetryable, bail instantly
                if let Some(nre) = e.downcast_ref::<NonRetryableError>() {
                    return Err(anyhow::anyhow!("{}", nre.0));
                }
                if attempt < max_retries {
                    let backoff = Duration::from_millis(100 * 2u64.pow(attempt));
                    debug!("Attempt {}/{} failed: {}. Retrying in {:?}…",
                        attempt + 1, max_retries + 1, e, backoff);
                    tokio::time::sleep(backoff).await;
                }
                last_err = e;
            }
        }
    }
    Err(last_err)
}
```

In `client.rs`, the status check inside every `with_retry` closure becomes:

```rust
// src/client.rs — shared helper used in both test_download and test_upload
fn check_status(r: &reqwest::Response) -> anyhow::Result<()> {
    match r.status() {
        s if s.is_success() => Ok(()),

        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            let wait_secs = r
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(900); // 15 minutes if Retry-After header is absent
            Err(anyhow::Error::new(NonRetryableError(anyhow::anyhow!(
                "You've been rate-limited by Cloudflare. \
                 Please wait {} minutes before running the test again.\n\n\
                 Alternatives:\n  \
                 • Use a custom server:  speedtest --server <URL>\n  \
                 • Run ping only:        speedtest --no-download --no-upload",
                wait_secs / 60
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
```

#### Before / After

| Scenario        | Before                                                | After                                      |
| --------------- | ----------------------------------------------------- | ------------------------------------------ |
| 429 received    | Retries 3× with backoff, ~700ms wasted per connection | Immediate bail, friendly message           |
| 403 received    | Retries 3× with backoff, generic "status 403"         | Immediate bail, Bot Fight Mode explanation |
| 500 received    | Retries 3× ✅ correct                                 | Retries 3× ✅ unchanged                    |
| Network timeout | Retries 3× ✅ correct                                 | Retries 3× ✅ unchanged                    |

---

### P2.6-2 — Respect `Retry-After` Header

**Release:** v0.1.2
**Priority:** 🟡 Medium
**Touches:** `src/client.rs` (already partially covered by P2.6-1)

#### Problem

When Cloudflare returns 429 it frequently includes a `Retry-After` header
specifying the exact cooldown in seconds. The tool currently ignores this
header entirely and either guesses (P2.6-1's 900s default) or provides no
timing guidance at all.

#### Fix

P2.6-1 already reads the `Retry-After` header within `check_status()`. This
item ensures the value is also:

1. **Stored** in the error context so the cooldown writer (P2.6-7) can read
   the server-recommended wait instead of using a hardcoded default.
2. **Displayed precisely** when the actual header value is present vs. the
   fallback:

```rust
// In check_status(), distinguish present vs. absent header in the message:
let (wait_secs, source) = r
    .headers()
    .get("retry-after")
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u64>().ok())
    .map(|s| (s, "server says"))
    .unwrap_or((900, "estimated"));

// Message example:
// "Rate limited. Wait 15 minutes (server says)."
// "Rate limited. Wait 15 minutes (estimated — no Retry-After header)."
format!("…Wait {} minutes ({}).", wait_secs / 60, source)
```

---

### P2.6-3 — Reduce Default Connection Counts

**Release:** v0.1.1
**Priority:** 🔴 High
**Touches:** `src/lib.rs`, `src/models.rs`

#### Problem

The current defaults open 8 + 4 = 12 simultaneous connections to
`speed.cloudflare.com` per user. When multiple users run the tool concurrently
from different IPs, the aggregate connection volume rapidly triggers Cloudflare's
rate limiter at the infrastructure level.

#### Fix

Lower the defaults to values that still accurately measure any home or
office connection up to approximately 1 Gbps:

| Direction | Old default | New default | Max accurate measurement      |
| --------- | ----------- | ----------- | ----------------------------- |
| Download  | 8           | 4           | ~1 Gbps on a 250ms RTT link   |
| Upload    | 4           | 2           | ~500 Mbps on a 250ms RTT link |

**In `src/lib.rs`:**

```rust
// run() function
let conns = args.connections.unwrap_or(4);   // was 8
// ...
let conns = args.connections.unwrap_or(2);   // was 4
```

**In `src/models.rs`:**

```rust
impl Default for MenuSettings {
    fn default() -> Self {
        Self {
            duration_secs: 10,
            connections: 4,    // was 8
            ping_count: 20,
            color: true,
        }
    }
}
```

The `--connections` flag is unchanged — power users on 10 Gbps+ links can
still pass `--connections 16`.

---

### P2.6-4 — Auto-Reduce Connections on First 429

**Release:** v0.1.2
**Priority:** 🟡 Medium
**Touches:** `src/lib.rs`, `src/client.rs`

#### Problem

P2.6-1 fails fast on 429, which is correct. But some 429 responses from
Cloudflare are triggered purely by connection concurrency — the endpoint would
accept the same request from a single connection. Bailing immediately on the
first 429 means these users get an error when a single-connection retry would
have succeeded.

#### Fix

Add a retry-at-reduced-concurrency layer above the per-request `with_retry`.
This lives in `lib.rs`, wrapping the `test_download` / `test_upload` calls:

```rust
// Pseudocode in lib.rs

async fn run_with_fallback_concurrency(
    test_fn: /* async fn */,
    initial_conns: usize,
    config: Arc<AppConfig>,
) -> anyhow::Result<f64> {
    match test_fn(initial_conns).await {
        Ok(speed) => Ok(speed),
        Err(e) if is_rate_limit_error(&e) && initial_conns > 1 => {
            if !config.quiet {
                eprintln!(
                    "⚠️  Rate limited at {} connections — retrying \
                     with 1 connection…",
                    initial_conns
                );
            }
            // Single retry with 1 connection — if this also 429s, bail
            test_fn(1).await
        }
        Err(e) => Err(e),
    }
}
```

`is_rate_limit_error()` checks whether the `anyhow::Error` originated from
a `NonRetryableError` wrapping a 429 (introduced in P2.6-1).

#### Decision tree

```
test_download(4 connections)
    │
    ├─ success → return speed
    │
    └─ 429 NonRetryableError
          │
          └─ retry: test_download(1 connection)
                │
                ├─ success → return speed (with warning shown)
                │
                └─ 429 again → bail with full rate-limit message (P2.6-1)
```

---

### P2.6-5 — Request Pacing with Random Jitter

**Release:** v0.1.3
**Priority:** 🟡 Medium
**Touches:** `src/client.rs`

#### Problem

Each worker in `test_download` and `test_upload` loops at maximum speed,
issuing requests the instant the previous one completes. This produces a
perfectly uniform, machine-generated request cadence — a primary signal used
by bot detection systems. Real browser traffic contains natural variation in
timing due to rendering, JavaScript execution, and user think time.

#### Fix

After each completed chunk response, sleep for a random interval before
issuing the next request. The interval is short enough to not affect
measurement accuracy but long enough to break the uniform pattern:

```rust
// In the 'request loop in both test_download and test_upload,
// after a full response body has been consumed:
let jitter_ms = rand::rng().random_range(50u64..=150);
tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
```

#### Impact on accuracy

The maximum jitter per request is 150ms. With a 10-second test duration and
2–4 connections, this adds at most 1–2 sleep intervals to the total elapsed
time per connection. The warm-up exclusion and `effective_duration` calculation
(already implemented) absorb this variance. Measured speed deviation is
estimated at < 1% for connections faster than 10 Mbps.

#### Where it does NOT apply

Jitter is not added between individual _chunk reads_ within a streaming
response — only between complete request/response cycles. Adding jitter inside
the stream reading loop would stall active connections and distort measurements.

---

### P2.6-6 — User-Agent Rotation

**Release:** v0.1.3
**Priority:** 🟡 Medium
**Touches:** `src/main.rs`

#### Problem

Every request currently carries `User-Agent: rust-speedtest/0.1.0`. This
string is an unambiguous bot identifier — Cloudflare's Bot Fight Mode can and
does use this header as a trivial ban signal.

#### Fix

Define a pool of realistic browser User-Agent strings in `main.rs` and select
one at random when the `reqwest::Client` is built. The selection happens once
per program invocation so all requests within a single test run share the same
User-Agent (consistent with real browser behaviour).

```rust
// src/main.rs

const USER_AGENTS: &[&str] = &[
    // Chrome on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    // Chrome on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    // Chrome on Linux
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    // Safari on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
    // Firefox on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) \
     Gecko/20100101 Firefox/125.0",
];

// In main(), before building the client:
let ua = USER_AGENTS[rand::rng().random_range(0..USER_AGENTS.len())];

let client = Client::builder()
    .user_agent(ua)
    .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
    .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
    .build()?;
```

#### Maintenance note

Browser User-Agent strings change with every major browser release. The pool
above is accurate as of Chrome 124 / Firefox 125 / Safari 17 (April 2026).
Update the strings on each major browser release cycle (~6 months). A comment
in the code should document the last update date.

#### Ethical note

Using browser User-Agent strings causes the tool's requests to appear to
originate from a web browser rather than a CLI program. This is the most
effective mitigation against Bot Fight Mode. Teams with concerns about this
form of identity spoofing can omit this item — P2.6-3 (lower connections) and
P2.6-5 (jitter) together provide meaningful risk reduction without changing
the tool's declared identity.

---

### P2.6-7 — Local Cooldown Enforcement (Disk-Persisted)

**Release:** v0.1.3
**Priority:** 🟡 Medium
**Touches:** `src/cooldown.rs` (new file), `src/main.rs`, `src/models.rs`, `Cargo.toml`

#### Problem

Users running the tool on a cronjob (e.g. `*/1 * * * * speedtest`) generate
a continuous stream of requests to Cloudflare's endpoint, triggering rate
limiting that affects other users. An in-memory cooldown would not solve this
— each cron invocation starts a fresh process with no memory of previous runs.
The cooldown **must be stored on disk** to work across process boundaries.

#### New file: `src/cooldown.rs`

```rust
// src/cooldown.rs

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_COOLDOWN_SECS: u64 = 300; // 5 minutes

/// Returns the platform-appropriate path for the last-run timestamp file.
/// Linux/macOS: ~/.local/share/speedtest/last_run
/// Windows:     %APPDATA%\speedtest\last_run
pub fn last_run_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("speedtest").join("last_run"))
}

/// Returns Some(seconds_remaining) if the cooldown is still active,
/// or None if the cooldown has elapsed or no previous run was recorded.
pub fn cooldown_remaining(cooldown_secs: u64) -> Option<u64> {
    let path = last_run_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let last_run_ts: u64 = contents.trim().parse().ok()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let elapsed = now.saturating_sub(last_run_ts);
    if elapsed < cooldown_secs {
        Some(cooldown_secs - elapsed)
    } else {
        None
    }
}

/// Writes the current Unix timestamp to the last-run file.
/// Creates the directory if it does not exist.
/// Called only on successful test completion — failed runs do not reset
/// the cooldown clock.
pub fn record_successful_run() -> anyhow::Result<()> {
    let path = last_run_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&path, now.to_string())?;
    Ok(())
}
```

#### Cooldown check in `main.rs`

```rust
// In run_app(), before calling cli_speedtest::run():
if !args.force_run {
    if let Some(remaining) = cooldown::cooldown_remaining(DEFAULT_COOLDOWN_SECS) {
        eprintln!(
            "⏳ Cooldown active. Last test ran recently.\n   \
             Wait {} more minutes, or override with: speedtest --force-run",
            remaining / 60 + 1
        );
        std::process::exit(1);
    }
}

// After a successful run(), record the timestamp:
cooldown::record_successful_run()?;
```

#### New CLI flag

```rust
// src/main.rs Args struct
/// Bypass the local cooldown and run the test immediately
#[arg(long, default_value_t = false)]
force_run: bool,
```

#### User experience

```
# Normal use — first run
$ speedtest
🚀 Starting Rust Speedtest...
[... test runs normally ...]

# Second run within 5 minutes
$ speedtest
⏳ Cooldown active. Last test ran recently.
   Wait 4 more minutes, or override with: speedtest --force-run

# Force override
$ speedtest --force-run
🚀 Starting Rust Speedtest...
[... test runs normally ...]
```

#### What does NOT reset the cooldown

- A run that fails with a network error
- A run that is rate-limited (429/403)
- A run aborted with Ctrl+C
- A run that fails validation (e.g. `--duration 1`)

Only a run that reaches `SpeedTestResult` serialisation and returns `Ok` calls
`record_successful_run()`.

#### Scope boundary

The cooldown file is intentionally minimal — a single Unix timestamp integer.
It is not a configuration file and does not store results. Result history is a
Phase 3 feature (CSV / NDJSON export).

---

### P2.6-8 — Global Test Timeout

**Release:** v0.1.1
**Priority:** 🟡 Medium
**Touches:** `src/main.rs`

#### Problem

Even with P2.6-1 in place, unforeseen blocking scenarios (DNS hang, stalled
TCP connection that never triggers a timeout, a third-party server that accepts
the connection but never sends bytes) could cause the tool to appear completely
frozen with no output.

#### Fix

Wrap the entire `run()` call in a `tokio::time::timeout`. This is a last-resort
guard — P2.6-1 should prevent hangs in practice — but it guarantees the tool
always terminates within a bounded window.

```rust
// src/main.rs

const GLOBAL_TEST_TIMEOUT_SECS: u64 = 120; // 2 minutes hard maximum

// In run_app(), replace the bare cli_speedtest::run() call with:
let result = tokio::time::timeout(
    Duration::from_secs(GLOBAL_TEST_TIMEOUT_SECS),
    cli_speedtest::run(run_args, config, client),
)
.await
.unwrap_or_else(|_| {
    Err(anyhow::anyhow!(
        "Test timed out after {}s. The server may be rate limiting \
         or unreachable.\n\n\
         Try a custom server: speedtest --server <URL>",
        GLOBAL_TEST_TIMEOUT_SECS
    ))
})?;
```

The 120s ceiling is deliberately generous — a legitimate 30s test with 2s
warm-up and retry overhead should never approach it. If the timeout fires, it
is almost certainly a server-side hang, not a slow connection.

---

### P2.6-9 — Consistent Rate-Limit Error Message with `--server` Suggestion

**Release:** v0.1.2
**Priority:** 🟢 Low
**Touches:** `src/client.rs`

#### Problem

After P2.6-1 ships, users who hit a rate limit will see a clear message. But
that message should always include a concrete next step — specifically the
`--server` flag, since switching to a private or alternative server is the most
reliable long-term workaround.

#### Standard error message template

All 429/403 error messages across the codebase must follow this structure:

```
[What happened]
[How long to wait, if known]

Alternatives:
  • Use a custom server:  speedtest --server https://your-server.example.com
  • Run ping only:        speedtest --no-download --no-upload
  • Force immediate run:  speedtest --force-run   (after cooldown ships in v0.1.3)
```

The `--force-run` suggestion is added once P2.6-7 ships. Until then, the
alternatives list omits it.

---

## Testing Requirements

### Unit tests

| Test                                | Location          | Asserts                                                                     |
| ----------------------------------- | ----------------- | --------------------------------------------------------------------------- |
| `non_retryable_error_skips_retry`   | `src/utils.rs`    | `with_retry` makes exactly 1 attempt when a `NonRetryableError` is returned |
| `retryable_error_uses_all_attempts` | `src/utils.rs`    | A regular `anyhow::bail!` still retries `max_retries + 1` times             |
| `check_status_success_passes`       | `src/client.rs`   | 2xx response returns `Ok(())`                                               |
| `check_status_429_is_non_retryable` | `src/client.rs`   | Returns `NonRetryableError`                                                 |
| `check_status_403_is_non_retryable` | `src/client.rs`   | Returns `NonRetryableError`                                                 |
| `check_status_500_is_retryable`     | `src/client.rs`   | Returns regular `anyhow::Error`                                             |
| `cooldown_none_when_no_file`        | `src/cooldown.rs` | Returns `None` when last-run file doesn't exist                             |
| `cooldown_none_when_elapsed`        | `src/cooldown.rs` | Returns `None` when timestamp is old                                        |
| `cooldown_some_when_active`         | `src/cooldown.rs` | Returns `Some(remaining)` when within window                                |
| `record_run_creates_file`           | `src/cooldown.rs` | File is created and contains a valid Unix timestamp                         |
| `record_run_creates_missing_dirs`   | `src/cooldown.rs` | Parent directories are created if absent                                    |

### Integration tests (mockito)

| Test                                | Asserts                                                                                                                      |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `download_bails_immediately_on_429` | `test_download` returns an error containing "rate-limited" without retrying (mock verifies exactly 1 request hit the server) |
| `download_bails_immediately_on_403` | Same pattern for 403 / "Bot Fight Mode"                                                                                      |
| `download_retries_on_500`           | `test_download` retries up to 3 times on a 500 response (mock verifies 3 requests)                                           |
| `upload_bails_immediately_on_429`   | Same as download variant                                                                                                     |
| `run_respects_global_timeout`       | `run()` returns an error if the server accepts connections but never sends data (mock hangs indefinitely)                    |

### Manual acceptance tests

```bash
# 1. Verify P2.6-1: confirm no hang on simulated 429
#    Run with a custom server that returns 429 immediately:
speedtest --server http://localhost:8080   # local mock returning 429
#    Expected: error message within < 1 second, mentions "rate-limited"

# 2. Verify P2.6-3: confirm new defaults
cargo run -- --help | grep connections    # should show no default in help
#    Start a test and check spinner — should open 4 download connections

# 3. Verify P2.6-6: confirm User-Agent rotation
#    Run with --debug and check stderr for the selected User-Agent
RUST_LOG=debug cargo run 2>&1 | grep -i user-agent

# 4. Verify P2.6-7: cooldown enforced across process boundaries
speedtest                         # run once (real or --no-download --no-upload)
speedtest                         # second run: must show cooldown message
speedtest --force-run             # must bypass cooldown and run

# 5. Verify cooldown file location
cat ~/.local/share/speedtest/last_run     # Linux/macOS
#    Must contain a Unix timestamp integer

# 6. Verify cooldown not reset on failed run
NO_COLOR=1 speedtest --server http://127.0.0.1:1  # guaranteed to fail
speedtest                         # must still show cooldown from the real run,
                                  # not reset by the failed one

# 7. Verify P2.6-8: global timeout fires on a hung server
#    Start a local server that accepts connections but never responds,
#    then run: speedtest --server http://localhost:9999
#    Expected: "timed out after 120s" error, tool exits cleanly
```

---

## Definition of Done

### v0.1.1

- [x] `NonRetryableError` type exists in `utils.rs`
- [x] `with_retry` short-circuits immediately on `NonRetryableError`
- [x] `check_status()` helper returns `NonRetryableError` for 429 and 403
- [x] 429 error message includes wait time from `Retry-After` or 15min fallback
- [x] 403 error message mentions Bot Fight Mode and `--server` flag
- [x] Default download connections changed to 4 in `lib.rs` and `models.rs`
- [x] Default upload connections changed to 2 in `lib.rs` and `models.rs`
- [x] Global 120s timeout wraps `run()` in `main.rs`
- [x] All unit tests for P2.6-1 pass (Missing unit/integration tests)
- [x] Integration test `download_bails_immediately_on_429` passes (Missing unit/integration tests)
- [x] `cargo clippy -- -D warnings` passes
- [x] `cargo fmt --check` passes

### v0.1.2

- [x] `Retry-After` header value vs. estimated fallback is distinguished in message
- [x] Auto-reduce-to-1-connection retry is implemented in `lib.rs`
- [x] All error messages include `--server` and `--no-download/--no-upload` suggestions

### v0.1.3

- [x] 50–150ms jitter added between chunk requests in `client.rs`
- [x] User-Agent pool defined in `main.rs`, one selected randomly at startup
- [x] `src/cooldown.rs` exists with `cooldown_remaining`, `record_successful_run`, `last_run_path`
- [x] `dirs = "5"` added to `Cargo.toml`
- [x] `--force-run` flag exists in `Args`
- [x] Cooldown check runs in `run_app()` before the test starts
- [x] `record_successful_run()` is called only on `Ok` result from `run()`
- [x] All cooldown unit tests pass (Missing tests for cooldown)
- [x] Manual acceptance tests 4, 5, and 6 pass
- [x] `TODO.md` Phase 2.6 items marked ✅

## 🐛 Bugs / Correctness Issues

**3. Ping uses `GET` instead of `HEAD`**
`find_best_server` correctly uses `.head()`, but `test_ping` switches to `.get()`, which downloads a response body and inflates the latency reading artificially.

**4. Test duration includes connection setup time**
The `start` timer fires *before* tasks are spawned. On slow connections, the TCP handshake overhead is silently baked into your "transfer duration," slightly under-reporting speeds.

---

## 📊 Measurement Quality (Makes Results Unreliable)

**6. No warm-up phase**
TCP slow-start means the first few seconds of a transfer are ramping up, not at full throughput. Production tools (fast.com, speedtest.net) discard the first 1–2 seconds and only measure the plateau. Your current implementation averages the ramp-up into the final number, under-reporting real speed.

**7. `num_connections` is hardcoded and non-configurable**
Download uses 8, upload uses 4, with no CLI flag to adjust. Users on 10 Gbps+ connections will saturate these long before the timer ends. This should be a `--connections` argument.

---

## ⚠️ Code Quality Issues

**8. `#[allow(unreachable_code)]` is a design smell**
Using an infinite `loop {}` and relying on `timeout()` to kill it is functional but opaque. The correct pattern is a `CancellationToken` (from `tokio-util`) or a `tokio::select!` with a shutdown channel, which makes intent explicit and avoids suppressing compiler warnings.

**9. `rand::thread_rng()` is deprecated**
In `rand` 0.9+, `thread_rng()` was removed. The modern API is:
```rust
// Old (deprecated)
rand::thread_rng().fill_bytes(&mut raw_payload);

// New
rand::rng().fill(&mut raw_payload[..]);
```

**10. Upload payload is generated once but the loop claims it's random per-request**
The spinner message says *"random data"* but the random bytes are generated once *before* the loop and the same `Bytes` clone is reused every iteration. This is actually fine for bandwidth testing, but the misleading label should be removed.

**11. `quiet` is threaded through every function signature**
This creates noisy function signatures and tight coupling. A `static` or thread-local config, or passing a single `AppConfig` struct, is the idiomatic solution.

---

## 🚀 Missing Production Features

**12. No retry / resilience logic**
If a single request in a connection loop fails (network blip, 5xx, etc.), the task panics with an error. There should be a small retry budget (e.g., 3 attempts with exponential backoff) before giving up.

**13. No `--server` / custom endpoint flag**
Users should be able to point the tool at their own `__down` / `__up` compatible server (e.g., for internal network testing), rather than being locked to Cloudflare.

**14. No jitter or packet loss metrics**
These are standard outputs of any production speedtest and are expected by users. Jitter comes for free once you fix issue #5 (multi-probe ping).

**15. No connection timeout / global timeout**
If `find_best_server` or an early request hangs, the CLI can stall indefinitely. A per-request `.timeout(Duration::from_secs(5))` on the `Client` builder is the minimum safeguard:
```rust
Client::builder()
    .timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(10))
    ...
```

**16. JSON output is missing jitter, timestamp, and tool version**
For scripting use cases, the JSON result should include:
```json
{
  "timestamp": "2026-04-03T10:00:00Z",
  "client_ip": "...",
  "ping_ms": 12,
  "jitter_ms": 1.4,
  "download_mbps": 450.2,
  "upload_mbps": 210.8,
  "tool_version": "0.1.0"
}
```

**17. No `--no-download` / `--no-upload` flags**
Common in production CLIs — allows users to run only one half of the test.

---

## 📦 Project / Release Readiness

**18. No `deny(warnings)` or `deny(clippy::all)` in CI**
Without this, warnings silently accumulate in release builds. Add to `lib.rs`/`main.rs`:
```rust
#![deny(warnings)]
```
or enforce it in your CI pipeline with `RUSTFLAGS="-D warnings"`.

**19. No structured logging levels beyond debug/error**
The gap between `error` and `debug` is too large. Add `info`-level logging for normal operational events (server selected, test started, etc.) so operators can diagnose issues without full debug noise.

**20. No integration/end-to-end tests**
The only tests are pure unit tests for `calculate_mbps`. There are no tests for the networking logic, no mock server, and no test for the JSON output format contract.

---

## Summary Checklist

| Category | Issue | Severity |
|---|---|---|
| Bug | Duplicate servers in pool | 🔴 High |
| Bug | Upload errors silently dropped | 🔴 High |
| Bug | `GET` ping inflates latency | 🟡 Medium |
| Bug | Timer starts before tasks spawn | 🟡 Medium |
| Measurement | Single-shot ping, no jitter | 🔴 High |
| Measurement | No TCP slow-start warm-up | 🔴 High |
| Measurement | Hardcoded connection count | 🟡 Medium |
| Code quality | `#[allow(unreachable_code)]` | 🟡 Medium |
| Code quality | Deprecated `thread_rng()` | 🟡 Medium |
| Code quality | `quiet` prop-drilled everywhere | 🟢 Low |
| Features | No retry logic | 🔴 High |
| Features | No request/connect timeout | 🔴 High |
| Features | No `--server` flag | 🟡 Medium |
| Features | JSON missing timestamp/version | 🟡 Medium |
| Features | No `--no-download/upload` flags | 🟢 Low |
| Release | No `deny(warnings)` in CI | 🟡 Medium |
| Release | No integration tests | 🟡 Medium |

The most critical blockers before shipping are issues **#1, #2, #5, #6, #12, and #15** — the rest are quality improvements that should follow shortly after.

-----

Reproducible benchmarks → you'd want to pin to a specific server, not anycast
Portability → Cloudflare's endpoints have no SLA or public API guarantee; they could change
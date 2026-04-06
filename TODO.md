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

## ✅ Phase 2.6: Rate Limit & Anti-Ban Hardening (Completed)

These issues were discovered in real-world multi-user testing. All items are now successfully implemented.

| Item   | Description                                 | Status   |
| ------ | ------------------------------------------- | -------- |
| P2.6-1 | Treat 429 and 403 as fatal, not retryable   | ✅ Fixed |
| P2.6-2 | Respect `Retry-After` header                | ✅ Fixed |
| P2.6-3 | Reduce default connection counts            | ✅ Fixed |
| P2.6-4 | Auto-reduce connections on first 429        | ✅ Fixed |
| P2.6-5 | Request pacing with random jitter           | ✅ Added |
| P2.6-6 | User-Agent rotation                         | ✅ Added |
| P2.6-7 | Local cooldown enforcement (disk-persisted) | ✅ Added |
| P2.6-8 | Global test timeout as a safety net         | ✅ Added |
| P2.6-9 | Consistent error message with `--server`    | ✅ Fixed |

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

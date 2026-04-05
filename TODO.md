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

## ✅ Phase 2: Visual Polish & UX (Completed)

| Item | Description | Status |
|---|---|---|
| P2-1 | Semantic color system (thresholds for Mbps/Ping) | ✅ Fixed |
| P2-2 | Live rolling-speed display (250ms interval) | ✅ Fixed |
| P2-3 | Speed rating labels in summary (Excellent/Great/etc) | ✅ Fixed |
| P2-4 | `--no-color` / `NO_COLOR` / non-TTY compliance | ✅ Fixed |
| P2-5 | Terminal-width-aware dynamic summary box | ✅ Fixed |

---

## ✅ Phase 2.5: Interactive Experience & Rendering Fix (Completed)

| Item | Description | Status |
|---|---|---|
| P2.5-1 | Summary box rendering fix (ANSI length awareness) | ✅ Fixed |
| P2.5-2 | Interactive main menu (TTY-only, flag-compatible) | ✅ Added |
| P2.5-3 | Interactive settings & interpretation guide | ✅ Added |

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

## 🛠️ Internal Maintenance & Polish

- [ ] **CI Pipeline**: Add a GitHub Action to run `cargo test`, `cargo fmt`, and `cargo clippy` on every push/PR.
- [ ] **README Update**: Document all new CLI flags (`--server`, `--no-download`, etc.) and fix the License placeholder.
- [ ] **Documentation**: Add Rustdoc comments to all public functions and enable `#[warn(missing_docs)]` in `lib.rs`.
- [ ] **Unit Test Expansion**: Add more granular unit tests for `models.rs` and `utils.rs`.
- [ ] **Refine Error Handling**: Implement custom Error types using `thiserror` for better programmatic error handling.
- [ ] **WASM Support**: Explore if the core library can be compiled to WASM for a browser-based version.
- [ ] **Crate Modularization**: Move the core measurement logic into a standalone crate `speedtest-core` if the library grows.
```

---

The ordering matters: **P2-2 (live speed)** is marked high because it's the most noticeable missing feature for a speedtest specifically — users expect to watch the number climb in real-time. **P2-1 (color system)** comes first in the list because `theme.rs` is a shared dependency that P2-2 and P2-3 both build on.
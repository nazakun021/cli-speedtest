# CLI Speedtest TODO

The core measurement engine and CLI interface are now stable and production-ready. All previously identified bugs and architectural issues have been resolved.

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

## 🚀 Phase 2: Advanced Features (Next)

These items are proposed for the next iteration of the tool.

- [ ] **Multi-Server Selection**: Automatically find the closest server or allow a list of servers to be tested.
- [ ] **CSV / NDLJSON Record Export**: Append results to a local file for history tracking.
- [ ] **Adaptive Connection Scaling**: Automatically increase connections if saturation isn't reached.
- [ ] **Latency Histogram**: Provide a more detailed breakdown of ping distribution.
- [ ] **ISP / Location Metadata**: Integrate with an IP-API to show local ISP and city in the summary.
- [ ] **Better Error Reporting for Custom Servers**: Better validation when a custom `--server` doesn't support the Cloudflare-specific `/__down` or `/__up` endpoints.

---

## 🛠️ Internal Maintenance

- [ ] **CI Pipeline**: Add a GitHub Action to run `cargo test` and `cargo fmt` on every push.
- [ ] **WASM Support**: Explore if the core library can be compiled to WASM for a browser-based version.
- [ ] **Crate Modularization**: Move the core measurement logic into a standalone crate `speedtest-core` if the library grows.
## ❌ TODO Says "Done" — Code Says Otherwise

These are the most important issues because they create a false sense of completion.

**#8 — `#[allow(unreachable_code)]` / CancellationToken**
The TODO claims this was replaced with `CancellationToken`. It was not. Both `test_download` and `test_upload` in `client.rs` still use the exact same `timeout()` + infinite `loop {}` pattern with the suppressed warning:
```rust
// Still present in client.rs — lines ~60 and ~130
#[allow(unreachable_code)]
Ok::<(), anyhow::Error>(())
```

**#11 — Global Config / No more prop-drilling**
The TODO says `quiet` was eliminated via a `TestConfig` struct. Every function in `client.rs` still accepts `quiet: bool` as a parameter. This was not done.

**#13 — `--server` flag**
Not present in the `Args` struct in `main.rs`. There is no `--server` field.

**#17 — `--no-download` / `--no-upload` flags**
Also not present in `Args`. The `run_app` function always runs both tests unconditionally.

---

## 🐛 Bugs Remaining in the Shared Code

**`rand::thread_rng()` is still deprecated (issue #9)**
`client.rs` line ~103 still calls `rand::thread_rng().fill_bytes(...)`. Your `Cargo.toml` pins `rand = "0.8"` which is why it compiles, but you're carrying a known-deprecated API on an outdated minor version. The fix is to bump to `rand = "0.9"` and update the call:
```rust
// Replace this:
rand::thread_rng().fill_bytes(&mut raw_payload);

// With this (rand 0.9+):
rand::rng().fill(&mut raw_payload[..]);
```

**`WARMUP_SECS` is defined twice**
It's declared as a `const` in both `client.rs` and `main.rs`. This isn't a compiler error, but it's a maintenance hazard — changing one won't change the other. It belongs in one place, either in `utils.rs` or re-exported from `models.rs`.

**`start` timer fires before tasks are spawned**
In both `test_download` and `test_upload`, `let start = Instant::now()` is called before the `for _ in 0..num_connections` loop. On systems under load, the spawning overhead is baked into your measurement window. The timer should start after the last `tokio::spawn` call, or you should use a `Barrier` to synchronize all workers before any of them begin transferring.

---

## ⚠️ `Cargo.toml` Issues

**Wrong email syntax in `authors`**
```toml
# Wrong — brackets inside angle brackets
authors = ["Tirso Benedict J. Naza <[benedictnaza@gmail.com]>"]

# Correct
authors = ["Tirso Benedict J. Naza <benedictnaza@gmail.com>"]
```

**Outdated dependencies**
| Crate | Pinned | Current | Risk |
|---|---|---|---|
| `reqwest` | `0.11` | `0.12` | API differences, security patches |
| `rand` | `0.8` | `0.9` | Deprecated API still being called |
| `clap` | `4.4` | `4.5` | Minor |

**No `rust-version` (MSRV) field**
For a published CLI tool, you should declare the minimum supported Rust version so users get a clear error instead of a cryptic compile failure:
```toml
rust-version = "1.75"  # or whatever your actual minimum is
```

---

## 🟡 Remaining Quality Issues

**Upload spinner label is still misleading**
The spinner still reads `"Uploading (random data)..."`. The random bytes are generated once before the loop and the same buffer is reused every iteration. The label is technically false. Change it to just `"Uploading..."`.

**`--connections` applies asymmetrically without documentation**
```rust
let down_connections = args.connections.unwrap_or(8);
let up_connections = args.connections.unwrap_or(4);
```
A user passing `--connections 6` will get 6 download connections and 6 upload connections, but the defaults are asymmetric (8 vs 4). This behavior is undocumented in the help text. The arg comment should be updated, or you should add separate `--down-connections` and `--up-connections` flags.

**No `--ping-count` validation**
`args.ping_count` has no lower bound check. Passing `--ping-count 0` will cause `test_ping_stats` to return an "All ping attempts failed" error rather than a meaningful validation message. Add a guard:
```rust
if args.ping_count == 0 {
    anyhow::bail!("--ping-count must be at least 1");
}
```

---

## Updated Checklist

| Category | Issue | Severity | Actual Status |
|---|---|---|---|
| Bug | Upload errors silently dropped | 🔴 High | ✅ Fixed |
| Bug | `GET` ping inflates latency | 🟡 Medium | ✅ Fixed |
| Bug | Duplicate `WARMUP_SECS` | 🟡 Medium | 🔴 Not fixed |
| Bug | Timer starts before task spawn | 🟡 Medium | 🔴 Not fixed |
| Bug | Deprecated `thread_rng()` | 🟡 Medium | 🔴 Not fixed |
| Measurement | Single-shot ping, no jitter | 🔴 High | ✅ Fixed |
| Measurement | No TCP slow-start warm-up | 🔴 High | ✅ Fixed |
| Measurement | Hardcoded connection count | 🟡 Medium | ✅ Fixed |
| Code quality | `#[allow(unreachable_code)]` | 🟡 Medium | 🔴 Claimed done, not done |
| Code quality | `quiet` prop-drilled | 🟢 Low | 🔴 Claimed done, not done |
| Features | No retry logic | 🔴 High | ✅ Fixed |
| Features | No connect/request timeout | 🔴 High | ✅ Fixed |
| Features | No `--server` flag | 🟡 Medium | 🔴 Claimed done, not done |
| Features | No `--no-download/upload` | 🟢 Low | 🔴 Claimed done, not done |
| Features | JSON missing timestamp/version | 🟡 Medium | ✅ Fixed |
| Release | `authors` email malformed | 🟢 Low | 🔴 Not fixed |
| Release | No MSRV in `Cargo.toml` | 🟢 Low | 🔴 Not fixed |
| Release | Outdated `reqwest`/`rand` | 🟡 Medium | 🔴 Not fixed |
| Release | No integration tests | 🟡 Medium | Claimed done — not visible in shared code |

The foundation is solid and the measurement correctness issues are genuinely resolved. The main thing to address before shipping is reconciling your TODO with what's actually in the code — several "completed" items are still open.
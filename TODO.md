## ✅ Completed (Critical Blockers Cleared)

*   [x] **#2 Upload errors caught**: Errors in the upload loop are no longer silently dropped.
*   [x] **#3 Head-based ping**: Latency probes now use `HEAD` requests to avoid body download overhead.
*   [x] **#5 Multi-probe Ping & Jitter**: Implemented multi-probe measurement with jitter and packet loss.
*   [x] **#6 TCP Warm-up phase**: Added a 2-second warm-up period to exclude TCP slow-start from measurements.
*   [x] **#12 Resilience & Retries**: Added `with_retry` helper with exponential backoff for transient failures.
*   [x] **#15 Global Timeouts**: Configured connect (10s) and request (30s) timeouts on the HTTP client.

---

## 🚀 Planned Production Features

**#7 Configurable Connection Count**
Add a `--connections` argument. Currently hardcoded (8 for download, 4 for upload).

**#13 Custom Server Support**
Add a `--server` flag to allow users to specify their own `__down`/`__up` endpoints.

**#16 Detailed JSON Output**
Include `jitter`, `timestamp`, `packet_loss`, and `version` in the JSON result.

**#17 Test Selection Flags**
Add `--no-down` and `--no-up` to allow running partial tests.

---

## 🧹 Code Quality & Maintenance

**#8 Refactor Shutdown Logic**
Replace `#[allow(unreachable_code)]` with `CancellationToken` or a cleaner `tokio::select!` shutdown pattern.

**#11 Global Config Context**
Stop prop-drilling `quiet` (from `args.json`) through every function signature. Use a context struct or similar.

**#18 CI Readiness**
Implement `deny(warnings)` and add comprehensive integration tests (mocking server responses).

**#20 Integration Tests**
The only tests are pure unit tests for `calculate_mbps`. There are no tests for the networking logic, no mock server, and no test for the JSON output format contract.

---

## Summary Checklist

| Category | Issue | Severity | Status |
|---|---|---|---|
| Bug | Upload errors silently dropped | 🔴 High | ✅ Fixed |
| Bug | `GET` ping inflates latency | 🟡 Medium | ✅ Fixed |
| Measurement | Single-shot ping, no jitter | 🔴 High | ✅ Fixed |
| Measurement | No TCP slow-start warm-up | 🔴 High | ✅ Fixed |
| Features | No retry logic | 🔴 High | ✅ Fixed |
| Features | No request/connect timeout | 🔴 High | ✅ Fixed |
| Measurement | Hardcoded connection count | 🟡 Medium | ⏳ Pending |
| Features | No `--server` flag | 🟡 Medium | ⏳ Pending |
| Features | JSON missing timestamp/version | 🟡 Medium | ⏳ Pending |
| Code quality | `#[allow(unreachable_code)]` | 🟡 Medium | ⏳ Pending |
| Code quality | `quiet` prop-drilled everywhere | 🟢 Low | ⏳ Pending |
| Release | No integration tests | 🟡 Medium | ⏳ Pending |

# CLI Speedtest Operations Guide

This document describes the current operational contract of `cli-speedtest` v0.1.5 for people running it manually and from automation.

## Supported Interfaces

- **Interactive Mode** starts only when stdout is a TTY and no execution-affecting flag is supplied.
- **Direct Mode** runs immediately when a measurement or execution option is supplied, such as `--duration`, `--connections`, `--server`, `--quick`, `--force-run`, `--json`, or `--self-update`. It is the supported interface for scripts and monitoring. `--debug` and `--no-color` alone do not leave Interactive Mode on a TTY.
- **JSON output** is written to stdout. A successful run emits a result object. A failed run emits `{ "error": "..." }` and exits with status `1`. Cancellation exits with status `130`.
- **Diagnostic output** and interactive progress are written to stderr or suppressed by `--json`; do not parse human-readable output in automation.

## Interactive TUI

The main menu displays the active measurement mode, duration, connection count, ping probes, and Cooldown state before a test starts. **Run Configured Test** respects the selected Default Test Mode. **Run One-Off Quick Test** always runs Quick Mode and explains that Warm-up is skipped and successful runs count toward the five-test Quick Burst limit.

Commands and the results guide use framed panels on normal terminals and a compact unboxed layout below 64 columns. Server selection and download/upload toggles remain Direct Mode capabilities; they are not yet available in Interactive Settings.

## Provider-Friendly Operation

The default Cloudflare provider is shared infrastructure. A successful Integrity Mode test records a local **Cooldown** of five minutes. Quick Mode bypasses the Warm-up and standard Cooldown, but only permits one **Quick Burst** of five successful tests before the same Cooldown applies. `--force-run` is an explicit override that also resets the Quick Burst count; reserve it for intentional troubleshooting.

Default concurrency is four download connections and two upload connections. Supplying `--connections` applies one value to both directions, so use it conservatively when targeting a shared Provider.

## Accuracy and Limits

Integrity Mode discards the first two seconds of download and upload data as **Warm-up**. Tests therefore require a duration greater than two seconds. Quick Mode accepts a positive duration but can report lower throughput during TCP slow start.

Metrics describe the path to the selected Provider at the time of the test. Only successful HTTP latency probes contribute to ping statistics; timeouts and non-success HTTP responses count as Packet Loss. Results are not a complete ISP diagnosis and should not be compared across different providers, regions, Wi-Fi conditions, or connection counts.

Custom providers must implement compatible `GET /__down`, `POST /__up`, and `GET /cdn-cgi/trace` endpoints. Before a custom-provider measurement, the CLI preflights the latency endpoint plus only the selected throughput directions with tiny requests. Incompatible URLs fail with the endpoint and HTTP status before Warm-up or high-concurrency traffic begins.

## Local State and Updates

Local state is stored in the platform data directory: commonly `~/Library/Application Support/speedtest/` on macOS, `$XDG_DATA_HOME/speedtest/` or `~/.local/share/speedtest/` on Linux, and the local application-data directory on Windows. The relevant files are:

- `last_run`: completion time used for the Cooldown.
- `burst_count`: completed Quick Mode runs in the current Quick Burst.
- `last_update_check`: timestamp that limits interactive update checks to once per 24 hours.

Interactive update checks can be disabled with `NO_UPDATE` or `CLI_SPEEDTEST_NO_UPDATE`. Direct Mode never performs automatic updates. Manual `--self-update` downloads the platform-specific release asset and its adjacent `.sha256` checksum, verifies the SHA-256 digest, then replaces the current executable. This detects corruption and mismatched assets, but it does not protect against a compromised GitHub release or build pipeline. System-wide installations may require a manual update when the executable is not writable.

## Release Readiness

Before a release, run:

```zsh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit
```

Interactive startup is covered by an injected main-menu selector in tests, so it does not require a TTY or stdin.

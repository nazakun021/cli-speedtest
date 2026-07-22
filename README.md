# CLI Speedtest (Rust)

A production-grade, resilient CLI speedtest tool written in Rust. Designed for modern developers and system administrators who need accurate, machine-readable network performance metrics without the bloat—optimized for reliability against public infrastructure.

[![Release](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml/badge.svg)](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml)

## Features

- **Interactive by Default**: User-friendly TTY menu for manual tests and settings. Automatically switches to script-mode when flags are provided or in non-TTY environments.
- **Resilient Network Engine**:
  - **Provider-Friendly Design**: Built-in 5-minute local cooldown for standard runs. Supports **Quick Mode** (bypasses warm-up and standard cooldown) with a burst limit of 5 successive runs.
  - **Anti-Ban Hardening**: Implements User-Agent rotation and request pacing (jitter) to ensure consistent connectivity.
  - **Adaptive Fallback**: Automatically scales down to a single connection if rate-limited, ensuring the test completes even on restrictive networks.
- **Production Grade Accuracy**:
  - **Warm-up Phase**: Discards the first 2 seconds of transfer data to avoid TCP slow-start bias (bypassed in Quick Mode).
  - **High Concurrency**: Multi-threaded engine using `tokio` tasks and `Barrier` synchronization to saturate high-speed links.
- **Comprehensive Metrics**:
  - **Latency**: Min, Max, Average, Jitter, and Packet Loss.
  - **Throughput**: Real-time Mbps for both Download and Upload.
- **Self-Update**: Checks for updates on interactive startup at most once every 24 hours, prompts for confirmation, verifies the downloaded binary with SHA-256, and performs an in-place replacement.
- **Visual Polish**: Semantic color-coding (Mbps/Ping thresholds) and live rolling-speed displays.
- **Machine-Readable**: Use the `--json` flag for clean, parseable output perfect for cron jobs and monitoring.

## Installation

### Using Cargo (Recommended for Rust Users)

If you have [Rust](https://rustup.rs/) installed, you can install the CLI easily:

```zsh
cargo install cli-speedtest
```

### Pre-compiled Binaries (GitHub Releases)

You can directly download and install the latest pre-compiled binaries from the terminal.

**Linux (amd64):**

```bash
curl -L https://github.com/nazakun021/cli-speedtest/releases/latest/download/speedtest-linux-amd64 -o cli-speedtest
chmod +x cli-speedtest
sudo mkdir -p /usr/local/bin
sudo mv cli-speedtest /usr/local/bin/
```

**macOS (Apple Silicon):**

```bash
curl -L https://github.com/nazakun021/cli-speedtest/releases/latest/download/speedtest-macos-arm64 -o cli-speedtest
chmod +x cli-speedtest
sudo mkdir -p /usr/local/bin
sudo mv cli-speedtest /usr/local/bin/
```

**macOS (Intel):**

```bash
curl -L https://github.com/nazakun021/cli-speedtest/releases/latest/download/speedtest-macos-intel -o cli-speedtest
chmod +x cli-speedtest
sudo mkdir -p /usr/local/bin
sudo mv cli-speedtest /usr/local/bin/
```

> **Note for macOS Users**: If you get a "command not found" error, ensure `/usr/local/bin` is in your `$PATH`.
> If macOS prevents the binary from running due to an "Unidentified Developer" warning (Gatekeeper), run:
> `sudo xattr -d com.apple.quarantine /usr/local/bin/cli-speedtest`

**Windows (PowerShell):**

```powershell
Invoke-WebRequest -Uri "https://github.com/nazakun021/cli-speedtest/releases/latest/download/speedtest-windows-amd64.exe" -OutFile "cli-speedtest.exe"
# The executable will be available in your current directory as `cli-speedtest.exe`
```

### From Source

You will need the [Rust toolchain](https://rustup.rs/) installed.

```zsh
git clone https://github.com/nazakun021/cli-speedtest.git
cd cli-speedtest
cargo build --release
```

The binary will be available at `target/release/cli-speedtest`.

## Usage

### Interactive Mode (Human Interface)

Simply run the installed binary without flags to enter the interactive menu for manual checks:

```zsh
cli-speedtest
```

### Direct Mode (Machine Interface)

Pass a measurement or execution flag to bypass the menu and run directly. Direct Mode is optimized for scripting and automation:

| Flag                    | Description                                                  | Default            |
| ----------------------- | ------------------------------------------------------------ | ------------------ |
| `-d, --duration <SECS>` | Length of the test in seconds                                | `10`               |
| `-c, --connections <N>` | Number of parallel connections                               | `4` (DL), `2` (UL) |
| `--server <URL>`        | Custom Provider base URL; selected endpoints are preflighted | Cloudflare         |
| `--ping-count <N>`      | Number of pings to send                                      | `20`               |
| `--no-download`         | Skip the download test                                       | -                  |
| `--no-upload`           | Skip the upload test                                         | -                  |
| `--json`                | Output results in JSON format                                | -                  |
| `--no-color`            | Disable terminal styling                                     | -                  |
| `--debug`               | Enable verbose logging                                       | -                  |
| `--force-run`           | Bypass the local cooldown and run immediately                | -                  |
| `--quick`               | Bypass warm-up and cooldown (Quick Mode)                     | -                  |
| `--self-update`         | Check for updates and install latest immediately             | -                  |

### Environment Variables

You can configure the tool using the following environment variables:

| Variable                                 | Description                                                                            |
| ---------------------------------------- | -------------------------------------------------------------------------------------- |
| `NO_COLOR`                               | Disables ANSI terminal styling/coloring (also honors the standard `NO_COLOR` env var). |
| `NO_UPDATE` or `CLI_SPEEDTEST_NO_UPDATE` | Disables automated background update checks on startup.                                |

### Examples

```zsh
# Run a 5-second test with 12 connections (Bypasses auto-defaults)
cli-speedtest --duration 5 --connections 12

# Skip upload test and get JSON output for monitoring
cli-speedtest --no-upload --json

# Use a custom server
cli-speedtest --server https://your-custom-speedtest-server.com
```

## JSON Output Schema

When running with `--json`, the tool returns a structured object. Note that latency limits `min_ms` and `max_ms` are integers (`u128`). Throughputs `download_mbps` and `upload_mbps` are optional and will be completely omitted from the JSON output if their respective tests are skipped (e.g., via `--no-download` or `--no-upload`).

```json
{
  "timestamp": "2026-04-05T12:00:00Z",
  "version": "0.1.4",
  "server_name": "Cloudflare",
  "ping": {
    "min_ms": 10,
    "max_ms": 25,
    "avg_ms": 15.1,
    "jitter_ms": 2.3,
    "packet_loss_pct": 0.0
  },
  "download_mbps": 450.2,
  "upload_mbps": 120.5
}
```

## How It Works

`cli-speedtest` is built on standard HTTP primitives, optimized for the Cloudflare infrastructure but designed for future provider extensibility:

1. **Ping Phase**: Measures cold latency using lightweight HEAD requests. Calculates min, max, average, jitter, and packet loss.
2. **Download Phase**: Spawns concurrent `tokio` tasks that stream chunks from the provider. Implements **request pacing** to break machine-like patterns.
3. **Upload Phase**: Generates random, uncompressible payload data in-memory and POSTs to the provider, ensuring network compression doesn't skew throughput results.
4. **Resiliency Layer**: If the provider returns a rate-limit signal (HTTP 429), the engine fails fast with clear guidance or automatically falls back to a single-connection retry if appropriate.
5. **Warm-up Period**: Discards results from the first 2 seconds (the warm-up phase) to ensure measurements reflect saturated connection speeds, not TCP slow-start artifacts.
6. **Real-time Engine**: A periodic tick polls atomic byte counters to display live rolling-window throughput.

## Testing

The project includes unit tests for core logic and integration tests using mock servers (via `mockito`).

```zsh
# Run all tests
cargo test

# Run tests with logging enabled
RUST_LOG=debug cargo test
```

## Documentation

Start with [docs/README.md](docs/README.md) for the documentation map. Operational behavior, local state, automation guarantees, and measurement limits are in [docs/OPERATIONS.md](docs/OPERATIONS.md).

## Project Structure

- `src/main.rs`: Entry point and CLI routing; manages User-Agent rotation and global timeouts.
- `src/lib.rs`: Core orchestration logic and adaptive concurrency fallback.
- `src/client.rs`: High-concurrency network architecture and request pacing (jitter).
- `src/cooldown.rs`: Disk-persisted local cooldown enforcement.
- `src/updater.rs`: Performs GitHub release checking, checksum validation, and self-updating.
- `src/menu.rs`: Interactive TTY menu and settings.
- `src/models.rs`: Data structures and JSON serialization models.
- `src/utils.rs`: Technical constants (like `WARMUP_SECS`) and measurement math.
- `src/theme.rs`: ANSI color system and UI rendering helpers.

## Contributing

We welcome contributions! Whether it's adding new features, fixing bugs, or improving documentation, your help is appreciated.

### Local Development Setup

1. **Fork & Clone**: Fork the repository on GitHub and clone your fork locally.
2. **Build the Project**: `cargo build`
3. **Run the Binary**: `cargo run -- --debug`

### Code Style & Quality

```zsh
cargo fmt        # Auto-format your code
cargo clippy     # Run the linter
cargo test       # Ensure all tests pass
```

## License

This project is dual-licensed under the MIT and Apache 2.0 licenses. See `LICENSE-MIT` and `LICENSE-APACHE` for more information.

---

Built by [Tirso Benedict J. Naza](https://github.com/nazakun021)

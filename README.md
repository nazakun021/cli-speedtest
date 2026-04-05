# 🚀 CLI Speedtest (Rust)

A blazing-fast, high-performance CLI speedtest tool written in Rust. Designed for modern developers and system administrators who need accurate, machine-readable network performance metrics without the bloat.

[![Release](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml/badge.svg)](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml)

## ✨ Features

- **Interactive Menu**: User-friendly TTY menu for quick tests, settings, and help.
- **Blazing Fast**: Powered by `tokio` for asynchronous, non-blocking I/O.
- **High Concurrency**: Multi-threaded download and upload tests using `tokio` tasks and `Barrier` synchronization to saturate high-speed connections.
- **Production Grade**: 
  - **Warm-up Phase**: Includes a 2-second warm-up period to avoid TCP slow-start bias for more consistent results.
  - **Retry Logic**: Built-in exponential backoff for failed network requests.
- **Comprehensive Metrics**: 
  - **Latency**: Min, Max, Average, Jitter, and Packet Loss.
  - **Throughput**: Real-time Mbps for both Download and Upload.
- **Visual Polish**: Semantic color-coding and live rolling-speed displays.
- **Machine-Readable**: Use the `--json` flag for clean, parseable output perfect for cron jobs and monitoring.
- **Graceful Degradation**: Automatically disables color and interactive features when piped or in non-TTY environments. Respects the `NO_COLOR` environment variable.

## 🛠️ Installation

### From Source
You will need the [Rust toolchain](https://rustup.rs/) installed.

```zsh
git clone https://github.com/nazakun021/cli-speedtest.git
cd cli-speedtest
cargo build --release
```

The binary will be available at `target/release/cli-speedtest`.

## 🚀 Usage

### Interactive Mode
Simply run the binary without flags to enter the interactive menu:
```zsh
cargo run
```

### Direct Run (Scripting Friendly)
Pass any configuration flag to bypass the menu and run directly:

| Flag | Description | Default |
|------|-------------|---------|
| `-d, --duration <SECS>` | Length of the test in seconds | `10` |
| `-c, --connections <N>` | Number of parallel connections | `8` (DL), `4` (UL) |
| `--server <URL>` | Custom target server URL | Cloudflare |
| `--ping-count <N>` | Number of pings to send | `20` |
| `--no-download` | Skip the download test | - |
| `--no-upload` | Skip the upload test | - |
| `--json` | Output results in JSON format | - |
| `--no-color` | Disable terminal styling | - |
| `--debug` | Enable verbose logging | - |

### Examples
```zsh
# Run a 5-second test with 12 connections
cargo run -- --duration 5 --connections 12

# Skip upload test and get JSON output
cargo run -- --no-upload --json

# Use a custom server
cargo run -- --server https://your-custom-speedtest-server.com
```

## 📊 JSON Output Schema
When running with `--json`, the tool returns a structured object:
```json
{
  "timestamp": "2026-04-05T12:00:00Z",
  "version": "0.1.0",
  "server_name": "Cloudflare",
  "ping": {
    "min_ms": 10.5,
    "max_ms": 25.2,
    "avg_ms": 15.1,
    "jitter_ms": 2.3,
    "packet_loss_pct": 0.0
  },
  "download_mbps": 450.2,
  "upload_mbps": 120.5
}
```

## 🧪 Testing

The project includes unit tests for core logic and integration tests using mock servers.

```zsh
cargo test
```

## 🏗️ Project Structure

- `src/main.rs`: Entry point and CLI routing logic using `clap`.
- `src/lib.rs`: Core orchestration logic for ping, download, and upload phases.
- `src/client.rs`: High-concurrency networking implementation using `tokio` and `reqwest`.
- `src/menu.rs`: Interactive TTY menu and settings using `dialoguer`.
- `src/models.rs`: Data structures and JSON serialization models.
- `src/utils.rs`: Technical constants (like `WARMUP_SECS`) and measurement math.
- `src/theme.rs`: ANSI color system and UI rendering helpers.

## 🤝 Contributing

Feel free to open issues or submit pull requests to improve the tool!

## 📜 License

Distributed under the Apache License 2.0. See `LICENSE` for more information.

---
Built with ❤️ by [Tirso Benedict J. Naza](https://github.com/nazakun021)

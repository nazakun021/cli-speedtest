# CLI Speedtest (Rust)

A blazing-fast, high-performance CLI speedtest tool written in Rust. Designed for modern developers and system administrators who need accurate, machine-readable network performance metrics without the bloat.

[![Release](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml/badge.svg)](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml)

## Features

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

### Interactive Mode
Simply run the installed binary without flags to enter the interactive menu:
```zsh
cli-speedtest
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
cli-speedtest --duration 5 --connections 12

# Skip upload test and get JSON output
cli-speedtest --no-upload --json

# Use a custom server
cli-speedtest --server https://your-custom-speedtest-server.com
```

## JSON Output Schema
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

## How It Works

Under the hood, `cli-speedtest` is built for maximum throughput and accuracy, utilizing Rust's powerful asynchronous ecosystem:

1. **Ping Phase**: The tool sends lightweight HTTP requests to the target server to measure cold latency. It records minimum, maximum, average latency, and calculates jitter and packet loss.
2. **Download Phase**: Spawns multiple concurrent `tokio` tasks (default: 8) that independently stream chunks of data from the target server. A shared atomic counter tracks total bytes received in real-time.
3. **Upload Phase**: Spawns concurrent tasks (default: 4) that generate random, uncompressible payload data in-memory and POST them to the server. This guarantees that network compression doesn't skew the results.
4. **Warm-up Period**: Before calculating the final speed, the engine discards the results from the first 2 seconds (the warm-up phase). This avoids TCP slow-start artifacts and ensures we're measuring the saturated connection speed.
5. **Real-time Engine**: A periodic tick (e.g., every 100ms) polls the byte counters and calculates the rolling window throughput to display the live speed on your terminal.

## Testing

The project includes unit tests for core logic and integration tests using mock servers (via `mockito`).

```zsh
# Run all tests
cargo test

# Run tests with logging enabled
RUST_LOG=debug cargo test
```

## Project Structure

- `src/main.rs`: Entry point and CLI routing logic using `clap`.
- `src/lib.rs`: Core orchestration logic for ping, download, and upload phases.
- `src/client.rs`: High-concurrency network architecture using `tokio` and `reqwest`.
- `src/menu.rs`: Interactive TTY menu and settings using `dialoguer`.
- `src/models.rs`: Data structures and JSON serialization models.
- `src/utils.rs`: Technical constants (like `WARMUP_SECS`) and measurement math.
- `src/theme.rs`: ANSI color system and UI rendering helpers.

## Contributing

We welcome contributions! Whether it's adding new features, fixing bugs, or improving documentation, your help is appreciated.

### Local Development Setup

1. **Fork & Clone**: Fork the repository on GitHub and clone your fork locally:
   ```zsh
   git clone https://github.com/nazakun021/cli-speedtest.git
   cd cli-speedtest
   ```

2. **Build the Project**: Compile the binary.
   ```zsh
   cargo build
   ```

3. **Run the Binary**: Test your changes locally. You can use the `--debug` flag for verbose output!
   ```zsh
   cargo run -- --debug
   ```

### Code Style & Quality
Before submitting a pull request, please ensure your code adheres to standard Rust formatting and linting rules:
```zsh
cargo fmt        # Auto-format your code
cargo clippy     # Run the linter
cargo test       # Ensure all tests pass
```

### Submitting a Pull Request
1. Create a new branch: `git checkout -b feature/your-feature-name`
2. Commit your changes: `git commit -m "feat: add new awesome feature"`
3. Push to your branch: `git push origin feature/your-feature-name`
4. Open a Pull Request on GitHub!

## License

Distributed under the Apache License 2.0. See `LICENSE` for more information.

---
Built with ❤️ by [Tirso Benedict J. Naza](https://github.com/nazakun021)

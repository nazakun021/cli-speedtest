# 🚀 CLI Speedtest (Rust)

A blazing-fast, high-performance CLI speedtest tool written in Rust. Designed for modern developers and system administrators who need accurate, machine-readable network performance metrics without the bloat.

[![Release](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml/badge.svg)](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml)

## ✨ Features

- **Interactive Menu**: User-friendly TTY menu for quick tests, settings, and help.
- **Blazing Fast**: Powered by `tokio` for asynchronous, non-blocking I/O.
- **High Concurrency**: Multi-threaded download and upload tests to saturate high-speed connections.
- **Visual Polish**: Semantic color-coding and live rolling-speed displays.
- **Machine-Readable**: Use the `--json` flag for clean, parseable output perfect for cron jobs and monitoring.
- **Production Grade**: Time-bound testing architecture with warm-up windows for consistent results.
- **Graceful Degradation**: Automatically disables color and interactive features when piped or in non-TTY environments.

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
```zsh
# Run a 5-second test with 12 connections
cargo run -- --duration 5 --connections 12
```

### JSON Output
For automation and scripts, suppress the UI and get a clean JSON string:
```zsh
cargo run -- --json
```

### Advanced Options
```zsh
# Skip upload test
cargo run -- --no-upload

# Use a custom server
cargo run -- --server https://your-custom-speedtest-server.com

# Disable all color output
cargo run -- --no-color
```

## 🧪 Testing

The project includes unit tests for core logic and integration tests using mock servers.

```zsh
cargo test
```

## 🏗️ Project Structure

- `src/main.rs`: Entry point and CLI routing logic.
- `src/menu.rs`: Interactive TTY menu and settings.
- `src/lib.rs`: Core orchestration logic.
- `src/client.rs`: Async networking and measurement tasks.
- `src/theme.rs`: ANSI color system and UI rendering helpers.

## 🤝 Contributing

Feel free to open issues or submit pull requests to improve the tool!

## 📜 License

Distributed under the Apache License 2.0. See `LICENSE` for more information.

---
Built with ❤️ by [Tirso Benedict J. Naza](https://github.com/nazakun021)
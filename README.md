# 🚀 CLI Speedtest (Rust)

A blazing-fast, high-performance CLI speedtest tool written in Rust. Designed for modern developers and system administrators who need accurate, machine-readable network performance metrics without the bloat.

[![Release](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml/badge.svg)](https://github.com/nazakun021/cli-speedtest/actions/workflows/release.yml)

## ✨ Features

- **Blazing Fast**: Powered by `tokio` for asynchronous, non-blocking I/O.
- **High Concurrency**: Multi-threaded download and upload tests to saturate high-speed connections.
- **Machine-Readable**: Use the `--json` flag for clean, parseable output perfect for cron jobs and monitoring.
- **Debugging**: Built-in `tracing` support with the `--debug` flag for troubleshooting.
- **Production Grade**: Time-bound testing architecture ensures consistent result reporting.
- **CI/CD Ready**: Automatic cross-platform releases for Linux, Windows, and macOS via GitHub Actions.

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

### Simple Test
Run a standard speedtest with visual progress bars:
```zsh
cargo run
```

### JSON Output
For automation and scripts, suppress the UI and get a clean JSON string:
```zsh
cargo run -- --json
```

### Advanced Options
```zsh
# Change test duration (default: 10s)
cargo run -- --duration 5

# Enable debug logging (emitted to stderr)
cargo run -- --debug
```

## 🧪 Testing

The project includes pure function unit tests for performance calculations.

```zsh
cargo test
```

## 🏗️ Project Structure

- `src/main.rs`: Core logic, including async tasks for networking and progress management.
- `Cargo.toml`: Project metadata and dependencies (reqwest, tokio, clap, etc.).
- `.github/workflows/release.yml`: Automated build and release pipeline.

## 🤝 Contributing

Feel free to open issues or submit pull requests to improve the tool!

## 📜 License

[Add License Info Here - e.g., MIT]

---
Built with ❤️ by [Tirso Benedict J. Naza](https://github.com/nazakun021)
# CLI Speedtest Strategic Mission

## The "So What": Why This Project Exists

Most speedtest tools assume you have a perfect environment and a high-bandwidth connection just to load the test itself. **CLI Speedtest exists to provide a terminal-first, low-overhead measurement tool that works when everything else fails.**

It is built for:

- **The "Lazy" Power User**: Who wants a result in ten seconds without leaving the terminal or loading a heavy web-browser.
- **The Constrained Environment**: For users whose internet is so slow that loading a speedtest website is impossible.
- **The Developer/DevOps Engineer**: Who needs to verify server-side bandwidth with a tool that won't trigger provider rate-limits or IP flagging.
- **The Measurement Purist**: Who demands "Zero-Skew" accuracy that ignores TCP slow-start and network-level compression.

This tool bridges the gap between "I just need a quick number" and "I need a production-grade audit of my network."

## Core Technical Principles

### 1. Zero-Skew Measurement

Every metric reported by the tool must represent the true state of the network, isolated from technical artifacts:

- **Warm-up Mandate**: All throughput measurements must discard the first 2 seconds of transfer data to bypass TCP slow-start bias.
- **Incompressibility**: Payloads must be generated using high-entropy random data to ensure that network-level compression does not inflate throughput results.
- **Rendering Isolation**: Measurement logic must run on dedicated tasks, decoupled from TTY rendering cycles to prevent UI latency from affecting byte-counting.

### 2. Resilient Performance

The tool must succeed in restrictive environments without compromising the provider's infrastructure:

- **Adaptive Fallback**: The engine detects rate-limiting and retries the affected throughput phase at one connection when its initial concurrency is greater than one. A Provider may still reject that retry.
- **Provider-Friendly Pacing**: Request jitter (50–150ms) must be injected between chunk requests to break machine-like patterns and ensure consistent connectivity.
- **Mandatory Cooldown**: A 5-minute local cooldown is enforced to prevent accidental IP flagging during automated or rapid manual testing.

### 3. Sovereign User Control

The tool must provide parity between human and machine interfaces:

- **Interactive Mode**: A TTY-optimized TUI for exploratory testing and manual configuration.
- **Direct Mode**: A flag-driven, machine-readable interface with valid JSON schema output for integration into monitoring pipelines.

## Measurement Integrity Spec

### Latency Sampling Window

- **Method**: HEAD requests to `/cdn-cgi/trace` (or equivalent).
- **Sample Count**: Default 20 samples; Direct Mode accepts any count of at least one.
- **Pacing**: 50ms interval between samples to avoid ICMP-style rate limiting.
- **Metrics**: Min, Max, Average, Jitter (Average Deviation), and Packet Loss. Timeouts and non-success HTTP responses count as Packet Loss.

### Saturated Throughput Window

- **Calculation**: `Total Bytes / (Actual Elapsed - WARMUP_SECS)`.
- **Download**: Concurrent GET streams (Default: 4 connections) using 50MB chunks.
- **Upload**: Concurrent POST streams (Default: 2 connections) using 2MB random-buffer chunks.
- **Timing**: Synchronized via `tokio::sync::Barrier` to ensure all workers start counting bytes at the exact same moment.

## Subsystem Architecture

### Orchestration Layer (`lib.rs`)

The "brain" of the tool. It validates custom Provider endpoints, manages the test sequence (Ping → Download → Upload), handles configuration aggregation, and performs the final mathematical reductions. It is strictly decoupled from CLI argument parsing to support library-level integration and automated testing.

### Measurement Engine (`client.rs`)

The high-concurrency I/O layer. It manages the lifecycle of `tokio` tasks, handles streaming byte counters using atomic primitives, and implements retry and rate-limit handling. Workers report progress to the display task through message passing so terminal rendering does not sit on the measurement path.

### Resiliency Layer (`cooldown.rs`, `main.rs`)

The defensive perimeter. It manages disk-persisted cooldown state, rotates User-Agents, and enforces global test timeouts (120s) to prevent stalled processes. It ensures the tool behaves as a "good citizen" on the public internet.

### Presentation Layer (`theme.rs`, `menu.rs`)

The user interface. It manages ANSI color thresholds, live rolling-speed calculations, and the TTY-aware summary box. It respects `NO_COLOR` and non-TTY environments by gracefully degrading to silent/plain-text output.

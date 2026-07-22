# CLI Speedtest Technical Stack

This document defines the technical foundations, dependency policies, and development standards for the CLI Speedtest (Rust) project. It serves as the project's technical constitution.

## Core Runtime & Infrastructure

### 1. Async Runtime: `tokio`

- **Role**: The foundational asynchronous runtime.
- **Rationale**: Provides the high-performance task scheduling required to manage multiple concurrent download/upload streams without blocking the main execution thread.
- **Lock-in**: **Hard-Core**. The Measurement Engine is deeply integrated with Tokio's synchronization primitives (`Barrier`, `CancellationToken`).

### 2. Network Stack: `reqwest` & `rustls`

- **Role**: HTTP client and TLS provider.
- **Rationale**: `reqwest` provides high-level streaming APIs for GET and POST requests. We use `rustls` for a pure-Rust, memory-safe TLS implementation that avoids dependencies on system-native SSL libraries.
- **Lock-in**: **Hard-Core**. Essential for the "Zero-Skew" performance mandate.

### 3. CLI & UX Primitives

- **`clap`**: Industrial-grade argument parsing with derive-macro support.
- **`indicatif`**: Managed progress bars and spinners for TTY environments.
- **`dialoguer`**: Interactive TTY prompts for the Human Interface.
- **`owo-colors` / `console`**: Terminal styling and TTY-awareness.
- **Lock-in**: **Peripheral**. These libraries may be swapped or upgraded without affecting the core measurement engine.

### 4. Distribution & Updates

- **`self-replace`**: Platform-independent binary self-replacement engine.
- **`semver`**: Semantic Versioning parser and comparer.
- **Lock-in**: **Peripheral**. Used for local updates and release checks.

## Development Standards

### Standard Operating Environment

- **MSRV**: Minimum Supported Rust Version is **1.85**. Contributions must not use features from newer releases unless a project-wide MSRV bump is agreed upon.
- **Edition**: Rust **2024**.
- **Linting**: Strict enforcement of `clippy`. CI is configured to fail on any warnings (`-D warnings`).
- **Formatting**: Strict adherence to `cargo fmt`.

### Testing Mandates

- **Integration Testing**: All core network behaviors (rate-limiting, fallback, retries) must be verified via integration tests using `mockito`.
- **Property Testing**: Where applicable, measurement math must be unit-tested against edge cases (e.g., zero duration, extreme throughput).
- **CLI Contract Testing**: Direct Mode failures must return a nonzero exit status and, with `--json`, a valid JSON error object on stdout.
- **Test Isolation**: Tests must not read from stdin, call public providers, or share mutable process environment without serialization.
- **Release Target Coverage**: The self-update asset mapping must be unit-tested for every target produced by the release workflow.
- **Dependency Security**: Run `cargo audit` before a release. Keep `Cargo.lock` under version control so audited dependency resolutions are reproducible.
- **Implementation Discipline**: Avoid `unwrap()` in production paths; work in vertical slices and add a failing test before a behavior change.

## Dependency Policy

### Strict Curation

To maintain the project's "Zero-Bloat" promise, adding new dependencies is a strategic decision:

1.  **Value vs. Bloat**: A dependency must provide significant functionality that is not feasible to implement internally.
2.  **Standardization**: We prefer "De Facto" standard crates (e.g., Tokio, Serde, Reqwest) over obscure or experimental libraries.
3.  **Binary Footprint**: New dependencies must be audited for their impact on compile times and final binary size.

### Serialization & Data

- **`serde` / `serde_json`**: The standard for all structured output. Ensures Direct Mode remains stable and machine-parseable.
- **`chrono`**: Used for consistent, timezone-aware timestamping of results.

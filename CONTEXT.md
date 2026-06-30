# CLI Speedtest Context

This context defines the language used for the CLI Speedtest (Rust) project.

## Measurement

**Warm-up**:
A preliminary phase (default: 2 seconds) during download and upload tests that allows TCP connections to reach a steady-state. While this phase consumes provider bandwidth, the bytes transferred are discarded from final metrics to avoid slow-start bias.
_Avoid_: Initialization, setup phase

**Hybrid Mode**:
A strategic design that allows users to choose between maximum measurement integrity (default) and immediate results via Quick Mode.

**Quick Mode**:
A test configuration that bypasses standard **Warm-up** and **Cooldown** periods for up to one **Quick Burst**, providing fast estimates while maintaining a hard safety ceiling.
_Avoid_: Fast test, instant mode

**Quick Burst**:
A limited sequence of **Quick Mode** tests (maximum: 5) allowed before a mandatory **Cooldown** is enforced. This prevents **Quick Mode** from being used to circumvent provider-friendly pacing.

**Jitter**:
The variation in latency between successive ping samples. Measured in milliseconds.

**Packet Loss**:
The percentage of ping requests that failed to receive a response within the timeout window.

## Network & Resiliency

**Cooldown**:
A mandatory local wait period (default: 5 minutes) between successful tests.
_Avoid_: Throttle, block, rate-limit (local)

**Resilience**:
The tool's ability to successfully complete measurements by adapting to restrictive network environments through pacing, jitter, and connection fallback.

**User-Agent Rotation**:
A resiliency technique that alternates the request HTTP headers to emulate standard browser traffic, preventing false-positive blocking by the **Provider**'s automated bot detection systems.
_Avoid_: User-agent spoofing, bot evasion.

**Provider**:
The remote infrastructure (e.g., Cloudflare) used to host the speedtest endpoints.
_Avoid_: Server, target, host

## User Interface

**Interactive Mode**:
A TTY-based menu system for manual test execution and settings management.

**Direct Mode**:
Non-interactive execution using CLI flags, optimized for scripting and JSON output.

**Direct Mode Activation**:
The routing logic that bypasses **Interactive Mode** and runs the speedtest immediately when any configuration-altering CLI flags are explicitly passed by the user.
_Avoid_: CLI bypass, auto-run routing.

## Distribution & Updates

**Self-Update**:
The mechanism by which the tool detects and updates its own executable. Triggered at most once in a 24-hour period on interactive TUI startup via a user confirmation prompt, or run manually in Direct Mode via the `--self-update` CLI option. Auto-updates are completely disabled for standard Direct Mode runs to avoid disrupting automated scripts/CI.
_Avoid_: Auto-upgrade, software update, patch install.

**Checksum Verification**:
The integrity check performed during a **Self-Update** that matches the computed SHA-256 hash of a downloaded executable against a trusted remote manifest.
_Avoid_: Hash validation, binary verification.

## Core Principles

**Zero-Skew Measurement**:
The mandate that all performance metrics must be free from artifacts like TCP slow-start, network compression, or local TTY rendering overhead.

**Sovereign User Control**:
The design requirement that the tool must provide equal utility to human operators (via Interactive Mode) and agents or automated systems (via Direct Mode).

## Agents

**Agent**:
An autonomous software entity (e.g., Gemini CLI) capable of executing tasks, interacting with the codebase, and maintaining state across sessions via **Session Persistence**.
_Avoid_: Bot, automation script

**Session Persistence**:
The mechanism by which an **Agent** records its current task, progress, and context to a file (e.g., `docs/resume.md`) to allow a subsequent session to resume implementation without loss of context.

**SDD (Software Design Document)**:
A high-level blueprint for a feature or architectural change. In this project, SDDs are often embedded in Phase Specifications (e.g., `docs/SPEC.md`) or standalone documents that the **Agent** uses as a source of truth for implementation.

## Architecture

**Orchestration Layer**:
The core logic responsible for test sequencing, configuration management, and result aggregation. Isolated from CLI-specific concerns.

**Measurement Engine**:
The high-concurrency subsystem responsible for raw network I/O, byte counting, and latency sampling.

**Resiliency Layer**:
The set of mechanisms (pacing, fallback, cooldowns) that ensure tool stability and provider-friendly behavior.

## Licensing

**Dual-Licensed**:
The project is distributed under both the MIT and Apache-2.0 licenses, allowing users to choose the terms that best fit their needs. This is the standard licensing model for the Rust ecosystem.

## Technology Stack

**Hard-Core Dependency**:
A foundational library (e.g., Tokio, Reqwest) that is deeply integrated into the Measurement Engine. Replacing these requires a major architectural revision.

**Peripheral Dependency**:
A UX or utility library (e.g., Dialoguer, Console) used in the Presentation Layer. These can be swapped with minimal impact on the core engine.

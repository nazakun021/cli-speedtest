# CLI Speedtest Documentation

This directory contains the maintained project documentation for `cli-speedtest`.

## Use the Right Document

- [MISSION.md](MISSION.md): product purpose, measurement principles, and subsystem boundaries.
- [OPERATIONS.md](OPERATIONS.md): user and automation contract, Provider behavior, local state, updates, and release checks.
- [ROADMAP.md](ROADMAP.md): completed work and future capabilities.
- [TECH-STACK.md](TECH-STACK.md): runtime choices, development standards, and dependency policy.
- [RELEASING.md](RELEASING.md): the verified release procedure for Crates.io and GitHub Releases.
- [../CHANGELOG.md](../CHANGELOG.md): shipped release history and release-specific verification evidence.
- [SPEC.md](SPEC.md): archived Phase 2.6 implementation plan; not the current operational source of truth.

## Decisions

Accepted architectural decisions are in [adr/](adr/). They explain why the project uses provider-friendly defaults, Hybrid Mode and Quick Burst limits, dual licensing, Self-Update, checksum verification, and User-Agent rotation.

## Documentation Rules

- Use [CONTEXT.md](../CONTEXT.md) for the project vocabulary.
- Treat [OPERATIONS.md](OPERATIONS.md), CLI `--help`, and automated tests as the current behavior contract.
- Keep ADRs as decision records. Correct an ADR only when it misstates the implemented mechanism; add a new ADR when a decision changes.
- Update the README when installation, flags, output, or automation behavior changes.

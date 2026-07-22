# Changelog

All notable user-facing changes are documented here.

## 0.1.5 - 2026-07-22

### Fixed

- Restored live Cloudflare latency measurements by using `GET /cdn-cgi/trace`; the endpoint returns `404` for `HEAD` requests.
- Updated custom Provider preflight validation to require the same supported trace method.
- Updated the locked dependency graph with patched `quinn-proto` and `rustls-webpki` releases following Cargo Audit findings.

### Changed

- Track `Cargo.lock` to make audited dependency resolutions reproducible for this application.
- Clarified the trace-endpoint contract in public and operational documentation.

### Verification

- Release binary smoke-tested against the live Cloudflare Provider with one connection and isolated local state.
- GitHub latest-release check and macOS ARM64 release-asset checksum verified.
- `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo audit`, and `cargo publish --dry-run --allow-dirty` completed before publication.
- Cargo Audit reported no vulnerabilities. It retains upstream warnings without published fixes for `anyhow`, `rand`, and transitive `number_prefix`; this code does not use `anyhow::Error::downcast_mut()` or a custom logger with `rand::rng()`.
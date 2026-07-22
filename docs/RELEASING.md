# Releasing CLI Speedtest

This runbook publishes an already reviewed version to Crates.io and triggers the GitHub Releases workflow.

## Preflight

1. Confirm `Cargo.toml`, the changelog, and release tag use the same SemVer version.
2. Update and commit `Cargo.lock` when dependency resolutions change.
3. Run the release gate:

```zsh
cargo build --release
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo audit
cargo publish --dry-run --allow-dirty
```

4. Run a conservative live Provider smoke test with isolated local state. Use one connection and Quick Mode only for the release check:

```zsh
SPEEDTEST_MOCK_DATA_DIR="$(mktemp -d)" \
  target/release/cli-speedtest --quick --duration 3 --connections 1 --ping-count 3 --json
```

5. Verify the interactive launch screen in a TTY and run `target/release/cli-speedtest --self-update`. The latter must report the current version before publication.

## Publish

1. Commit the release candidate.
2. Run `cargo publish` after confirming the target version does not already exist on Crates.io.
3. Create the annotated tag `vX.Y.Z` at that commit and push the branch and tag.
4. GitHub Actions builds Linux AMD64, Windows AMD64, macOS Intel, and macOS Apple Silicon assets, publishes each binary with an adjacent `.sha256` file, and makes the tag the latest GitHub Release.

## Post-Release

1. Check the GitHub Release contains all four binaries and four checksum files.
2. Download the native asset and verify its SHA-256 checksum.
3. Run `cli-speedtest --self-update` from the previous version to test the live update path when a newer release is available.

## Security Notes

`cargo audit` must report no vulnerabilities. Warnings without a published upstream fix require explicit release-owner review and must be recorded in the changelog or release notes.
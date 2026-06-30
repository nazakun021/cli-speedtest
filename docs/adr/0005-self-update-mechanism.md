# Self-Update Mechanism

We decided to implement an auto-update feature (Self-Update) that checks for, downloads, and applies newer versions of the CLI executable from GitHub Releases without blocking speedtest execution.

## Context

To ensure users run the latest version of the CLI with active bug fixes and performance improvements, we need a distribution update channel. However, update checks must not introduce latency prior to or during network tests (to maintain **Zero-Skew Measurement**), nor should they disrupt automated environments or scripts (to respect **Sovereign User Control**).

## Decision

We decided to implement the **Self-Update** mechanism with the following design:

1. **Trigger & Cache**: Checks are run on interactive menu (TUI) startup. The check timestamp is saved to `~/.local/share/speedtest/last_update_check`. A check is only performed if at least 24 hours have elapsed since the last check. No update check runs automatically during Direct Mode runs.
2. **Channel**: Release assets are queried directly from the GitHub Releases API (`https://api.github.com/repos/nazakun021/cli-speedtest/releases/latest`) using `reqwest`.
3. **Asset Mapping & Execution**:
   * Current OS and CPU architecture are matched at compile time to the corresponding raw binary asset on GitHub (`speedtest-macos-arm64`, `speedtest-macos-intel`, `speedtest-linux-amd64`, or `speedtest-windows-amd64.exe`).
   * The binary is downloaded directly and replaced using the `self-replace` crate.
4. **Overrides & Opt-outs**:
   * Users can manually force an update immediately using the `--self-update` CLI option.
   * Auto-update checking is completely bypassed in non-interactive/Direct Mode runs by default, and can be bypassed in Interactive Mode if the `NO_UPDATE` or `CLI_SPEEDTEST_NO_UPDATE` environment variable is set.
   * Permission errors (e.g. read-only system installation paths) are caught gracefully using a robust anyhow error-chain downcast check and logged to `stderr`.
5. **UX**: In Interactive Mode, when an update is detected, the user is prompted for confirmation via `dialoguer` before the update is applied. Manual updates run via `--self-update` display an active progress bar via `indicatif`.

## Consequences

- The tool remains blazing fast by preventing update checks from delaying speed measurements.
- Non-interactive scripts and automated runs are unaffected because updates are disabled in Direct Mode, respecting **Sovereign User Control**.
- Users are kept in control of changes to their local systems by requiring explicit confirmation in TUI mode.
- Deployment and compilation times remain fast since we avoid packaging binaries into `.tar.gz`/`.zip` archives, bypassing the need for heavy decompression dependencies in Cargo.

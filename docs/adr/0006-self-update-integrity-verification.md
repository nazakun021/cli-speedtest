# Self-Update Integrity Verification

We decided to use SHA-256 Checksum Validation to ensure the integrity of the downloaded executable during **Self-Update**.

## Context

The **Self-Update** mechanism downloads pre-built binaries from GitHub Releases. Without validation, network issues could result in corrupted downloads, or a compromised download path could lead to executing tampered binaries. We need a way to verify the downloaded executable before replacement.

## Decision

We decided to implement SHA-256 Checksum Validation:
1. **Release Asset**: The release pipeline generates a `SHA256SUMS` manifest file containing the SHA-256 hash of each build artifact and uploads it to the release.
2. **Download**: During update, the tool first downloads the `SHA256SUMS` file, parses it to extract the expected hash for the current target platform's asset, and then downloads the binary.
3. **Validation**: The tool calculates the SHA-256 hash of the downloaded binary. If the computed hash does not match the expected hash, the update is aborted, and the temporary file is removed.
4. **Crate**: We will use the `sha2` crate (via standard SHA-256 algorithm) to compute the hash.
5. **Threat Model Boundary**: We explicitly accept the risk of release pipeline compromise. The checksum and binary are hosted on the same GitHub Releases trust path. This mechanism is designed to prevent transit corruption and CDN tampering/MITM, not supply-chain attacks via compromised build credentials or repository hijacking.

We rejected full cryptographic signature verification (e.g. Minisign) because our release pipeline is completely automated on GitHub Actions; cryptographic signing in CI/CD would still rely on GitHub secrets and wouldn't improve security against GitHub infra compromise, but would add significant dependency and operational overhead.

## Consequences

- The **Self-Update** mechanism is protected against network corruption and basic file replacement/MITM attacks.
- Release pipeline compromise (e.g., stolen GitHub tokens, CI compromise) is documented as an accepted risk.
- Simple deployment and fast build/compilation times are maintained without requiring heavy cryptographic key management setups.

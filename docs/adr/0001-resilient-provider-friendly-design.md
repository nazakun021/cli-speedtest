# ADR 0001: Resilient and Provider-Friendly Design

## Status

Accepted

## Context

Initial versions of `cli-speedtest` (v0.1.0) prioritized raw performance, utilizing high concurrency (8 download / 4 upload connections) and immediate retries. In real-world multi-user testing, this aggressive traffic pattern frequently triggered HTTP 429 (Rate Limit) and 403 (Forbidden) responses from public providers like Cloudflare, leading to 60-second hangs and tool instability.

## Decision

We decided to pivot from a "Raw Power" model to a "Resilient and Provider-Friendly" model. This involves three key pillars:

1.  **Reduced Default Concurrency**: Lowered defaults to 4 download and 2 upload connections to reduce the infrastructure footprint per user while still saturating ~1Gbps links.
2.  **Adaptive Fallback**: Detect rate-limiting and retry the affected throughput phase with one connection when the initial concurrency is greater than one. The retry can still be rejected by the Provider.
3.  **Local Cooldown Enforcement**: A mandatory 5-minute local wait period between successful tests to prevent accidental or automated "banning" of user IPs.

## Consequences

- **Reliability**: Users are significantly more likely to get a successful, accurate result on the first attempt without manual intervention.
- **Performance**: Users on >1Gbps links may need to use explicit `--connections` flags to fully saturate their bandwidth, as the new defaults are conservative.
- **User Experience**: The tool may "refuse" to run if called too frequently, requiring the `--force-run` flag for intentional rapid testing.

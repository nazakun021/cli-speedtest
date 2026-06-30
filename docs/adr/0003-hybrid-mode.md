# ADR 0003: Hybrid Mode for Speed vs. Integrity

## Status
Accepted

## Context
We want to serve both "lazy" users who want a fast result and "purists" who want maximum accuracy. Currently, the tool enforces a 2s warm-up and a 5-minute cooldown, which can be frustrating for someone just trying to get a quick estimate of a slow connection.

## Decision
We will implement a "Hybrid Mode" strategy:
1. **Default (Integrity)**: Keep the 2s warm-up and 5-minute cooldown to ensure "Zero-Skew" results and provider friendliness.
2. **Quick Mode (`--quick`)**: Allow users to bypass the 2s warm-up (starting measurement immediately) and the 5-minute cooldown.

## Consequences
- **User Choice**: Lazy users get instant results; auditors get accurate data.
- **Accuracy Trade-off**: Results in `--quick` mode may be skewed by TCP slow-start, which we will document as a known limitation of that flag.
- **Provider Risk**: Repeated use of `--quick` mode increases the risk of IP flagging, as it bypasses the mandatory cooldown.

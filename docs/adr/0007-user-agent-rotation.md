# User-Agent Rotation

We decided to keep **User-Agent Rotation** to ensure the tool can successfully communicate with the **Provider**'s infrastructure without triggering automated bot blocks.

## Context

The speedtest relies on Cloudflare's speed test endpoints (`/__down`, `/__up`). Cloudflare implements automated bot-detection and web application firewall (WAF) policies. Requesting these endpoints with standard HTTP client User-Agents (such as `reqwest/0.12` or custom `cli-speedtest/x.y.z` headers) results in immediate HTTP 403 Forbidden responses, rendering the tool non-functional for users.

## Decision

We decided to keep the **User-Agent Rotation** mechanism:
1. A predefined list of common web browser User-Agent strings (Chrome, Safari, Firefox on Windows, Mac, and Linux) is maintained in `main.rs`.
2. When starting a speedtest run, the tool randomly selects one of these browser User-Agent strings to construct the HTTP client.
3. This decision is documented to clarify that while it is an adversarial evasion technique against the **Provider**'s automated filters, it is a hard technical constraint for the tool to function.

Alternative: We rejected using a standard custom client User-Agent because it leads to immediate blocking, preventing the tool from performing its primary function.

## Consequences

- The speedtest continues to function against Cloudflare's endpoints.
- We acknowledge the risk that a future change in Cloudflare's bot detection logic could break this evasion strategy without warning.
- Future developers are aware of why this rotation is present, preventing accidental removal during refactoring.

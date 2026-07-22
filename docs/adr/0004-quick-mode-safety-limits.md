# Quick Mode Safety Limits (Quick Burst)

## Status

Accepted

We decided to limit the number of successive **Quick Mode** tests to prevent users from circumventing our provider-friendly cooldowns.

## Context

**Quick Mode** allows users to bypass the standard 5-minute **Cooldown**. Without a hard limit, a user (or an **Agent**) could spam a **Provider** with high-concurrency requests, violating our core principle of **Resilience** and potentially leading to IP bans.

## Decision

We are implementing a **Quick Burst** limit:

1. A user can run up to 5 **Quick Mode** tests in rapid succession.
2. Upon reaching the 5th test, a mandatory 5-minute **Cooldown** is enforced, identical to the cooldown after a standard test.
3. The burst counter resets only after a successful **Cooldown**.

## Consequences

- Users get the benefit of fast, repeated tests for "spot checking" network conditions.
- The **Provider** is protected from sustained high-load abuse.
- **Agents** must be aware of the burst limit to handle a cooldown rejection in automation scripts.

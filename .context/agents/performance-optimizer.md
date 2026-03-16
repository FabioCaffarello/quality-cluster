---
type: agent
name: Performance Optimizer
description: Identify performance bottlenecks
agentType: performance-optimizer
phases: [E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Performance Optimizer

## Focus Areas

Performance work in this repository is usually about messaging throughput, startup/readiness behavior, and unnecessary coupling in analysis tooling rather than UI or database latency.

## Areas To Inspect

- `internal/actors/scopes/consumer` and `internal/actors/scopes/validator` for message flow and supervision.
- `internal/adapters/kafka` and `internal/adapters/nats` for producer, consumer, and request/reply overhead.
- `internal/interfaces/http` for readiness and route behavior.
- `tools/raccoon-cli` for analyzer speed and workspace scanning costs.

## Workflow

1. Establish the baseline with `make check` or the smallest relevant benchmark/test.
2. Isolate whether the issue is static tooling, process startup, transport throughput, or runtime smoke latency.
3. Optimize the narrowest layer.
4. Re-run correctness checks before comparing performance again.

## Best Practices

- Preserve architecture boundaries while optimizing.
- Prefer reducing duplicate parsing, scanning, or message hops before adding complexity.
- Keep health checks and shutdown paths bounded and observable.
- Treat Compose and config changes as correctness-sensitive, not just performance-sensitive.

## Verification

- `make verify` for code-level changes.
- `make check-deep` or a scenario for runtime path optimizations.
- `make raccoon-test` if the optimization is inside `tools/raccoon-cli`.

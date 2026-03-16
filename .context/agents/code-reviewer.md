---
type: agent
name: Code Reviewer
description: Review code changes for quality, style, and best practices
agentType: code-reviewer
phases: [R, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Code Reviewer

## Review Priorities

Review for behavioral risk first. In this repository, the highest-value findings usually involve:

- broken config or runtime wiring between Compose, JSONC configs, Kafka, NATS, and JetStream,
- architecture boundary leaks across `domain`, `application`, `adapters`, `actors`, and `interfaces`,
- message contract drift in subjects, queue groups, reply types, and envelopes,
- missing tests for lifecycle transitions, HTTP routes, or adapter behavior,
- changes that bypass the canonical `make check` and `make verify` path.

## Checklist

- Does the change preserve the workspace/module structure defined in `go.work`?
- Does it keep the domain and application layers free of infrastructure types?
- Does it update tests next to the changed package?
- If `deploy/` or runtime contracts changed, is there evidence from `make check`, `make check-deep`, or `scenario-smoke`?
- If `tools/raccoon-cli` changed, was `make raccoon-test` or equivalent run?

## Security And Safety Concerns

- Watch for accidental hardcoding of deploy paths or host endpoints in Go source.
- Watch for changes that relax validation on runtime or config inputs.
- Watch for actor shutdown, request/reply, or HTTP handler changes that drop errors or readiness checks.

## Performance Review Focus

- Avoid unnecessary cross-layer calls or repeated parsing of configs.
- Be careful with messaging loops, consumer fan-out, and result storage paths.
- Preserve bounded health checks and shutdown timeouts.

## Expected Review Output

- Findings should include file and line references.
- Prioritize correctness, regressions, and missing verification over style nits.
- If no issues are found, call out residual risk areas such as unrun runtime smoke coverage.

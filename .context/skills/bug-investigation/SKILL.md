---
type: skill
name: Bug Investigation
description: Systematic bug investigation and root cause analysis
skillSlug: bug-investigation
phases: [E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Bug Investigation

Use this skill when a change or failure affects runtime behavior, contract alignment, config projection, or service health.

## Primary workflow

1. Reproduce the issue with the smallest command that proves it.
2. Decide whether the issue is:
   - static structure/topology drift,
   - contract mismatch,
   - runtime cluster failure,
   - `raccoon-cli` tooling behavior.
3. Fix the narrowest layer that owns the bug.
4. Re-run the smallest verification set that proves the fix.

## Preferred commands

- `make check`
- `make verify`
- `make check-deep`
- `make scenario-smoke SCENARIO=<name>`
- `make trace-pack`
- `make results-inspect`
- `make logs SERVICE=<name>`
- `make ps`

## Repository-specific bug patterns

- compose and JSONC config drift
- NATS or Kafka endpoint mismatch across files
- subject, stream, durable, or queue-group mismatch
- domain/application layer contamination by adapter types
- runtime projection regressions in validator or consumer flow
- `raccoon-cli` analyzer regressions or output contract drift

## Evidence standard

Do not describe a failure only in prose. Capture:

- the command that fails,
- the relevant config or source path,
- the analyzer or runtime stage that broke,
- the verification command that proves the fix.

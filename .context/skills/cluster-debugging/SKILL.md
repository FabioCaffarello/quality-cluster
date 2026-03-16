---
type: skill
name: Cluster Debugging
description: Debug multi-service cluster failures across Compose, NATS, Kafka, configctl, server, consumer, validator, and emulator.
skillSlug: cluster-debugging
phases: [E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Cluster Debugging

## When to use

Use this skill when the repository passes local compilation or unit tests but the cluster still fails to boot, route, validate, or expose results correctly.

## Input signals

- `make check-deep` fails
- `make scenario-smoke` fails or hangs
- Compose services flap, restart, or never become ready
- `consumer` or `emulator` cannot bootstrap active bindings
- `validator` produces no results, wrong results, or stale results
- `make verify` is green but runtime behavior is wrong

## Canonical steps

1. Reproduce the failure with the smallest live command that proves it.
2. Classify the failing stage: cluster boot, control-plane bootstrap, ingestion bridge, validation path, persistence, or query surface.
3. Check infrastructure state before code assumptions.
4. Inspect the single service that first violates the expected flow.
5. Inspect downstream evidence in results or traces before changing code.
6. Re-run only the narrowest smoke or deep check that proves the fix.

## Relevant commands

- `make ps`
- `make logs SERVICE=<name>`
- `make check-deep`
- `make scenario-smoke SCENARIO=<name>`
- `make runtime-smoke`
- `make results-inspect`
- `make trace-pack`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- runtime-bindings`

## Common risks

- blaming the wrong service when the real failure is in bootstrap ordering or missing bindings
- changing contracts during debugging instead of proving the current contract path first
- reading only one service log and missing the upstream failure
- treating a green `make check` as evidence that the live cluster is healthy

## Acceptance criteria

- the failing stage is identified with command output, not guesswork
- the fix is proven by the smallest relevant deep or scenario command
- logs, results, or traces explain why the issue is resolved
- no unrelated service boundary or contract was changed as collateral

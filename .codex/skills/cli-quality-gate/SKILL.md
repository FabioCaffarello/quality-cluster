---
name: CLI Quality Gate
description: Use raccoon-cli and Make targets to enforce the repository's quality gate for runtime, contracts, architecture, drift, and smoke-sensitive changes.
phases: [E, V]
---

# CLI Quality Gate

## When to use

Use this skill whenever a change should be validated through the repository's canonical `raccoon-cli` gate instead of ad hoc command selection.

## Input signals

- any non-trivial code change in Go runtime or `tools/raccoon-cli`
- edits to `deploy/compose`, `deploy/configs`, contracts, or runtime bindings
- uncertainty about whether `check`, `verify`, `quality-gate-ci`, or `check-deep` is sufficient
- a need to convert a diff into a repeatable validation command set

## Canonical steps

1. Start with the fast gate before or during coding.
2. Run the standard post-change gate.
3. Escalate to strict or deep profiles based on blast radius.
4. If the change is in `raccoon-cli`, include Rust tests and repo-level gate checks.
5. Use diagnostics only after a gate or smoke failure has identified the failing layer.

## Relevant commands

- `make check`
- `make verify`
- `make quality-gate-ci`
- `make check-deep`
- `make raccoon-test`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- doctor`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- arch-guard`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- drift-detect`

## Common risks

- running only package tests for changes that affect topology or contracts
- escalating to deep smoke without first clearing the fast static gate
- changing `raccoon-cli` output contracts without re-running repository-level checks
- bypassing `make verify` and claiming completion from partial command coverage

## Acceptance criteria

- the selected gate matches the actual blast radius of the change
- all required static analyzers are green
- deep or scenario validation is added when the change crosses runtime boundaries
- CLI changes are covered by Rust tests and repo-level quality-gate execution
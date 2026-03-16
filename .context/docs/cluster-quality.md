---
type: doc
name: cluster-quality
description: Canonical validation workflow for structure, topology, contracts, architecture, drift, and live runtime proof
category: workflow
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Cluster Quality

## Purpose

This repository has a strict validation model because correctness depends on more than compilation. A change is only safe when the repository structure, Compose topology, transport contracts, architecture boundaries, runtime bindings, and live cluster behavior continue to agree.

## Validation Stack

The quality model is layered.

- **static repository checks**
  - `doctor`
  - `topology-doctor`
    - includes frozen Compose contract checks for required services, critical dependencies, profile mapping, broker images, and stable local ports
  - `contract-audit`
  - `runtime-bindings`
  - `arch-guard`
  - `drift-detect`
- **change-planning checks**
  - `tdd`
  - `recommend`
  - `impact-map`
  - `coverage-map`
  - `snapshot`, `snapshot-diff`, `baseline-drift`
- **live proof and diagnostics**
  - `runtime-smoke`
  - `scenario-smoke`
  - `results-inspect`
  - `trace-pack`

## Canonical Commands

### Before changing code

- `make check`
  - fast guard rail; no infrastructure required
- `make tdd`
  - identify affected symbols, tests, gaps, and likely scenarios
- `make recommend`
  - choose the right post-change verification set for the current diff

### After changing code

- `make verify`
  - Go tests plus fast quality-gate
- `make quality-gate-ci`
  - strict profile with warnings promoted to errors

### For runtime-significant changes

- `make up-dataplane`
- `make check-deep`
- `make scenario-smoke SCENARIO=happy-path`

### For Compose and cluster-shape changes

- `raccoon-cli topology-doctor`
  - catches drift in required services, `core`/`runtime`/`dataplane`/`all` profile membership, critical broker images, and operator-facing local ports before containers start
- `make verify`
  - should stay green before any `up-*` command when the change only touches Compose or static wiring

### For diagnostics

- `make trace-pack`
- `make results-inspect`
- `make logs SERVICE=<name>`
- `make ps`

## Quality-Gate Profiles

- `fast`
  - default local profile
  - validates structure, topology, contracts, runtime bindings, and architecture/drift without live infra
- `ci`
  - same static checks, but warnings become failures
- `deep`
  - static checks plus `runtime-smoke`
  - requires a running dataplane stack

The repository workflow assumes `make check` before coding and `make verify` before closing the change. Use `deep` when the change can affect actual cluster behavior.

## When To Escalate Validation

Escalate from `verify` to `check-deep` or `scenario-smoke` when the change affects:

- `deploy/configs/*.jsonc`
- `deploy/compose/docker-compose.yaml`
- NATS subjects, streams, durables, or queue groups
- Kafka topics or dataplane routing
- runtime bootstrap or readiness behavior
- runtime-change consumers, JetStream durables, or event-driven dataplane refresh
- `bootstrap.reconcile_interval` or other dataplane self-healing cadence
- validator runtime cache or result storage/query behavior
- validator incident storage/query behavior

Operational rule for Compose changes:

- if the change only alters static cluster definition, `topology-doctor` must fail first on meaningful drift
- if the change alters startup order, runtime dependencies, or readiness semantics, escalate to `make check-deep` after the static gate passes

Escalate to `make raccoon-test` when the change affects `tools/raccoon-cli`.

## Scenario-Smoke Use

The runtime scenarios are not redundant with static checks.

- `happy-path`
  - lifecycle + dataplane + validation results
- `config-lifecycle`
  - control plane only
- `invalid-payload`
  - validator must produce failure results
  - validation incidents should also reflect the repeated failed pattern through the incident query surface
- `missing-binding`
  - absent scope/binding handling
- `readiness-probe`
  - bootstrap and readiness behavior

Use the smallest scenario that proves the claim you are changing.

Operational expectation:

- `config-lifecycle` should fail in bootstrap if the control-plane baseline is absent; it should not sit in long readiness polling when `nats`, `configctl`, or `server` are not available
- `trace-pack` should still produce deploy-config evidence even when Docker and HTTP endpoints are unavailable
- `trace-pack` should also capture NATS monitor evidence (`nats/healthz.json`, `nats/jsz.json`) so event-driven refresh failures do not depend only on scattered service logs
- environment failures must surface as explicit findings from `raccoon-cli`, not as silent hangs
- smoke executions should carry unique tooling identity for evidence, but the live cluster baseline remains `global/default`; treat scenario parallelism as unsupported unless the runtime itself is hardened for it
- after Block 4, dataplane bootstrap changes must be proven not only at startup but also across aggregate binding refresh; `happy-path`, `invalid-payload`, and `check-deep` are the minimum runtime proof for that class of change
- after Block 5, dataplane refresh changes must prove both sides of the seam: `contract-audit`/`runtime-bindings` for signal integrity and `scenario-smoke`/`check-deep` for live event-driven convergence
- after Block 7, dataplane refresh changes must also preserve the fallback semantics of `bootstrap.reconcile_interval`: the event remains primary, but stale local state must self-heal through aggregate bootstrap without unnecessary reload churn
- after Block 8, `topology-doctor` must treat `bootstrap.reconcile_interval` as a frozen config invariant for `consumer` and `emulator`, and `trace-pack` must summarize both reconcile cadence and refresh-durable lag from `CONFIGCTL_EVENTS`
- after Block 9, `trace-pack` must classify refresh health as `healthy` or `degraded`; operators should not need to infer status manually from raw JetStream counters
- after Block 10, degraded refresh must also expose a canonical `refresh mode` so telemetry loss, cadence mismatch, transient lag, and stalled refresh do not collapse into the same troubleshooting path

## Drift And Architecture Discipline

Two checks are especially important in this repository:

- `arch-guard`
  - catches layer violations, leaked infrastructure types, cross-cmd imports, and boundary erosion
- `drift-detect`
  - checks whether declarations, configs, compose, source, and docs still tell the same story

Together, they prevent the common failure mode where the cluster still builds but no longer matches its own repository definition.

## Troubleshooting Workflow

When the cluster or quality-gate fails:

1. rerun the failing analyzer or scenario directly
2. inspect Compose status with `make ps`
3. inspect service logs with `make logs` or `make logs SERVICE=<name>`
4. inspect validator outputs with `make results-inspect`
5. inspect `/runtime/validator/incidents` when failure patterns need aggregation
6. collect a bundle with `make trace-pack`

For dataplane refresh issues specifically:

1. inspect `consumer` and `emulator` logs for bootstrap refresh and runtime generation transitions
2. rerun `raccoon-cli contract-audit` and `raccoon-cli runtime-bindings` to verify the runtime-change signal and active-binding map still match the repository
3. inspect `nats/jsz.json` from `trace-pack` to verify `CONFIGCTL_EVENTS` and the refresh durables are visible in monitor state
4. inspect whether `bootstrap.reconcile_interval` is configured as expected in `deploy/configs/consumer.jsonc` and `deploy/configs/emulator.jsonc`
5. rerun `scenario-smoke happy-path` after the config lifecycle that should emit `config.ingestion_runtime_changed`
6. use the `Refresh observability` section in `trace-pack/SUMMARY.md` to compare configured reconcile cadence with current durable lag
7. treat `refresh status: degraded` as the primary escalation signal; use the emitted `diagnosis` and `next step` before falling back to manual log trawling
8. use `refresh mode` to choose the first action: `telemetry-unavailable` means infrastructure/monitor first, `cadence-mismatch` means config drift first, and `stalled-refresh` or `redelivery-detected` means dataplane convergence investigation first

The expected debugging style is evidence-first. Do not guess across NATS, Kafka, runtime projection, and validation routing without command output.

## Done Definition

A runtime-sensitive change is done only when:

- the relevant tests pass,
- `make verify` passes,
- the appropriate static analyzers remain green,
- runtime proof exists when the change touches live cluster behavior,
- any `raccoon-cli` changes are validated with Rust tests and repo-level checks.

## Cross-References

- [`architecture-runtime.md`](./architecture-runtime.md)
- [`messaging-contracts.md`](./messaging-contracts.md)
- [`tooling-raccoon-cli.md`](./tooling-raccoon-cli.md)
- [`development-workflow.md`](./development-workflow.md)

---
type: doc
name: tooling-raccoon-cli
description: Canonical reference for raccoon-cli as repository control-plane tooling
category: tooling
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Tooling Raccoon CLI

## Purpose

`raccoon-cli` is the repository's quality control plane. It exists to make repository truth testable: structure, Compose topology, transport contracts, runtime bindings, architecture boundaries, semantic drift, and live behavior can all be asserted through one command surface.

## Boundary With The Go Runtime

The Go services execute the cluster. `raccoon-cli` does not join that runtime path.

- it reads repository files, source, config, and documentation
- it builds structural indices over the Go codebase
- it starts live runtime checks only when explicitly running smoke or diagnostic commands
- it must remain independent of Go runtime imports and side effects

That boundary is essential. The tool is valuable because it evaluates the runtime from outside the runtime.

## Command Families

### Static structure and topology

- `doctor`
- `topology-doctor`

Use these to confirm repository layout, configs, compose, and source wiring still agree.

Operational expectation for `topology-doctor` in this repository:

- it must catch drift in the frozen Compose contract before `make up-dataplane`
- that contract includes required services, critical `depends_on` edges, profile membership, frozen broker image families, and stable local operator ports
- failures should point back to actionable repository invariants, not generic YAML lint noise

### Contract and routing validation

- `contract-audit`
- `runtime-bindings`

Use these when touching subjects, streams, queue groups, payload expectations, or routing continuity.

### Architecture and drift enforcement

- `arch-guard`
- `drift-detect`

Use these when changing package structure, layer boundaries, config, documentation, or compose/runtime alignment.

Operational rule:

- `drift-detect` must derive `raccoon-cli` subcommands from `tools/raccoon-cli/src/main.rs` instead of a stale manual allowlist, so `workflow-drift` stays aligned with the real CLI surface
- `drift-detect` must normalize transport event names like `configctl.event.config.activated` into domain names like `config.activated` before `contract-domain-drift` comparison, so adapter registry wiring is checked against domain truth rather than transport prefixes

### Change planning and semantic analysis

- `tdd`
- `recommend`
- `impact-map`
- `symbol-trace`
- `rename-safety`
- `coverage-map`
- `snapshot`
- `snapshot-diff`
- `baseline-drift`

These commands convert the repository into a change-aware planning surface. They answer what changed, what is affected, what needs proof, and what semantic drift occurred.

### Live runtime proof and diagnostics

- `runtime-smoke`
- `scenario-smoke`
- `results-inspect`
- `trace-pack`

These are the commands that connect static repository truth to live cluster evidence.

Operational rule:

- smoke and trace commands must fail fast when Docker or HTTP dependencies are unavailable
- `trace-pack` should capture both application endpoints and NATS monitor endpoints so refresh failures can be triaged at the transport layer
- `scenario-smoke config-lifecycle` is expected to preflight only the control-plane baseline (`nats`, `configctl`, `server`) before attempting readiness or lifecycle actions
- `scenario-smoke happy-path` must now prove not only end-to-end data flow but also that `consumer` and `emulator` converged on the same loaded aggregate bootstrap generation before the scenario is considered healthy
- named smoke scenarios should be run sequentially unless the CLI gives each scenario its own config key and runtime scope
- the current smoke engine already isolates per-run `config_key`, binding name, and correlation id, but it still uses the canonical runtime scope baseline; do not infer full multi-scope parallel safety from that
- after Block 4, bootstrap-sensitive runtime changes must be validated against aggregate dataplane refresh, not only startup readiness; `runtime-bindings` remains the static guard rail and `scenario-smoke`/`check-deep` remain the live proof
- after Block 5, event-driven dataplane refresh changes must also keep `contract-audit` green, because refresh now depends on the `config.ingestion_runtime_changed` contract as well as on aggregate bootstrap
- after Block 7, runtime proof for dataplane refresh must account for `bootstrap.reconcile_interval`: the CLI still proves the event-driven path, but live convergence can no longer assume the event is the only self-healing mechanism
- diagnostics that hang without producing evidence are considered tooling defects, not acceptable operator behavior
- `topology-doctor` now treats `bootstrap.reconcile_interval` in `consumer.jsonc` and `emulator.jsonc` as part of the frozen dataplane contract
- `trace-pack` now summarizes refresh observability directly in `SUMMARY.md`, including configured reconcile cadence and JetStream lag for `consumer-runtime-refresh-v1` and `emulator-runtime-refresh-v1`
- `trace-pack` now also classifies that observability as `healthy` or `degraded`, and must emit diagnosis-oriented guidance instead of leaving the operator to interpret raw counters alone
- `trace-pack` now also emits `refresh mode` for degraded refresh, so the first troubleshooting step depends on the actual failure shape rather than on a generic lag warning
- `trace-pack` now also summarizes the latest loaded bootstrap generation seen by `consumer` and `emulator`, including `bootstrap_signature`, compact `runtime_refs`, and whether both services are aligned on the same aggregate runtime

## Operational Position In The Workflow

The root `Makefile` turns `raccoon-cli` into the default workflow:

- `make check`
  - fast quality-gate before coding
- `make verify`
  - Go tests plus quality-gate
- `make check-deep`
  - deep quality-gate with runtime smoke
- `make tdd`
  - test planning before or during changes
- `make recommend`
  - tailored verification after a diff
- `make trace-pack`
  - evidence bundle for failures

If a change bypasses these commands, it is probably bypassing the repository's real safety model.

## Internal Structure

The Rust tool is organized around a few stable subsystems:

- `src/analyzers/`
  - doctor, topology, contracts, runtime bindings, arch guard, drift, snapshots, planning helpers
- `src/gate/`
  - quality-gate orchestration and profiles
- `src/smoke/`
  - runtime smoke stages and named scenarios
- `src/codeintel/`
  - AST-style structural indexing of the Go codebase
- `src/lsp/`
  - optional semantic enrichment via `gopls`
- `src/output/`
  - human and JSON rendering
- `src/results_inspect/`
  - validator results inspection
- `src/trace_pack/`
  - diagnostic bundle collection

The command surface in [`tools/raccoon-cli/src/main.rs`](/Volumes/OWC%20Express%201M2/Develop/quality-service/tools/raccoon-cli/src/main.rs) is intentionally broad because the repository treats quality as part of implementation, not as a separate afterthought.

## Maintenance Rules

When changing `raccoon-cli`:

- preserve command intent and output contracts
- update docs if command behavior changes materially
- keep false certainty out of analyzers; explicit limits are better than silent under-detection
- map analyzer changes back to concrete repository invariants
- verify both the Rust test suite and the repo-level workflow commands
- keep Compose parsing aligned with the real cluster file; if `deploy/compose/docker-compose.yaml` changes materially, update analyzer fixtures and checks in the same change

Minimum verification:

- `make raccoon-test`
- `make check`
- `make quality-gate-ci`

Use deeper runtime proof when the tooling change affects smoke, runtime diagnostics, or drift detection assumptions.

When the repository changes event-driven refresh behavior:

- `contract-audit` proves subject, stream, durable, and event-name continuity
- `runtime-bindings` proves the aggregate bootstrap still describes the effective dataplane set
- `runtime-bindings` now also guards the bootstrap runtime summary contract, so `/runtime/ingestion/bindings` keeps both `bindings` and compact `runtimes` aligned with the canonical `/runtime/configctl/projections` surface
- `trace-pack` should expose NATS monitor and JetStream state for `CONFIGCTL_EVENTS` when refresh diagnostics are needed
- `scenario-smoke` and `check-deep` prove that the runtime really converges after the event
- docs, configs, and smoke expectations must stay aligned on `bootstrap.reconcile_interval` so operators understand that event-driven refresh is primary and bounded reconciliation is fallback
- when refresh diagnostics are needed, `trace-pack` should be the first evidence bundle because it now carries both the config cadence and current durable counters in one place
- after Block 9, `trace-pack` is also expected to tell the operator whether refresh is healthy, why it is degraded, and which next check to run first
- after Block 10, `trace-pack` is also expected to say which degraded mode it detected, using only the existing monitor payload and frozen config cadence

## Repository Truths This Tool Must Preserve

- the cluster really is Compose + NATS + Kafka + five Go services
- the runtime really is layered and boundary-sensitive
- config, compose, source, contracts, and docs can drift independently
- the repository needs both static proof and live proof
- `raccoon-cli` is part of the product's engineering operating model, not a side utility

## Cross-References

- [`project-overview.md`](./project-overview.md)
- [`architecture-runtime.md`](./architecture-runtime.md)
- [`cluster-quality.md`](./cluster-quality.md)
- [`messaging-contracts.md`](./messaging-contracts.md)

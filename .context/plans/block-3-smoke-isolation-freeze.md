---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-3-smoke-isolation-hardening"
phase: "phase-1"
---

# Block 3 Smoke Isolation Freeze

## Objective

Freeze the real smoke identity problem before implementation so the next block hardens the quality engine itself instead of reopening stable runtime architecture.

## Fixed Identifiers Found In The Smoke Engine

### Shared Config Identity

- `tools/raccoon-cli/src/smoke/stages.rs`
  - `client.create_draft("raccoon-smoke", ...)`
- `tools/raccoon-cli/src/smoke/scenarios.rs`
  - `config-lifecycle` creates draft `"scenario-lifecycle"`

### Shared Binding Identity

- `tools/raccoon-cli/src/smoke/stages.rs`
  - binding name: `smoke_events`
  - topic: `smoke.events.created`

### Shared Scope Assumptions

- `tools/raccoon-cli/src/smoke/api.rs`
  - active config lookup always queries `scope_kind=global` and `scope_key=default`
  - ingestion bindings lookup always queries `scope_kind=global` and `scope_key=default`
  - validation results default to `global/default`

### Shared Evidence Identity

- `tools/raccoon-cli/src/smoke/api.rs`
  - correlation id is only `raccoon-smoke-<pid>`
- this distinguishes process, but not scenario kind or multiple executions within the same process

## Observed Operational Failure

Block 2 runtime proof exposed a real tooling limitation:

- `happy-path` and `invalid-payload` both depend on the same smoke config key
- when they run concurrently, one can fail with `409` during `POST /configctl/configs`
- this is not a cluster regression; it is a shared-identity defect in the smoke engine

## Freeze Decision

Block 3 will harden smoke identity in the tooling before any attempt to parallelize smoke runs.

Implementation rule:

- isolate execution identity first
- preserve the cluster contracts and the canonical `global/default` baseline unless isolation strictly requires additional query metadata
- prefer unique naming over destructive cleanup

## In Scope

- unique run identity in `SmokeConfig`
- derived config keys, binding names, and correlation ids
- filters or metadata needed so scenario evidence does not mix across runs
- smoke test coverage for the new identity model
- doc updates describing the final operating rule

## Out Of Scope

- changing ownership in the Go runtime
- introducing new cleanup endpoints in `configctl`
- redesigning `results-inspect` output format unless required by identity tracking
- making every scenario parallel-safe in the same step if sequential determinism is not yet proven

## Immediate Execution Order

1. add a run identity model to the smoke config
2. thread that identity through inject, route, validate, and scenario-specific flows
3. prove results and bindings are attributed to the correct execution
4. rerun the canonical smoke ladder and deep gate

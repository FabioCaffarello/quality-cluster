---
status: filled
generated: 2026-03-16
updated: 2026-03-16
phase: "phase-3"
plan: "block-2-operational-signal-hardening"
---

# Block 2 Phase 3 Validation

## Scope

Final runtime proof for Block 2 after:

- actor lifecycle signal cleanup
- static Compose contract checks added to `raccoon-cli topology-doctor`

## Static And Guard-Rail Proof

Passed:

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml topology`
- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml compose_runtime_contract`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- `make verify`
- `make check-deep`

The deep gate now proves the new static Compose contract and the existing runtime smoke together.

## Runtime Proof

Passed with a fresh stack:

- `make down`
- `make up-dataplane`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`

## Important Operational Finding

The smoke scenarios are **not** safe to run in parallel today.

Reason:

- `happy-path` and `invalid-payload` both create drafts under the same config key, `raccoon-smoke`
- when they are launched concurrently, one of them can fail with `409` during `POST /configctl/configs`
- this is a smoke orchestration constraint, not a regression in the runtime hardening work

Operational rule preserved for the repository:

- run named `scenario-smoke` flows sequentially unless they are made key-isolated in the CLI

## Evidence Notes

- `results-inspect` showed the expected failed validation results for invalid payload samples under the `smoke_events` binding
- `trace-pack` collected a full live bundle at `trace-pack-20260316-142154` during the phase and confirmed Compose, readiness, bindings, runtime and results were all collectible

## Phase 3 Outcome

- `B2-P3-S1`: complete
- `B2-P3-S2`: complete
- `B2-P3-S3`: complete
- `B2-P3-S4`: complete

## Final Outcome

Block 2 now leaves the repository in a stronger operational position:

- actor bootstrap logs are quieter and easier to trust
- Compose drift in critical cluster invariants now fails in the quality engine before live runtime proof
- the `raccoon-cli` remains the primary quality motor for both static and live validation of the cluster

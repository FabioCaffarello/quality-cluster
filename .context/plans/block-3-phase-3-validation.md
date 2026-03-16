---
status: filled
generated: 2026-03-16
updated: 2026-03-16
phase: "phase-3"
plan: "block-3-smoke-isolation-hardening"
---

# Block 3 Phase 3 Validation

## Scope

Final validation for the smoke identity hardening work in `tools/raccoon-cli/src/smoke`.

The implemented scope was intentionally narrower than full multi-scope runtime isolation:

- unique execution identity now covers `config_key`, binding name and correlation id
- the runtime baseline stays on `global/default`
- named smoke scenarios still run sequentially by policy

## Why The Scope Stayed Narrow

The runtime proof showed that the cluster does not yet behave as a fully isolated multi-scope dataplane for smoke purposes:

- `validator` accepted tenant-scoped runtime updates
- `configctl` exposed tenant-scoped bindings correctly
- but `consumer` and `emulator` remained effectively anchored to the canonical active dataplane baseline

Because this block is about hardening the quality engine, not redesigning the Go runtime, the final implementation preserved `global/default` and isolated only the tooling-owned identifiers.

## Implemented Outcome

- `SmokeConfig` now generates a per-run identity
- `runtime-smoke` and `scenario-smoke` use unique config keys instead of reusing `raccoon-smoke`
- smoke bindings are uniquely named per execution
- `X-Correlation-ID` now carries the run identity
- the smoke engine still queries and activates against the canonical scope unless explicitly changed in future work

## Static Validation

Passed:

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml smoke`
- `make verify`

## Runtime Validation

Passed:

- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

## Operational Outcome

- the false `409` conflict caused by reusing the same smoke config key is removed for sequential executions
- evidence is easier to attribute to a single run because config key, binding name and correlation id now move together
- the repository keeps a clear operational rule:
  - run named smoke scenarios sequentially
  - treat deeper scope isolation as future runtime work, not as an accidental side effect of the CLI

## Phase 3 Outcome

- `B3-P3-S1`: complete
- `B3-P3-S2`: complete
- `B3-P3-S3`: complete

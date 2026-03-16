---
status: filled
generated: 2026-03-16
updated: 2026-03-16
phase: "phase-3"
plan: "block-1-lifecycle-hardening"
---

# Block 1 Phase 3 Validation

## Scope

Validation and handoff evidence for the lifecycle hardening work across `server`, `configctl`, `validator`, and `tools/raccoon-cli`.

## Static Guard Rails

The repository-level static and architectural proof passed after the phase-2 changes:

- `make verify`
- `make check`
- `make check-deep`
- `raccoon-cli topology-doctor`
- `raccoon-cli drift-detect`
- `raccoon-cli arch-guard`
- `raccoon-cli contract-audit`

## Raccoon CLI Hardening Added In Phase 3

The quality engine itself was hardened so live validation fails fast and diagnostically:

- `scenario-smoke config-lifecycle` now performs a control-plane bootstrap preflight for `nats`, `configctl`, and `server` before entering readiness polling
- `smoke::compose` uses bounded `docker compose ps` execution instead of potentially hanging indefinitely
- `trace-pack` uses the same bounded Docker command strategy for compose status and service logs
- compose file invocation was corrected to use a canonical path, avoiding duplicated `deploy/compose/...` resolution errors

## Runtime Proof Status

The runtime proof is now complete with the cluster running under Compose and the `raccoon-cli` acting as the primary validation engine.

Passed:

- `make scenario-smoke SCENARIO=config-lifecycle`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make check-deep`

Key phase-3 findings that were fixed:

- the dataplane was blocked by an unavailable image reference in Compose; `kafka` now uses `bitnamilegacy/kafka:3.9.0`, preserving the existing 3.9 line and Bitnami-specific runtime layout
- deep quality-gate exposed a real lifecycle race: a late `config.deactivated` event could clear a newer validator runtime, causing `server /readyz` to fall back to `503 {"status":"not_ready"}` while active bindings still existed
- the validator runtime cache now ignores stale deactivation events when they do not match the currently loaded `config_set_id` and `version_id` for the scope

## Diagnostic Evidence

Trace pack collected at:

- `.context/plans/artifacts/block-1-phase-3/trace-pack-20260316-050225`
- `.context/plans/artifacts/block-1-phase-3/trace-pack-20260316-051232`

Summary:

- `trace-pack-20260316-050225` captured the earlier Docker-daemon outage and proved the new fail-fast diagnostics in the CLI
- `trace-pack-20260316-051232` captured a live dataplane with compose status, service logs, active config, bindings, runtime and validation results
- the second trace pack made the runtime race visible: active config and bindings existed while validator runtime had been cleared

## Phase 3 Outcome

- `B1-P3-S1`: complete
- `B1-P3-S2`: complete
- `B1-P3-S3`: complete for the blocked environment via trace-pack evidence
- `B1-P3-S4`: complete

## Final Validation Summary

- lifecycle surface remains canonical and the scenario smoke matches the real HTTP contract
- `server` readiness now behaves honestly and stays green through the deep gate
- the validator runtime lifecycle survives back-to-back config replacement without dropping into stale runtime state
- the raccoon CLI is now proving the cluster for this block instead of masking runtime races or hanging on Docker calls

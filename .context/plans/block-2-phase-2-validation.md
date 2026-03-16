---
status: filled
generated: 2026-03-16
updated: 2026-03-16
phase: "phase-2"
plan: "block-2-operational-signal-hardening"
---

# Block 2 Phase 2 Validation

## Scope

Evidence for the two implementation fronts of Block 2:

- actor lifecycle signal cleanup in `configctl`, `consumer`, and `validator`
- static Compose contract protection inside `raccoon-cli topology-doctor`

## Signal Cleanup Outcome

The actor layer now ignores expected Hollywood lifecycle messages such as `actor.Initialized` and `actor.Started` before falling back to `Warn("unknown message")`.

Applied through the shared helper in:

- `internal/actors/common/lifecycle.go`

Validated with:

- `go test ./internal/actors/common ./internal/actors/scopes/configctl ./internal/actors/scopes/consumer ./internal/actors/scopes/validator`
- `make verify`
- `make scenario-smoke SCENARIO=readiness-probe`

Observed runtime effect:

- `docker compose logs --tail=120 configctl consumer validator | rg "level=WARN|unknown message"` returned no matches during normal bootstrap after the cleanup

## Compose Guard Rail Outcome

`raccoon-cli topology-doctor` now encodes the frozen Compose invariants from the Block 2 scope freeze:

- required services remain present
- `configctl` and `server` stay in `core`
- `validator` stays in `runtime`
- `kafka`, `consumer`, and `emulator` stay in `dataplane`
- `all` membership remains attached to profiled services
- broker images remain frozen at `nats:2.10.18-alpine` and `bitnamilegacy/kafka:3.9.0`
- stable local ports remain exposed for NATS (`4222`, `8222`), `server` (`8080`) and Kafka (`19092`)

Implementation points:

- `tools/raccoon-cli/src/analyzers/topology/compose.rs`
- `tools/raccoon-cli/src/analyzers/topology.rs`

Validated with:

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml topology`
- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml compose_runtime_contract`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- `make verify`
- `make check-deep`

## Phase 2 Outcome

- `B2-P2-S1`: complete
- `B2-P2-S2`: complete
- `B2-P2-S3`: complete
- `B2-P2-S4`: complete
- `B2-P2-S5`: complete

## Current Position

The repository now protects the local cluster in two layers:

- bootstrap logs are quieter and more actionable because framework lifecycle chatter no longer looks like a runtime anomaly
- Compose drift in critical images, ports, and profiles now fails in the static quality engine before runtime proof starts

This keeps `raccoon-cli` in its intended role: quality motor for the cluster, not just a wrapper around smoke scenarios.

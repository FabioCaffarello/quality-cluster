---
type: doc
name: architecture-runtime
description: Canonical view of runtime topology, layer boundaries, and service interaction paths
category: architecture
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Architecture Runtime

## Runtime Boundary

`quality-service` has two cooperating systems:

- the **Go runtime cluster**, which executes config lifecycle, runtime projection, dataplane ingestion, and validation;
- the **Rust tooling layer**, which validates whether the repository still describes and operates that cluster correctly.

This document covers the Go runtime side and the boundary it keeps against the tooling side.

## Cluster Topology

The local cluster is defined in [`deploy/compose/docker-compose.yaml`](/Volumes/OWC%20Express%201M2/Develop/quality-service/deploy/compose/docker-compose.yaml).

- `nats`
  - control plane, event transport, runtime cache feeds, and results query transport
- `kafka`
  - dataplane ingestion broker
- `configctl`
  - owns config draft, validation, compile, activate, deactivate, and domain event emission
- `server`
  - exposes HTTP APIs and proxies requests to NATS request/reply gateways
- `validator`
  - caches active runtime, consumes dataplane messages, evaluates rules, and serves results
- `consumer`
  - bootstraps from active ingestion bindings and republishes canonical dataplane messages to JetStream
- `emulator`
  - bootstraps from the same runtime view and emits valid and invalid Kafka messages for each active binding

Compose profiles:

- `core`: `nats`, `configctl`, `server`
- `runtime`: `core` + `validator`
- `dataplane`: `runtime` + `kafka`, `consumer`, `emulator`

## Layered Code Architecture

The Go repository is organized around explicit runtime boundaries:

- `internal/domain`
  - config lifecycle entities, runtime projections, rules, and domain events
- `internal/application`
  - contracts, ports, use cases, bootstrap clients, dataplane routing, validation logic
- `internal/adapters`
  - NATS/JetStream, Kafka, and repository implementations
- `internal/actors`
  - process and message orchestration via Hollywood actors
- `internal/interfaces/http`
  - HTTP request/response layer
- `internal/shared`
  - settings, bootstrap, envelope, problems, request context, events

These are not naming conventions only. `raccoon-cli arch-guard` treats them as enforcement boundaries.

## Service Execution Paths

### Config lifecycle path

1. `server` receives HTTP requests on `/configctl/*`.
2. The server uses NATS request/reply gateways to reach `configctl`.
3. `configctl` routes control messages into actor-driven use cases and in-memory state.
4. Lifecycle transitions emit domain events over the `CONFIGCTL_EVENTS` stream.
5. Activation produces runtime projection records that downstream services depend on.

Relevant code paths:

- [`cmd/server/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/run.go)
- [`cmd/server/gateway.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/gateway.go)
- [`internal/actors/scopes/configctl/control_router.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/configctl/control_router.go)
- [`internal/actors/scopes/configctl/control_responder.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/configctl/control_responder.go)
- [`internal/actors/scopes/configctl/event_router.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/configctl/event_router.go)

### Runtime query path

1. `server` exposes `/runtime/configctl/projections`, `/runtime/validator/active`, `/runtime/ingestion/bindings`, and `/runtime/validator/results`.
2. Runtime queries traverse NATS gateways into configctl or validator responders according to ownership.
3. `configctl` serves the active runtime truth and the active ingestion binding set.
4. The validator serves runtime state from its runtime cache and validation results from its results store.

This means the HTTP layer is intentionally thin. It is a transport facade over NATS-backed application contracts, not the owner of runtime state. The separation is deliberate:

- `/runtime/configctl/projections`
  - configctl truth about active runtime projections
- `/runtime/ingestion/bindings`
  - operational bootstrap view derived from that truth
- `/runtime/validator/active`
  - validator loaded-state only, not source of truth
- `/runtime/validator/results`
  - validation output, not runtime ownership

### Dataplane validation path

1. Activation in `configctl` projects active ingestion bindings and runtime metadata.
2. `consumer` boots from the aggregate `/runtime/ingestion/bindings` view, builds a runtime topology across the active binding set, consumes Kafka topics, and republishes canonical dataplane messages into JetStream.
3. `consumer` refreshes that topology when `configctl` emits `config.ingestion_runtime_changed`; the event is the primary trigger, while aggregate bootstrap remains the state source.
4. `consumer` also runs bounded self-healing reconciliation through `bootstrap.reconcile_interval`, so a missed local event does not leave the dataplane stale indefinitely.
5. `validator` consumes both config activation events and dataplane ingestion events.
6. The validator resolves the active runtime for the message scope, evaluates rules, and stores the result.
7. `emulator` uses the same aggregate bootstrap seam, refreshes on the same runtime-change signal, and continuously produces one valid and one invalid synthetic JSON payload per active binding, which closes the loop for smoke validation.
8. `emulator` also reconciles by `bootstrap.reconcile_interval` before continuing synthetic publication; this keeps smoke useful even if the refresh event is delayed or lost locally.

Relevant code paths:

- [`internal/application/runtimebootstrap/client.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/runtimebootstrap/client.go)
- [`internal/application/dataplane/registry.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/registry.go)
- [`internal/application/dataplane/contracts.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/contracts.go)
- [`internal/actors/scopes/consumer/runtime.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)
- [`internal/actors/scopes/validator/runtime_consumer.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_consumer.go)
- [`internal/actors/scopes/validator/dataplane_consumer.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/dataplane_consumer.go)
- [`internal/actors/scopes/validator/validation_router.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_router.go)

## Operational Boundaries

- `cmd/*` binaries should remain thin startup adapters.
- `server` should not own domain rules; it should proxy through gateways and use cases.
- `configctl` owns lifecycle and event emission.
- `validator` owns runtime cache resolution and result evaluation.
- `consumer` and `emulator` are runtime clients of active-ingestion bootstrap, not independent sources of truth.
- `consumer` and `emulator` now default to aggregate bootstrap plus signature-based refresh.
- `consumer` and `emulator` also use bounded reconciliation via `bootstrap.reconcile_interval`; that fallback exists for self-healing, not as a replacement for the event-driven path.
- `config.ingestion_runtime_changed` is the canonical dataplane refresh trigger; it does not replace aggregate bootstrap as the source of truth.
- the scope-specific bootstrap path remains an internal troubleshooting seam, not the primary dataplane mode.
- `tools/raccoon-cli` must stay outside this runtime path and inspect it from the repository boundary.

## Runtime Invariants

- `server` readiness depends on successful configctl reachability through NATS.
- `consumer` and `emulator` depend on the aggregate active-ingestion view exposed by `/runtime/ingestion/bindings`.
- dataplane refresh is event-driven by `config.ingestion_runtime_changed` and gated by binding-set signature: no reload when the aggregate bootstrap is unchanged; runtime replacement when the active set changes.
- when the local event path is missed or delayed, dataplane convergence falls back to aggregate bootstrap reconciliation on `bootstrap.reconcile_interval`.
- `validator` depends on both activation events and dataplane ingestion events.
- scope defaults are `global/default` unless explicitly overridden.
- changes to `deploy/configs`, compose dependencies, subjects, streams, or runtime bootstrap must be treated as runtime-significant.

## Cross-References

- [`project-overview.md`](./project-overview.md)
- [`messaging-contracts.md`](./messaging-contracts.md)
- [`cluster-quality.md`](./cluster-quality.md)
- [`tooling-raccoon-cli.md`](./tooling-raccoon-cli.md)

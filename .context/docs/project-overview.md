---
type: doc
name: project-overview
description: Canonical high-level view of the repository, its runtime system, and its validation model
category: overview
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Project Overview

`quality-service` is a local quality-validation cluster implemented as a Go workspace and governed by a Rust quality CLI. The repository combines config lifecycle management, runtime projection, dataplane validation, and repository-aware analyzers into a single operational system: the Go services execute the cluster, and `raccoon-cli` proves that the cluster definition, contracts, architecture, and runtime behavior still match.

The project is useful to engineers who need to evolve runtime behavior without losing control of topology, messaging contracts, or architectural boundaries. It is not just a set of binaries and tests; it is a controlled environment for validating configuration-driven ingestion and validation flows end to end.

## Codebase Reference

> **Generated inventory**: [`codebase-map.json`](./codebase-map.json) contains the MCP-generated file inventory. In this repository it is useful as a rough file map, but it under-detects Docker, architecture layers, and the role of `raccoon-cli`. Treat the docs in this folder as the operational source of truth.

## Quick Facts

- Root: `/Volumes/OWC Express 1M2/Develop/quality-service`
- Primary runtimes: Go workspace services plus a Rust CLI in `tools/raccoon-cli`
- Main cluster services: `configctl`, `server`, `validator`, `consumer`, `emulator`, with `nats` and `kafka` as infrastructure
- Primary validation entrypoints: `make check`, `make verify`, `make check-deep`, `make scenario-smoke`, `make raccoon-test`
- Full generated inventory: [`codebase-map.json`](./codebase-map.json)

## Entry Points

- [`cmd/configctl/main.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/configctl/main.go) and [`cmd/configctl/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/configctl/run.go)
  - control-plane owner for config lifecycle and event publication
- [`cmd/server/main.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/main.go) and [`cmd/server/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/run.go)
  - HTTP facade over config, runtime, and validation-result queries through NATS gateways
- [`cmd/validator/main.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/validator/main.go) and [`cmd/validator/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/validator/run.go)
  - runtime cache, dataplane consumer, validation router, and results responders
- [`cmd/consumer/main.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/consumer/main.go) and [`cmd/consumer/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/consumer/run.go)
  - bootstrap from active ingestion bindings and republish canonical dataplane messages into JetStream
- [`cmd/emulator/main.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/emulator/main.go) and [`cmd/emulator/run.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/emulator/run.go)
  - synthesize valid and invalid Kafka payloads per active binding
- [`tools/raccoon-cli/src/main.rs`](/Volumes/OWC%20Express%201M2/Develop/quality-service/tools/raccoon-cli/src/main.rs)
  - command surface for static analysis, runtime proof, diagnostics, and change planning

## Key Exports

- HTTP routes exposed by `server`:
  - `/healthz`
  - `/readyz`
  - `/configctl/*`
  - `/runtime/configctl/projections`
  - `/runtime/validator/active`
  - `/runtime/ingestion/bindings`
  - `/runtime/validator/results`
  - `/runtime/validator/incidents`
- NATS control surfaces:
  - `configctl.control.*`
  - `validator.runtime.get_active`
  - `validator.results.list`
  - `validator.incidents.list`
- Event and dataplane surfaces:
  - `CONFIGCTL_EVENTS`
  - `DATA_PLANE_INGESTION`
  - `dataplane.ingestion.received.>`
- Tooling command surface:
  - `quality-gate`
  - `topology-doctor`
  - `contract-audit`
  - `runtime-bindings`
  - `arch-guard`
  - `drift-detect`
  - `runtime-smoke`
  - `scenario-smoke`
  - `trace-pack`

## File Structure & Code Organization

- `cmd/`
  - executable startup wrappers; each binary loads config, validates, and hands off to runtime wiring
- `internal/domain/`
  - config lifecycle entities, projections, and domain events
- `internal/application/`
  - use cases, contracts, ports, dataplane registry, runtime bootstrap, and validation logic
- `internal/adapters/`
  - Kafka, NATS/JetStream, and repository implementations
- `internal/actors/`
  - Hollywood actor supervisors, routers, consumers, and responders
- `internal/interfaces/http/`
  - HTTP handlers, route registration, and webserver
- `internal/shared/`
  - settings, bootstrap, problems, envelopes, events, request context, and supporting utilities
- `deploy/`
  - Compose profiles, JSONC runtime config, NATS config, and Dockerfile
- `tests/http/`
  - runnable HTTP smoke sequences for control-plane and runtime checks
- `tools/raccoon-cli/`
  - repository analysis, quality-gate orchestration, smoke diagnostics, codeintel, and LSP enrichment

## Technology Stack Summary

The runtime stack is Go plus Docker Compose, NATS, JetStream, Kafka, and HTTP. The repository stack is completed by a Rust CLI that inspects the Go source tree, config files, compose topology, and runtime expectations without importing the Go runtime. This split is deliberate: the runtime system executes the cluster, while `raccoon-cli` enforces structural and operational truth about that system.

## Getting Started Checklist

1. Install Go, Docker, and Cargo.
2. Read [`architecture-runtime.md`](./architecture-runtime.md) and [`tooling-raccoon-cli.md`](./tooling-raccoon-cli.md) before changing runtime-sensitive areas.
3. Run `make check` from the repository root to establish a known-good baseline.
4. Start the cluster you need with `make up-core`, `make up-runtime`, or `make up-dataplane`.
5. Use `make verify` for normal completion, and `make check-deep` or `make scenario-smoke` when the change affects live runtime flow.

## Next Steps

Use the rest of the canonical docs in this folder as the reusable operational context:

- [`architecture-runtime.md`](./architecture-runtime.md)
- [`cluster-quality.md`](./cluster-quality.md)
- [`messaging-contracts.md`](./messaging-contracts.md)
- [`tooling-raccoon-cli.md`](./tooling-raccoon-cli.md)

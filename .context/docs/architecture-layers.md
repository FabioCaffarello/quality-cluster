---
type: doc
name: architecture-layers
description: Repository layering model and dependency rules across Go services and tooling
category: architecture
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Architecture Layers

## Purpose

This repository uses a layered Go architecture that is enforced both by code layout and by `raccoon-cli arch-guard`.

## Layer Map

- `internal/domain`
  - business entities, lifecycle rules, and domain events
- `internal/application`
  - use cases, contracts, ports, runtime/bootstrap orchestration
- `internal/adapters`
  - NATS, Kafka, and repository implementations
- `internal/actors`
  - process supervision and actor-based orchestration
- `internal/interfaces/http`
  - HTTP handlers, routes, and webserver wiring
- `internal/shared`
  - cross-cutting support code such as bootstrap, settings, envelope, problem, and request context

Additional repository boundaries:

- `cmd/*`
  - thin executable entrypoints for each binary
- `deploy/*`
  - compose, Docker, NATS, and JSONC runtime config
- `tools/raccoon-cli/*`
  - Rust quality platform for repository analysis

## Allowed Dependency Direction

`arch-guard` documents the intended inward dependency direction:

- `domain` can depend on `shared`
- `application` can depend on `domain` and `shared`
- `adapters` can depend on `application`, `domain`, and `shared`
- `actors` can depend on `adapters`, `application`, `domain`, and `shared`
- `interfaces` can depend on `application`, `domain`, and `shared`

Forbidden or discouraged patterns include:

- `domain` importing infrastructure packages
- `application` importing adapters, actors, or HTTP interfaces directly
- `interfaces` importing adapters or actors directly
- one `cmd` binary importing another

## Why The Layers Matter Here

This repository mixes:

- config lifecycle logic,
- runtime projection,
- messaging contracts,
- actor orchestration,
- HTTP access,
- Rust-based structural analysis.

Without explicit boundaries, runtime wiring would leak into the domain and application layers quickly. The repository relies on `arch-guard` to prevent that drift.

## Architectural Hotspots

### Config lifecycle

- `internal/domain/configctl`
- `internal/application/configctl`
- `internal/application/configctlclient`

### Runtime and dataplane

- `internal/application/dataplane`
- `internal/application/runtimebootstrap`
- `internal/application/runtimecontracts`
- `internal/application/validatorruntime*`
- `internal/application/validatorresults*`

### Infrastructure adapters

- `internal/adapters/nats`
- `internal/adapters/kafka`
- `internal/adapters/repositories`

### Orchestration

- `internal/actors/scopes/configctl`
- `internal/actors/scopes/consumer`
- `internal/actors/scopes/server`
- `internal/actors/scopes/validator`

### HTTP surface

- `internal/interfaces/http/handlers`
- `internal/interfaces/http/routes`
- `internal/interfaces/http/webserver`

## Review Rules

When changing code in this repository, ask:

- Did infrastructure types leak into `domain` or `application/ports`?
- Did an adapter concern move into a use case or entity?
- Did HTTP routing logic bypass the application layer?
- Did runtime config or deploy paths become hardcoded into internal packages?

If the answer might be yes, run:

- `make arch-guard`
- `make drift-detect`
- `make verify`

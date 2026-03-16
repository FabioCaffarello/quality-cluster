---
type: doc
name: runtime-topology
description: Cluster topology, service profiles, and end-to-end runtime data flow
category: architecture
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Runtime Topology

## Purpose

This document explains the operational topology of `quality-service`: which services run, how they depend on each other, and how data moves through the cluster.

## Runtime Profiles

The cluster is defined in [`deploy/compose/docker-compose.yaml`](/Volumes/OWC%20Express%201M2/Develop/quality-service/deploy/compose/docker-compose.yaml).

- `core`
  - `nats`
  - `configctl`
  - `server`
- `runtime`
  - everything in `core`
  - `validator`
- `dataplane`
  - everything in `runtime`
  - `kafka`
  - `consumer`
  - `emulator`

Key entry commands:

- `make up-core`
- `make up-runtime`
- `make up-dataplane`
- `make down`

## Service Roles

- `nats`
  - control, events, runtime cache, and result transport
- `kafka`
  - dataplane ingestion broker for the pipeline
- `configctl`
  - config lifecycle service: draft, validate, compile, activate, deactivate
- `server`
  - HTTP entrypoint with readiness and runtime/config routes
- `validator`
  - consumes projected runtime state and produces validation results
- `consumer`
  - bridges Kafka ingestion into the NATS/JetStream side
- `emulator`
  - produces dataplane traffic for smoke and runtime validation

## Data Flow

### Control plane

1. A config is created and moved through lifecycle stages in `configctl`.
2. Active runtime bindings are projected for downstream consumers.
3. `server` exposes HTTP access to health, readiness, config, and runtime operations.

### Data plane

1. `emulator` produces messages toward Kafka.
2. `consumer` reads Kafka topics and republishes the payload into NATS/JetStream.
3. `validator` consumes the projected runtime and dataplane messages.
4. Validation results are persisted and can be inspected with `make results-inspect`.

## Health And Readiness

- `nats` exposes `8222` health.
- `server` exposes `8080` and uses `/readyz`.
- the Go services use process-level healthchecks over the container command line.
- Compose dependencies enforce startup order between the services.

When validating the live cluster, use:

- `make ps`
- `make logs`
- `make check-deep`
- `make scenario-smoke SCENARIO=happy-path`

## Config Sources

The runtime topology is configured by:

- `deploy/configs/configctl.jsonc`
- `deploy/configs/server.jsonc`
- `deploy/configs/validator.jsonc`
- `deploy/configs/consumer.jsonc`
- `deploy/configs/emulator.jsonc`
- `deploy/nats/nats-server.conf`

Any change in these files should be treated as runtime-significant and verified with at least `make check`.

## Operational Invariants

- NATS and Kafka endpoints must stay aligned across config, compose, and source.
- Subjects, streams, and durables must stay consistent with the runtime bindings and contract registries.
- `server` readiness must remain healthy before `consumer` and `emulator` are considered valid.
- Runtime-facing changes should prefer `make check-deep` or a named `scenario-smoke`, not only unit tests.

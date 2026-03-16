---
type: doc
name: messaging-contracts
description: Canonical transport and payload contracts across control plane, event plane, dataplane, runtime bootstrap, and validation results
category: architecture
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Messaging Contracts

## Purpose

`quality-service` depends on stable contracts between HTTP, NATS request/reply, JetStream streams, Kafka topics, runtime bootstrap payloads, and validator result records. These contracts are not hidden inside adapters; they are explicit repository surfaces and are enforced by `contract-audit`, `runtime-bindings`, and related tests.

## Contract Surfaces

### 1. Configctl control plane

`configctl` exposes NATS request/reply control subjects through [`internal/adapters/nats/configctl_registry.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/adapters/nats/configctl_registry.go).

Core control subjects:

- `configctl.control.create_draft`
- `configctl.control.get_config`
- `configctl.control.get_active`
- `configctl.control.list_active_ingestion_bindings`
- `configctl.control.list_configs`
- `configctl.control.validate_draft`
- `configctl.control.validate_config`
- `configctl.control.compile_config`
- `configctl.control.activate_config`

Each control route carries:

- a request subject,
- a request type such as `configctl.command.*` or `configctl.query.*`,
- a reply type such as `configctl.reply.*`,
- queue group `configctl.control`.

This is the transport contract behind both the HTTP facade and internal clients.

### 2. Configctl event plane

Lifecycle events are emitted to stream `CONFIGCTL_EVENTS` with subject pattern `configctl.events.config.>`.

Transport event subjects include:

- `configctl.events.config.draft_created`
- `configctl.events.config.validated`
- `configctl.events.config.compiled`
- `configctl.events.config.activated`
- `configctl.events.config.deactivated`
- `configctl.events.config.ingestion_runtime_changed`
- `configctl.events.config.archived`
- `configctl.events.config.rejected`

The domain event names in [`internal/domain/configctl/events.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/domain/configctl/events.go) are shorter, for example `config.activated` and `config.ingestion_runtime_changed`. The repository intentionally distinguishes:

- **domain event name**
  - used inside Go domain/application logic
- **transport type and subject**
  - used in NATS/JetStream registries and analyzers

That separation must remain aligned.

Operationally, `config.ingestion_runtime_changed` is the canonical dataplane refresh signal after Block 5:

- `consumer` and `emulator` each attach their own durable consumer to the same transport subject
- the event triggers a reload of aggregate bootstrap state
- the event payload does not replace bootstrap as the source of truth

### 3. Validator runtime and results control

The validator exposes separate request/reply contracts:

- runtime query:
  - subject `validator.runtime.get_active`
  - type `validator.runtime.query.get_active`
  - reply `validator.runtime.reply.get_active`
  - queue group `validator.runtime`
- results query:
  - subject `validator.results.list`
  - type `validator.results.query.list`
  - reply `validator.results.reply.list`
  - queue group `validator.results`

These must remain separate from `configctl` subjects and from each other.

The validator also exposes an incident query contract for compact operational aggregation:

- incidents query:
  - subject `validator.incidents.list`
  - type `validator.incidents.query.list`
  - reply `validator.incidents.reply.list`
  - queue group `validator.incidents`

This surface is intentionally additive to `validator.results.list`. Results remain the per-message technical truth; incidents are the small operational view derived from those results.

### 4. Dataplane ingestion contract

The dataplane registry under [`internal/application/dataplane/registry.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/registry.go) defines the canonical ingestion route:

- stream: `DATA_PLANE_INGESTION`
- subject prefix: `dataplane.ingestion.received`
- subject pattern: `dataplane.ingestion.received.>`
- event type: `dataplane.event.ingestion.received`
- validator durable: `validator-dataplane-v1`

Subjects are derived from scope and binding identity:

`dataplane.ingestion.received.<scope-kind>.<scope-key>.<binding-name>`

Tokens are normalized to lowercase and sanitized for subject-safe routing.

### 5. Runtime bootstrap payloads

`consumer` and `emulator` do not hardcode dataplane topology. They bootstrap from `/runtime/ingestion/bindings`, which returns:

- `bindings`: `ActiveIngestionBindingRecord` values containing:
  - binding identity and Kafka topic,
  - field shape,
  - compact runtime metadata:
    - scope,
    - config version,
    - checksum,
    - artifact,
    - activation time.
- `runtimes`: the deduplicated compact active runtime set for the same bootstrap state.

Default scope behavior is operationally important:

- `scope_kind = global`
- `scope_key = default`

Those defaults appear repeatedly across active config, runtime lookup, ingestion bootstrap, and validation-result queries.

After Block 5, bootstrap has a stricter operational role:

- aggregate `/runtime/ingestion/bindings` is the canonical state source for dataplane refresh
- `config.ingestion_runtime_changed` is only the trigger that tells dataplane clients to reload that state
- dataplane bootstrap must include a compact `runtimes` set that matches the active bindings; bindings without matching runtime summaries are invalid bootstrap state
- signature comparison on the bootstrap state now includes compact runtime/artifact metadata as well as binding identity, which prevents silent drift when artifacts change without changing the topic map

### 6. Validation result payloads

Validation results are returned as `ValidationResultRecord` values and carry:

- `processing_key`
- `message_id`
- optional `correlation_id`
- binding identity and scope
- active config version metadata
- `status`: `passed` or `failed`
- `violations` when failed
- `processed_at`

The contract is strict:

- passed results must not contain violations
- failed results must contain at least one violation
- `processing_key` must stay stable for replay/redelivery of the same canonical dataplane message
- scope, binding, config version, and message identity must be populated

## Validation incident payloads

Validation incidents are returned as `ValidationIncidentRecord` values and carry:

- `incident_key`
- `kind`
- `status`
- binding identity and scope
- active config version metadata
- aggregate counters and timestamps:
  - `count`
  - `first_seen_at`
  - `last_seen_at`
- latest evidence pointers:
  - `latest_message_id`
  - optional `latest_correlation_id`
  - optional `latest_processing_key`
- representative `violations`

The contract is strict:

- incidents stay operational and compact; they are not notification workflow records
- incidents are derived from validation output and do not replace `ValidationResultRecord`
- the incident query surface must stay additive to the results query surface

## HTTP Contract Surface

The HTTP layer does not invent new semantics. It exposes transport-backed contracts through:

- `/configctl/configs`
- `/configctl/config-versions/:id`
- `/configctl/configs/active`
- `/runtime/configctl/projections`
- `/runtime/validator/active`
- `/runtime/ingestion/bindings`
- `/runtime/validator/results`

These endpoints should be read as stable runtime views over application contracts, not as an independent API design space.

- `/runtime/configctl/projections`
  - thin HTTP facade for `configctl.control.list_active_runtime_projections`
- `/runtime/ingestion/bindings`
  - thin HTTP facade for `configctl.control.list_active_ingestion_bindings`
  - keeps bootstrap-friendly `bindings` plus the compact `runtimes` summary in the same reply
- `/runtime/validator/active`
  - thin HTTP facade for `validator.runtime.get_active`
- `/runtime/validator/results`
  - thin HTTP facade for `validator.results.list`
- `/runtime/validator/incidents`
  - thin HTTP facade for `validator.incidents.list`

## Contract Invariants

- control, event, and dataplane subject spaces must remain separate
- validator runtime and results controls must remain separate from configctl controls
- every durable must target an existing stream and compatible subject filter
- dedicated durables for `consumer`, `emulator`, and `validator` must stay distinct even when they observe related config lifecycle subjects
- runtime bootstrap responses must stay sufficient for `consumer` and `emulator`
- domain event names, transport event types, and stream subjects must remain aligned
- content type and payload validity rules for dataplane messages must remain enforced

## Verification Commands

- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `make check`
- `make drift-detect`
- `make check-deep` when a live path is affected

## Cross-References

- [`architecture-runtime.md`](./architecture-runtime.md)
- [`cluster-quality.md`](./cluster-quality.md)
- [`tooling-raccoon-cli.md`](./tooling-raccoon-cli.md)

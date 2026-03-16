---
name: Contract Audit
description: Audit transport, payload, binding, and query contracts across configctl, server, consumer, validator, emulator, NATS, JetStream, and Kafka.
phases: [R, E, V]
---

# Contract Audit

## When to use

Use this skill when a change touches message subjects, request or reply types, stream membership, durable consumers, runtime binding payloads, or validator result queries.

## Input signals

- edits under `internal/adapters/nats/`
- edits under `internal/application/dataplane/`
- changes to configctl event names, validator query routes, or runtime binding records
- changes in `deploy/configs/*.jsonc` that affect transport or binding semantics
- `contract-audit`, `runtime-bindings`, or `scenario-smoke` failures

## Canonical steps

1. Identify the contract surface: control plane, event plane, dataplane, runtime bootstrap, or results query.
2. Trace producer, transport, and consumer ownership before editing names or payloads.
3. Confirm that subject space, event type, stream binding, queue group, and payload shape still agree.
4. Verify any scope defaults and binding identifiers used by `consumer`, `emulator`, and `validator`.
5. Run the static contract checks first, then escalate to a live scenario only if the path is runtime-significant.

## Relevant commands

- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- contract-audit`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- runtime-bindings`
- `make check`
- `make drift-detect`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`

## Common risks

- renaming a subject without updating both producer and consumer registries
- conflating domain event names with transport types or subjects
- changing payload fields without checking bootstrap and results consumers
- treating HTTP endpoints as an independent API when they are transport-backed runtime views

## Acceptance criteria

- producer, transport, and consumer paths are all updated or all intentionally unchanged
- static contract checks pass
- runtime validation exists when the changed path affects live message flow
- docs and CLI checks still describe the same contract surface
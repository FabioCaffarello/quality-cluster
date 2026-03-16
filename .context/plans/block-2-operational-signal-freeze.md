---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-2-operational-signal-hardening"
phase: "phase-1"
---

# Block 2 Signal Freeze

## Objective

Freeze the operational scope of Block 2 before implementation so signal cleanup stays narrow and Compose guard rails stay tied to real runtime invariants.

## Warning Inventory

The current cluster bootstrap shows repeated `unknown message` warnings for framework lifecycle messages that are expected during actor startup:

### Configctl

- `configctl event router` -> `actor.Initialized`
- `configctl control router` -> `actor.Initialized`
- `configctl control responder` -> `actor.Initialized`

### Consumer

- `consumer supervisor` -> `actor.Initialized`
- `consumer runtime` -> `actor.Initialized`
- `consumer publisher` -> `actor.Initialized`
- `consumer topic router` -> `actor.Initialized`
- `consumer topic router` -> `actor.Started`
- `kafka topic consumer` -> `actor.Initialized`

### Validator

- `validator runtime cache` -> `actor.Initialized`
- `validator results store` -> `actor.Initialized`
- `validation router` -> `actor.Initialized`
- `validation worker` -> `actor.Initialized`
- `validation worker` -> `actor.Started`
- `validator runtime consumer` -> `actor.Initialized`
- `validator data plane consumer` -> `actor.Initialized`
- `validator runtime responder` -> `actor.Initialized`
- `validator results responder` -> `actor.Initialized`

## Freeze Decision

Block 2 should treat the warning inventory above as noise candidates, not guaranteed bugs.

Implementation rule:

- expected Hollywood lifecycle messages such as `actor.Initialized` and `actor.Started` should not log as `WARN` by default in steady bootstrap
- unexpected domain messages, protocol failures, request handling anomalies and transport errors must continue to log at warning or error level

This block does **not** change actor ownership, supervision topology or runtime contracts.

## Compose Invariants To Protect Statically

The following invariants are important enough to catch before `make up-dataplane`:

- required services exist: `nats`, `kafka`, `configctl`, `server`, `validator`, `consumer`, `emulator`
- profile mapping remains coherent:
  - `core` -> `configctl`, `server`
  - `runtime` -> `validator`
  - `dataplane` -> `kafka`, `consumer`, `emulator`
  - `all` includes every supported service
- critical dependency chain remains intact:
  - `server` depends on `nats` and `configctl`
  - `validator` depends on `nats` and `configctl`
  - `consumer` depends on `server` and `kafka`
  - `emulator` depends on `server`, `validator`, `consumer` and `kafka`
- critical local ports stay stable:
  - `4222`, `8222` for NATS
  - `8080` for `server`
  - `19092` for Kafka external access
- image family drift stays explicit:
  - `nats:2.10.18-alpine`
  - `bitnamilegacy/kafka:3.9.0`
- Kafka and NATS broker names used by configs stay aligned with Compose service names:
  - `kafka:9092`
  - `nats://nats:4222`

## Existing Guard Rails To Reuse

- `raccoon-cli topology-doctor`
- `raccoon-cli drift-detect`
- `raccoon-cli runtime-bindings`
- `raccoon-cli quality-gate --profile deep`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`

## Out Of Scope

- redesign of `readyz`
- persistence or replay redesign
- changes to HTTP or NATS contracts
- changing Compose service ownership or adding new services
- suppressing all warnings globally without actor-by-actor review

## Immediate Execution Order

1. reduce non-actionable lifecycle warnings in `configctl`
2. reduce the same class of warnings in `consumer` and `validator`
3. add static protection for the frozen Compose invariants
4. prove no runtime regression with `readiness-probe`, `invalid-payload`, `happy-path` and `check-deep`

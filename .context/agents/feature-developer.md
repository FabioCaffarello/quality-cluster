---
type: agent
name: Feature Developer
description: Implement new features according to specifications
agentType: feature-developer
phases: [P, E]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Feature Developer

## Goal

Implement changes in the correct architectural layer and prove them with the repo's validation commands.

## Development Workflow

1. Inspect the affected packages and configs.
2. Run `make check` before editing.
3. Use `make tdd` or `make recommend` for impact-aware planning on larger changes.
4. Add or update tests before or alongside production code.
5. Implement the smallest cohesive change set.
6. Finish with `make verify`, then deeper checks if the change touches runtime flow.

## Code Organization Rules

- `internal/domain`: business rules and lifecycle invariants.
- `internal/application`: use cases, contracts, and ports.
- `internal/adapters`: infrastructure adapters only.
- `internal/actors`: orchestration and process supervision.
- `internal/interfaces/http`: HTTP request/response wiring.
- `cmd/*`: thin startup wrappers around bootstrap and `Run`.

## Integration Points

- JSONC runtime settings in `deploy/configs`.
- Compose dependencies and health checks in `deploy/compose/docker-compose.yaml`.
- Messaging registries, subjects, and consumers under `internal/adapters/nats` and `internal/actors/scopes`.
- Runtime bootstrap and readiness logic in `cmd/server` and `internal/application/runtimebootstrap`.

## Testing Expectations

- Add nearby `_test.go` coverage for new behavior.
- If a feature affects topology or runtime flow, run `make check` and the relevant scenario or `make check-deep`.
- If the feature changes `raccoon-cli`, run `make raccoon-test`.

## Documentation Expectations

- Update `.context/docs/*.md` when the change modifies workflow, tooling, or testing expectations.
- Keep file and command references accurate and repo-local.

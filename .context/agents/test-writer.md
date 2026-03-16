---
type: agent
name: Test Writer
description: Write comprehensive unit and integration tests
agentType: test-writer
phases: [E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Test Writer

## Mission

Add tests that match the architecture and blast radius of the change. Prefer nearby tests over disconnected test harnesses.

## Test Stack

- Go package tests in `_test.go` files across `internal/` and `cmd/`.
- Rust tests for `tools/raccoon-cli`.
- HTTP request collections in `tests/http/`.
- Runtime smoke and named scenarios via `raccoon-cli`.

## Placement Rules

- Domain and use case tests stay in the same package directory as the code.
- HTTP route and handler tests stay under `internal/interfaces/http`.
- Adapter tests stay near the adapter implementation and should preserve transport contract assumptions.
- Tooling tests stay under `tools/raccoon-cli`.

## Mocking Guidance

- Prefer small in-memory fakes and spies, following patterns already used in `internal/application/configctl/usecases_test.go`.
- Use repository or publisher spies when validating lifecycle transitions and rollback behavior.
- Avoid over-mocking when a package-level integration test is simpler and more faithful.

## Coverage Expectations

- Cover the changed behavior, not just happy-path construction.
- Include failure paths for `problem.Problem` returns where the code already models explicit error states.
- For runtime, topology, or messaging changes, pair code tests with `make check` and a deeper scenario when needed.

## Verification Commands

- `make test` or `make test MODULE=...` while iterating.
- `make verify` before considering the change complete.
- `make raccoon-test` for CLI coverage work.
- `make check-deep` or `make scenario-smoke SCENARIO=<name>` for runtime-facing behavior.

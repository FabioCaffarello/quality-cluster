---
type: doc
name: testing-strategy
description: Test frameworks, patterns, coverage requirements, and quality gates
category: testing
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Testing Strategy

## Testing Layers

The repository mixes unit, package integration, transport, HTTP, and runtime smoke testing.

- Go unit and package tests live next to the code in `_test.go` files across `internal/` and `cmd/`.
- Rust tests for `raccoon-cli` live under `tools/raccoon-cli`.
- Manual and scripted HTTP exercises live under [`tests/http/`](/Volumes/OWC%20Express%201M2/Develop/quality-service/tests/http).
- Runtime validation is handled by `raccoon-cli runtime-smoke` and `scenario-smoke`.

## Default Commands

- `make test`: runs `go test ./...` across every module listed by `scripts/utils/list-modules.sh`.
- `make verify`: runs `make test` plus the fast `quality-gate`.
- `make raccoon-test`: runs the Rust CLI test suite.
- `make quality-gate-ci`: strict static verification with JSON output.
- `make check-deep`: full validation including runtime smoke.

## What To Test

### Domain and application logic

- Prefer focused tests near the use case or entity being changed.
- Existing examples include `internal/domain/configctl/*_test.go` and `internal/application/configctl/usecases_test.go`.
- Validate lifecycle transitions, repository rollbacks, event publication, and contract mapping behavior.

### HTTP layer

- Add or update handler, route, and webserver tests under `internal/interfaces/http`.
- Keep readiness and health coverage in place when changing bootstrap or route registration.

### Adapters and messaging

- Exercise NATS and Kafka adapters through package tests where possible.
- Preserve envelope, codec, registry, and gateway expectations, especially around content type and subject naming.

### Runtime and topology

- When changing `deploy/configs`, compose wiring, subjects, streams, or consumer bindings, validate with `make check` at minimum.
- Use `make check-deep` or `make scenario-smoke SCENARIO=<name>` when the change can affect the live pipeline.

## Coverage Expectations

The repository does not define a numeric coverage threshold in-tree. The practical expectation is:

- every behavior change gets a nearby automated test or a runtime scenario,
- every topology or contract change is covered by the appropriate `raccoon-cli` analyzers,
- every meaningful change passes `make verify`, and
- wider runtime-impacting changes are proven with `make check-deep` or a scenario run.

Use `make coverage-map` to inspect protected areas and gaps before or after a change.

## Failure Diagnosis

- `make results-inspect` helps when validator outputs are missing or incorrect.
- `make trace-pack` is the preferred evidence bundle for runtime failures.
- `make logs SERVICE=<name>` and `make ps` help isolate unhealthy services during compose-based debugging.

## Test Placement Conventions

- Keep tests in the same package directory as the production code they validate.
- Name files with the standard `_test.go` suffix.
- For multi-step workflows, prefer end-to-end package tests that assert state transitions rather than isolated mocks only.
- For `raccoon-cli`, preserve stable output contracts because CI and humans both rely on them.

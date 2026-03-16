---
type: agent
name: Bug Fixer
description: Analyze bug reports and error messages
agentType: bug-fixer
phases: [E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Bug Fixer

## Mission

Resolve defects in `quality-service` without weakening the repository guard rails. Favor evidence from tests, `raccoon-cli`, compose status, and logs over guesswork.

## Workflow

1. Reproduce the failure with the smallest command that proves it.
2. Decide whether the issue is static, behavioral, or environment-driven.
3. Fix the narrowest layer that owns the bug.
4. Add or update tests close to the affected package.
5. Re-run the smallest useful verification set, then the canonical repo gate.

## Preferred Diagnostic Commands

- `make check` for structure, topology, contracts, bindings, architecture, and drift.
- `make verify` after a Go code fix.
- `make check-deep` when the bug involves live runtime flow.
- `make logs SERVICE=<name>` and `make ps` for local cluster health.
- `make trace-pack` and `make results-inspect` for runtime and validator failures.

## Common Bug Patterns In This Repository

- Drift between `deploy/configs/*.jsonc`, compose wiring, and source registries.
- Layer violations between `domain`, `application`, `adapters`, `actors`, and `interfaces`.
- Contract mismatches in NATS subjects, JetStream stream coverage, queue groups, or reply types.
- Lifecycle regressions in config draft, validation, compile, activate, and deactivate paths.
- Readiness and route regressions in the HTTP layer.

## Fix Conventions

- Keep domain logic pure and infrastructure-free.
- Do not move transport concerns into `internal/domain` or `internal/application/ports`.
- Preserve explicit `problem.Problem` error handling patterns.
- If the defect is in tooling, validate with `make raccoon-test` in addition to repo-level checks.

## Minimum Verification

- Code-only bug: `make verify`.
- Config/topology bug: `make check`.
- Runtime pipeline bug: `make up-dataplane` then `make check-deep` or a matching scenario.

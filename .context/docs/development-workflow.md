---
type: doc
name: development-workflow
description: Day-to-day engineering processes, branching, and contribution guidelines
category: workflow
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Development Workflow

## Canonical Loop

This repository already defines its preferred workflow in [`DEVELOPMENT.md`](/Volumes/OWC%20Express%201M2/Develop/quality-service/DEVELOPMENT.md). The short version is:

1. Understand the affected area before editing.
2. Confirm the repository is in a known-good state.
3. Add or update tests first when behavior is changing.
4. Implement the change.
5. Re-run the appropriate static and runtime checks.

The default guard rail is `make check`, which builds `raccoon-cli` if needed and runs the fast `quality-gate` profile.

## Recommended Day-To-Day Sequence

### 1. Inspect the target area

- Read the relevant package under `internal/`, `cmd/`, or `deploy/`.
- Use `make briefing TARGETS="..."` when you want structured context from `raccoon-cli`.
- For changes with broad surface area, use `make tdd` or `make recommend` before editing.

### 2. Confirm the baseline

- Run `make check` before coding.
- If the change is limited to Go packages, also run `make test MODULE=./path/to/module` where practical.
- If `make check` fails, fix the baseline first instead of stacking new changes on top.

### 3. Implement with the architecture in mind

- Keep domain rules in `internal/domain`.
- Keep use cases, contracts, and ports in `internal/application`.
- Keep infrastructure details inside `internal/adapters`.
- Keep actor orchestration inside `internal/actors`.
- Keep HTTP-only concerns inside `internal/interfaces/http`.

This separation is enforced by `raccoon-cli arch-guard` and is treated as a repository-level invariant.

### 4. Verify the change

- `make verify`: default post-change command. Runs Go tests plus the fast quality gate.
- `make quality-gate-ci`: strict static validation equivalent to CI expectations.
- `make check-deep`: full proof including runtime smoke. Requires `make up-dataplane`.
- `make scenario-smoke SCENARIO=happy-path`: targeted runtime proof for a specific scenario.

### 5. Troubleshoot with evidence

- `make trace-pack`: collect service status, responses, configs, and logs.
- `make results-inspect`: inspect validator outputs.
- `make logs SERVICE=<name>` or `make ps`: inspect the compose environment.

## Change Types

### Application or domain change

- Start with `make check`.
- Update or add `_test.go` files close to the affected package.
- Finish with `make verify`.

### Messaging, config, or topology change

- Start with `make check`.
- Run targeted analyzers such as `make drift-detect`, `make arch-guard`, or `raccoon-cli topology-doctor`.
- If the change affects runtime flow, validate with `make check-deep` or a matching `scenario-smoke`.

### Tooling or CLI change

- Work under `tools/raccoon-cli`.
- Run `make raccoon-test`.
- If the tooling change alters validation behavior, also run the relevant root-level `make check` or `make quality-gate-ci`.

## Branching And Contribution Notes

The repository does not define an in-tree branching or PR policy beyond the workflow docs above. If your team has external conventions, apply them. Inside the repo, the important invariant is that changes are proven by the correct gate for their blast radius before merge.

## Fast Command Map

- `make check`: pre-change baseline.
- `make verify`: post-change safety check.
- `make check-deep`: static plus runtime smoke.
- `make tdd`: structural impact plus test guidance.
- `make recommend`: what to validate for the current diff.
- `make trace-pack`: debugging bundle for failures.

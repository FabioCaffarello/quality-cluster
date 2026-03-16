---
name: Runtime Validation
description: Validate runtime-significant changes through the repository's real cluster workflow, from static gate to live smoke and result proof.
phases: [E, V]
---

# Runtime Validation

## When to use

Use this skill when a change affects live cluster behavior and needs proof beyond unit tests or static analyzers.

## Input signals

- edits under `cmd/`, `internal/`, `deploy/configs/`, or `deploy/compose/`
- changes to startup, runtime bindings, readiness, ingestion routing, or validator results
- `make recommend` points to deep or scenario validation
- `make verify` is green but the change still needs live cluster proof

## Canonical steps

1. Clear the fast static gate first with `make check` or `make verify`.
2. Decide whether the claim needs broad runtime proof or a named scenario.
3. Start the dataplane stack when live validation is required.
4. Run the smallest runtime command that can fail for the right reason.
5. If runtime proof fails, inspect results, logs, and traces before changing code again.
6. Close the task only when the changed path has explicit live evidence.

## Relevant commands

- `make check`
- `make verify`
- `make recommend`
- `make up-dataplane`
- `make check-deep`
- `make runtime-smoke`
- `make scenario-smoke SCENARIO=<name>`
- `make results-inspect`
- `make trace-pack`
- `make ps`
- `make logs SERVICE=<name>`

## Common risks

- jumping straight to deep validation before clearing static drift or contract failures
- using `runtime-smoke` when a narrower named scenario would prove the change better
- accepting a green `make verify` as sufficient for startup, routing, or result-flow changes
- stopping at service health without checking produced results or observable runtime evidence

## Acceptance criteria

- the selected runtime check matches the actual blast radius of the change
- static gates are green before live proof is used as the final signal
- live validation exists for the touched runtime path
- failures, if any, are explained with logs, results, or trace artifacts rather than guesses
---
type: skill
name: Scenario Design
description: Design the smallest scenario-smoke path that proves a runtime, contract, or validation claim in the quality-service cluster.
skillSlug: scenario-design
phases: [P, E, V]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Scenario Design

## When to use

Use this skill when a change needs live proof and you must choose or shape the correct `scenario-smoke` path instead of running the whole cluster workflow blindly.

## Input signals

- a diff touches runtime bootstrap, bindings, results, or validator behavior
- `make recommend` points to smoke or deep validation
- a bug report requires reproducing a real traffic path
- an existing scenario is too broad, too weak, or mismatched to the claim

## Canonical steps

1. State the behavior claim in one sentence.
2. Map the minimum required path: control-plane only, dataplane ingestion, invalid payload handling, missing binding handling, or readiness.
3. Choose the smallest existing scenario that can fail for the right reason.
4. Define what evidence proves success: HTTP response, results record, service log, or trace artifact.
5. Pair the scenario with the static checks that should run before it.

## Relevant commands

- `make recommend`
- `make tdd`
- `make check`
- `make check-deep`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=config-lifecycle`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make scenario-smoke SCENARIO=missing-binding`
- `make scenario-smoke SCENARIO=readiness-probe`

## Common risks

- using `happy-path` when a narrower scenario would isolate the bug faster
- designing a scenario that proves only startup and not the changed runtime behavior
- skipping static checks and discovering obvious drift only after a live run
- failing to define what observable artifact should change

## Acceptance criteria

- the chosen scenario is the smallest one that proves the claim
- the expected evidence is explicit before execution
- static and live checks are sequenced intentionally
- another engineer can run the same scenario and reach the same conclusion

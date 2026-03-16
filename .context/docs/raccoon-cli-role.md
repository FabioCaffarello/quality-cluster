---
type: doc
name: raccoon-cli-role
description: Role of raccoon-cli in repository validation, analysis, and engineering workflow
category: tooling
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Raccoon CLI Role

## What It Is

`tools/raccoon-cli` is the repository's engineering quality platform. It is not an accessory utility. It is the main mechanism that turns repository structure, contracts, topology, architecture, and runtime expectations into repeatable checks.

It is intentionally isolated from the Go runtime:

- it reads files, configs, and source;
- it performs structural analysis over the repository;
- it only touches live runtime concerns when executing smoke or diagnostic commands.

## Why It Matters

The repository is operationally complex:

- multiple Go binaries,
- layered architecture,
- NATS and Kafka integration,
- JSONC runtime config,
- Compose-driven local cluster,
- runtime and contract drift risk.

`raccoon-cli` exists to make those risks observable before and after changes.

## Core Responsibilities

### Structure and topology

- `doctor`
- `topology-doctor`

These check whether the repository layout, configs, compose wiring, and source-level topology still agree.

### Contract integrity

- `contract-audit`
- `runtime-bindings`

These check subjects, streams, queue groups, envelopes, payload expectations, and routing continuity.

### Architecture enforcement

- `arch-guard`
- `drift-detect`

These catch layer violations and cross-layer drift between declarations, config, compose, and source.

### Planning and validation support

- `tdd`
- `coverage-map`
- `recommend`
- `impact-map`
- `symbol-trace`
- `rename-safety`
- `snapshot`
- `baseline-drift`

These are not documentation helpers. They are change-planning and risk-calibration tools for real repository work.

### Live runtime proof

- `runtime-smoke`
- `scenario-smoke`
- `trace-pack`
- `results-inspect`

These prove and diagnose end-to-end behavior when static checks are not enough.

## Operational Position In The Workflow

The real repository workflow is:

1. `make check` before coding
2. `make tdd` or `make recommend` for impact-aware planning
3. implement the change
4. `make verify` for normal completion
5. `make check-deep` or `make scenario-smoke` for runtime-significant work

That means `.context` should always treat `raccoon-cli` as a primary source of operational truth.

## Maintenance Implications

Changes under `tools/raccoon-cli` require different review attention than normal Go changes:

- preserve output contracts;
- preserve analyzer intent;
- keep the tooling independent from Go runtime imports;
- run `make raccoon-test`;
- recheck the commands in `DEVELOPMENT.md` and the `Makefile` if behavior changed.

---
type: doc
name: tooling
description: Scripts, IDE settings, automation, and developer productivity tips
category: tooling
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Tooling Guide

## Primary Tooling Surface

The repository is operated primarily through the root [`Makefile`](/Volumes/OWC%20Express%201M2/Develop/quality-service/Makefile). Prefer Make targets over ad-hoc commands because the targets encode module iteration, compose profiles, and the canonical quality workflow.

## Core Tools

- Go workspace tooling via [`go.work`](/Volumes/OWC%20Express%201M2/Develop/quality-service/go.work).
- Docker Compose via [`deploy/compose/docker-compose.yaml`](/Volumes/OWC%20Express%201M2/Develop/quality-service/deploy/compose/docker-compose.yaml).
- Rust/Cargo for `tools/raccoon-cli`.
- Shell helpers under `scripts/utils/`.

## Useful Make Targets

- `make build`: compile all service binaries into `bin/`.
- `make docker-build`: build local service images.
- `make up-core`, `make up-runtime`, `make up-dataplane`, `make down`: manage local environments.
- `make logs`, `make ps`, `make restart`: inspect and control running services.
- `make check`, `make verify`, `make quality-gate-ci`, `make check-deep`: validation entrypoints.
- `make tdd`, `make recommend`, `make coverage-map`, `make trace-pack`: engineering support commands.

## Raccoon CLI

`tools/raccoon-cli` is a first-class repository tool, not an optional side project. It provides:

- structural checks with `doctor`,
- topology and contract analysis,
- architecture and drift detection,
- impact-aware guidance (`tdd`, `recommend`),
- runtime smoke validation,
- evidence collection (`trace-pack`).

Build it with `make raccoon-build` or directly with `cargo build --release --manifest-path tools/raccoon-cli/Cargo.toml`.

## Scripts

- `scripts/utils/list-modules.sh`: enumerates Go workspace modules for repo-wide operations.
- `scripts/utils/for-each-module.sh`: applies a command across modules and respects `MODULE=...` scoping.

These scripts are part of the contract behind `make tidy` and other multi-module commands. If module layout changes, update the scripts together with `go.work`.

## Configuration And Environment Files

- Service configs live in `deploy/configs/*.jsonc`.
- NATS local config lives in `deploy/nats/nats-server.conf`.
- Compose health checks and dependency graph live in `deploy/compose/docker-compose.yaml`.

When editing runtime settings, keep these files aligned with Go source expectations and verify with `make check`.

## Productivity Notes

- Use `MODULE=./internal/shared` to scope `make test` or `make tidy`.
- Use `SERVICE=server` to scope `make build`, `make docker-build`, `make logs`, or `make restart`.
- Prefer `make recommend` before broad changes and `make trace-pack` after runtime failures.
- Keep generated context and analysis artifacts under `.context/` when possible so the repository root stays clean.

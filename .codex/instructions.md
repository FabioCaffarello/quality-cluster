# Project Rules and Guidelines

> Auto-generated from .context/docs on 2026-03-16T19:55:02.783Z

## README

# Documentation Index

Welcome to the repository knowledge base. Use the canonical set first. Supporting guides exist to deepen or decompose the same runtime truth, not to compete with it.

## Canonical Set

- [Project Overview](./project-overview.md)
- [Architecture Runtime](./architecture-runtime.md)
- [Cluster Quality](./cluster-quality.md)
- [Messaging Contracts](./messaging-contracts.md)
- [Tooling Raccoon CLI](./tooling-raccoon-cli.md)

These files are the primary source of truth for Codex, Opus, and future automations.

## Supporting Guides
- [Initial Context Audit](./context-audit.md)
- [Runtime Topology](./runtime-topology.md)
- [Architecture Layers](./architecture-layers.md)
- [Raccoon CLI Role](./raccoon-cli-role.md)
- [Development Workflow](./development-workflow.md)
- [Testing Strategy](./testing-strategy.md)
- [Tooling & Productivity Guide](./tooling.md)

## Workflow Maintenance

- [Context Maintenance Plan](../workflow/plan.md)
- [`../workflow/status.yaml`](../workflow/status.yaml)

These files define when `.context/` must be updated and what maintenance loop agents should follow.

## Repository Snapshot
- `bin/`
- `cmd/`
- `deploy/`
- `DEVELOPMENT.md`
- `go.work`
- `go.work.sum`
- `internal/`
- `Makefile`
- `scripts/`
- `tests/` — HTTP request collections and validation fixtures.
- `tools/` — includes `raccoon-cli`, the repository quality toolkit.

## Document Map
| Guide | File | Primary Inputs |
| --- | --- | --- |
| Project Overview | `project-overview.md` | `go.work`, `Makefile`, `deploy/`, `cmd/`, `internal/`, `codebase-map.json` |
| Architecture Runtime | `architecture-runtime.md` | `deploy/compose`, `cmd/*`, `internal/actors`, `internal/interfaces/http`, `internal/application` |
| Cluster Quality | `cluster-quality.md` | `DEVELOPMENT.md`, `Makefile`, `tools/raccoon-cli/README.md`, `tests/http` |
| Messaging Contracts | `messaging-contracts.md` | `internal/adapters/nats/*registry*.go`, `internal/application/*/contracts`, `internal/domain/configctl/events.go` |
| Tooling Raccoon CLI | `tooling-raccoon-cli.md` | `tools/raccoon-cli/src/main.rs`, `tools/raccoon-cli/README.md`, `Makefile`, `DEVELOPMENT.md` |
| Context Maintenance Plan | `../workflow/plan.md` | `Makefile`, canonical docs, agents, skills, `AGENTS.md`, MCP adherence checks |
| Initial Context Audit | `context-audit.md` | `context.getMap`, `context.listToFill`, `go.work`, `deploy/`, `internal/`, `tools/raccoon-cli` |
| Runtime Topology | `runtime-topology.md` | `deploy/compose`, `deploy/configs`, `DEVELOPMENT.md`, runtime smoke docs |
| Architecture Layers | `architecture-layers.md` | `internal/`, `tools/raccoon-cli arch-guard`, `go.work` |
| Raccoon CLI Role | `raccoon-cli-role.md` | `tools/raccoon-cli/README.md`, `Makefile`, `DEVELOPMENT.md` |
| Development Workflow | `development-workflow.md` | `DEVELOPMENT.md`, `Makefile`, `tools/raccoon-cli/README.md` |
| Testing Strategy | `testing-strategy.md` | `_test.go` files, `tests/http`, `Makefile`, `tools/raccoon-cli` |
| Tooling & Productivity Guide | `tooling.md` | `Makefile`, `scripts/utils`, `go.work`, `deploy/compose` |


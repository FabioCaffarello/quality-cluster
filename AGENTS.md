# AGENTS.md

## Purpose

This file is the root operating contract for AI agents working in `quality-service`.

- `AGENTS.md` defines repository-wide rules, source-of-truth files, and the default validation workflow.
- `.context/agents/` contains task-specific playbooks that specialize those rules for cluster architecture, runtime validation, contracts, CLI evolution, and TDD planning.
- `.context/docs/` contains the canonical runtime, contract, cluster-quality, and tooling references that both humans and agents should prefer over assumptions.

## Repository Reality

This repository is not a generic app scaffold.

- Runtime: Go services under `cmd/` and `internal/`
- Cluster: Docker Compose + NATS + Kafka + five main binaries (`configctl`, `server`, `consumer`, `validator`, `emulator`)
- Tooling: Rust quality control plane in `tools/raccoon-cli`
- Workflow: Make targets wrap the real validation path and should be preferred over ad hoc command selection

## Default Workflow

Use the repository workflow in this order unless the task clearly needs a narrower path.

1. `make check`
2. `make tdd`
3. implement the smallest correct change
4. `make verify`
5. escalate to `make check-deep` or `make scenario-smoke SCENARIO=<name>` for runtime-significant work
6. use `make trace-pack`, `make results-inspect`, `make logs`, and `make ps` for troubleshooting

If the task touches `tools/raccoon-cli`, also run:

- `make raccoon-test`
- `make quality-gate-ci`

## Primary Source Files

Read these before making non-trivial changes:

- `Makefile`
- `DEVELOPMENT.md`
- `.context/docs/project-overview.md`
- `.context/docs/architecture-runtime.md`
- `.context/docs/cluster-quality.md`
- `.context/docs/messaging-contracts.md`
- `.context/docs/tooling-raccoon-cli.md`

## Repository Map

- `cmd/`
  - Go entrypoints for the runtime binaries
- `internal/`
  - domain, application, adapters, actors, interfaces, and runtime wiring
- `deploy/`
  - Compose topology and JSONC runtime/config declarations
- `tools/raccoon-cli/`
  - Rust analyzers, gates, smoke helpers, diagnostics, and code intelligence
- `tests/`
  - integration and scenario-oriented test assets
- `.context/docs/`
  - canonical operational and architectural context
- `.context/agents/`
  - specialized playbooks for recurring repository tasks
- `.context/skills/`
  - reusable execution procedures tied to the real cluster and CLI workflow

## Agent Selection

Prefer these `.context/agents` playbooks for daily work:

- `cluster-architect`
  - service ownership, topology, Compose profiles, cross-service design
- `runtime-validator`
  - smoke, readiness, live proof, troubleshooting
- `cli-evolution`
  - `raccoon-cli` analyzers, gate behavior, diagnostics, output contracts
- `contract-guardian`
  - subjects, streams, payloads, runtime bindings, result queries
- `tdd-coordinator`
  - choosing the smallest reliable validation ladder before implementation

Use the older generic or audit-oriented agents only as secondary helpers after selecting the primary repo-specific playbook.

## Testing And Validation Rules

- Do not substitute local compilation for cluster validation when a change affects runtime behavior.
- Do not change message contracts without running the contract and binding checks.
- Do not treat docs as truth if code, config, or `raccoon-cli` analyzer output disagree; resolve the drift.
- Prefer `make` targets over direct raw commands unless debugging requires a lower-level invocation.

## Alignment With AI Context

- Agent handbook: `.context/agents/README.md`
- Skills index: `.context/skills/README.md`
- Documentation index: `.context/docs/README.md`

When `AGENTS.md` and a playbook are both relevant:

- `AGENTS.md` sets the repository-wide rules
- `.context/agents/*.md` specializes execution for the task
- `.context/docs/*.md` remains the canonical system description

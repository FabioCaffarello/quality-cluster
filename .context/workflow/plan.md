# Context Maintenance Plan

## Purpose

This workflow keeps `.context/` aligned with the repository's real operating model. It is not a one-time bootstrap checklist. It is the maintenance loop for runtime truth, contract truth, validation truth, and agent or skill reuse.

## Canonical Scope

Keep these areas aligned as one system:

- `.context/docs/`
  - canonical system description
- `.context/agents/`
  - task owners and decision boundaries
- `.context/skills/`
  - reusable execution procedures
- `.context/workflow/`
  - maintenance loop, status, and handoff state

If one of these changes without the others when required, the context has drifted.

## Update Triggers

Update `.context/` whenever a repository change affects any of the following:

- topology
  - `deploy/compose/docker-compose.yaml`
  - service ownership across `cmd/` and `internal/actors`
- contracts
  - NATS subjects, JetStream streams, Kafka routes, payload shapes, result queries
- bindings
  - runtime ingestion bindings, scope defaults, validator runtime projection
- quality-gate
  - `make check`, `make verify`, `make check-deep`, `make quality-gate-ci`
  - `raccoon-cli` analyzers, gate profiles, diagnostics
- scenario-smoke
  - available scenarios, their purpose, or required evidence
- AST or LSP intelligence
  - `tools/raccoon-cli/src/codeintel`
  - `tools/raccoon-cli/src/lsp`
  - command behavior that affects `tdd`, `recommend`, `impact-map`, `symbol-trace`, or snapshots
- cluster workflow
  - bring-up, troubleshooting, trace collection, or done-definition changes in `DEVELOPMENT.md`, `Makefile`, or runtime docs

## Continuous PREVC Loop

### P: Plan the context change

- identify which canonical docs, agents, skills, and workflow files are impacted
- classify the change as topology, contract, binding, gate, scenario, semantic tooling, or workflow
- choose the smallest maintenance update set that keeps repository truth consistent
- start with:
  - `make check`
  - `make tdd`
  - `make recommend`

### R: Review repository truth

- compare code, configs, docs, agents, and skills for drift
- use MCP outputs as secondary evidence, not as primary truth
- prioritize:
  - `make arch-guard`
  - `make drift-detect`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- doctor`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- contract-audit`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- runtime-bindings`

### E: Update the context

- edit only the canonical files affected by the repository change
- keep `docs` dense and operational
- keep `agents` role-specific and non-overlapping
- keep `skills` procedural and tied to real commands
- update workflow files when maintenance triggers, gate behavior, or handoff rules change

### V: Validate the context

- re-run the minimum validation set required by the changed surfaces
- use live proof when the repository change is runtime-significant:
  - `make up-dataplane`
  - `make check-deep`
  - `make scenario-smoke SCENARIO=<name>`
  - `make results-inspect`
  - `make trace-pack`
- if `tools/raccoon-cli` changed, also run:
  - `make raccoon-test`
  - `make quality-gate-ci`
- confirm `.context` integrity with MCP `context.check`

### C: Confirm and hand off

- confirm that docs, agents, skills, and workflow still describe the same system
- record what changed in `status.yaml`
- keep the next maintenance trigger obvious for Codex, Opus, or automation

## Canonical Ownership Map

- runtime and topology
  - docs: `architecture-runtime.md`, `runtime-topology.md`, `cluster-quality.md`
  - agents: `cluster-architect`, `runtime-validator`
  - skills: `runtime-validation`, `cluster-debugging`, `scenario-design`
- contracts and bindings
  - docs: `messaging-contracts.md`
  - agents: `contract-guardian`
  - skills: `contract-audit`
- quality gate and semantic tooling
  - docs: `tooling-raccoon-cli.md`, `raccoon-cli-role.md`, `development-workflow.md`
  - agents: `cli-evolution`, `tdd-coordinator`
  - skills: `cli-quality-gate`, `semantic-drift-review`

## Maintenance Rules

- Prefer updating an existing canonical file over creating a new document fragment.
- Remove stale scaffold output when it contradicts repository reality.
- Do not keep generic placeholder content inside `.context/`.
- If MCP codebase maps or fill suggestions underfit the repository, prefer the code, configs, and canonical docs, then update the context manually.
- A change is incomplete if it altered runtime truth but left `.context/` misleading.

## Done Definition

The context maintenance cycle is done only when:

- the affected canonical files were updated,
- redundant or stale context was removed or explicitly downgraded,
- relevant static and runtime validations were executed,
- `context.check` still reports the `.context` as initialized,
- another agent can recover the same repository truth from `.context/` without reading stale scaffolds.

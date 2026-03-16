---
type: doc
name: context-audit
description: Initial adherence audit between AI Context scaffold and the real repository
category: overview
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Initial Context Adherence Audit

## Audit Scope

This audit compares the current `ai-context` scaffold with the real `quality-service` repository. The goal is not to fill the entire `.context` yet. The goal is to decide what should be filled first, what should be kept, and what is too generic for this repository.

## Evidence Used

### AI Context MCP

- `context.check` confirmed `.context` is initialized and has docs, agents, and plans.
- `context.listToFill` still reports the main docs and agents as candidates to fill, even after they contain repository-specific content. This suggests the MCP fill-state detection is conservative.
- `context.getMap` produced a generic codebase map that underfit the repository.
- `context.buildSemantic` in `compact` mode returned almost no semantic detail.
- `scaffoldPlan` for this audit generated useful fill instructions but generic content.

### Real Repository

- Go workspace in [`go.work`](/Volumes/OWC%20Express%201M2/Develop/quality-service/go.work) with multiple modules under `cmd/` and `internal/`.
- Docker Compose cluster in [`deploy/compose/docker-compose.yaml`](/Volumes/OWC%20Express%201M2/Develop/quality-service/deploy/compose/docker-compose.yaml) with NATS, Kafka, `configctl`, `server`, `validator`, `consumer`, and `emulator`.
- Runtime and configuration contracts in `deploy/configs/*.jsonc`, `internal/application`, `internal/adapters`, and `internal/actors`.
- Rust quality tooling in [`tools/raccoon-cli/README.md`](/Volumes/OWC%20Express%201M2/Develop/quality-service/tools/raccoon-cli/README.md) and `tools/raccoon-cli/src/**`.
- Canonical developer workflow in [`DEVELOPMENT.md`](/Volumes/OWC%20Express%201M2/Develop/quality-service/DEVELOPMENT.md) and [`Makefile`](/Volumes/OWC%20Express%201M2/Develop/quality-service/Makefile).

## Scaffold vs Reality

### Accurate enough in the scaffold

- The base doc set is directionally useful: project overview, development workflow, testing strategy, and tooling all matter.
- The core agent set is mostly useful: feature developer, code reviewer, bug fixer, test writer, documentation writer, and refactoring specialist all map to real work in this repository.
- PREVC workflow is a good fit for non-trivial changes.

### Under-specified or incorrect in the scaffold

- The MCP codebase map missed the real architecture:
  - it did not detect Docker even though `deploy/compose/docker-compose.yaml` exists;
  - it did not detect the layered Go architecture under `internal/domain`, `internal/application`, `internal/adapters`, `internal/actors`, and `internal/interfaces`;
  - it treated the repository mostly as a generic CLI project;
  - it did not surface `raccoon-cli` as a first-class engineering subsystem;
  - it reported no symbols and no imports, which is not representative.
- Generic workflow orchestration recommended agents such as `frontend-specialist`, `mobile-specialist`, and `database-specialist`, which are not aligned with this repository.
- The generated skills are all generic templates. Some are useful starting points, but several are too broad or irrelevant for the first pass.

## What The Repository Actually Needs First

## Docs

### Keep and refine first

- `project-overview.md`
- `development-workflow.md`
- `testing-strategy.md`
- `tooling.md`

These are already the right top-level documents because they explain:

- the Go workspace and service topology,
- the cluster/runtime workflow,
- the Rust tooling and quality gate,
- the real test and validation entrypoints.

### Add next because they are operationally important

- `runtime-topology.md`
  - Purpose: explain cluster composition, data flow, health checks, and the relation between NATS, Kafka, `consumer`, `validator`, and `emulator`.
- `architecture-layers.md`
  - Purpose: explain the real codebase layering under `internal/`.
- `raccoon-cli-role.md`
  - Purpose: document why `raccoon-cli` is not optional tooling, but part of the repository control plane for quality.

### Defer because they would be ornamental too early

- generic contributor guides detached from `DEVELOPMENT.md`
- glossary-only docs
- generic API docs not anchored in current HTTP handlers and runtime contracts

## Agents

### Keep and adapt first

- `feature-developer`
- `code-reviewer`
- `bug-fixer`
- `test-writer`
- `documentation-writer`
- `refactoring-specialist`

These match the real work surface of the repository.

### Keep, but lower priority

- `performance-optimizer`

It is useful, but it should not be one of the first playbooks to deepen because the immediate gap is adherence, not profiling.

### Domain-specific agents added after the first audit pass

- `runtime-topology-auditor`
  - Focus on cluster wiring, Compose, JSONC config, health checks, and pipeline continuity.
- `contract-auditor`
  - Focus on control/event/dataplane contracts, subjects, streams, queue groups, and drift detection.
- `raccoon-cli-maintainer`
  - Focus on Rust analyzers, codeintel, output contracts, and validation pipeline behavior.

These are now part of `.context/agents/` because they are materially more useful than generic agent suggestions returned by the scaffold.

## Skills

### Priority skills to fill first

- `bug-investigation`
- `code-review`
- `documentation`
- `refactoring`
- `test-generation`

These map directly to the operational needs of this repository.

### Useful later, but not first-pass priorities

- `security-audit`
  - Relevant, but should come after the core repository context is sharper.
- `feature-breakdown`
  - Useful once the context is richer and task decomposition is grounded in the real architecture.

### Too generic or low-value for the first pass

- `api-design`
  - Too generic for a repository where the critical complexity is runtime topology and contracts, not generic REST design.
- `commit-message`
  - Useful convenience, but not part of the core repository context.
- `pr-review`
  - Depends on team process details not visible in-tree.

### Deferred until immediate practical use

The remaining unfilled skills should stay deferred until a real task triggers them:

- `security-audit` when the work is explicitly security-sensitive
- `feature-breakdown` when a concrete multi-step feature enters planning
- `api-design` when HTTP or contract redesign becomes an actual task
- `commit-message` when commit authoring becomes part of the operational workflow
- `pr-review` when there is a real PR review flow to codify from repository behavior

This repository benefits more from task-triggered skill filling than from blanket completion of all scaffolded skills.

## Workflow

### What should remain

- PREVC is a good default model.
- `require_plan` should remain enabled for real changes.
- Bootstrap/setup work can stay `SMALL`.

### What should be adapted in practice

- Runtime or deploy changes should usually be handled as `MEDIUM` or `LARGE`, not `SMALL`.
- Validation outputs should explicitly reference:
  - `make check`
  - `make verify`
  - `make check-deep`
  - `make scenario-smoke`
  - `make raccoon-test`
- Handoffs should prefer repository-specific agents, not the generic frontend/mobile/database suggestions returned by the MCP.

## Initial Fill Strategy

### Fill first

1. Docs that anchor reality:
   - `runtime-topology.md`
   - `architecture-layers.md`
   - `raccoon-cli-role.md`
2. Skills that support real work:
   - `bug-investigation`
   - `code-review`
   - `test-generation`
   - `documentation`
   - `refactoring`
3. Agent refinements:
   - deepen the existing core agents
   - add repo-specific runtime and contract auditors later

### Do not fill first

- generic workflow ornaments
- skills detached from runtime, architecture, or tooling
- anything that assumes a frontend, mobile app, or conventional database layer

## Gaps Between Scaffold And Reality

- The scaffold understands that the repository is technical and multi-part, but it does not understand the real operational center of gravity.
- The real center of gravity is:
  - Go service runtime,
  - cluster topology and config projection,
  - messaging contracts,
  - `raccoon-cli` as enforcement tooling,
  - validation discipline encoded in `DEVELOPMENT.md` and the `Makefile`.

The next fill cycle should therefore optimize for operational and architectural truth, not for breadth.

## Consolidation Status

This first audit has now been carried into a consolidated `.context`:

- canonical runtime, contract, quality, and tooling docs are filled
- repo-specific daily agents are present and separated from secondary helpers
- repository skills now capture cluster debugging, contract audit, scenario design, quality-gate use, semantic drift review, and runtime validation
- workflow maintenance now lives in `.context/workflow/plan.md` and `.context/workflow/status.yaml`
- generic QA scaffold files were removed because they contradicted repository reality

The main remaining limitation is MCP under-detection, not missing repository context. `context.check` confirms the scaffold is active, but `listToFill` still flags files conservatively. Treat MCP fill-state as a hint, not as the final authority on adherence.

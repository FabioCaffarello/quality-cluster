---
status: filled
generated: 2026-03-16
agents:
  - type: "documentation-writer"
    role: "Audit the context scaffold against the real repository and write the initial context docs"
  - type: "code-reviewer"
    role: "Challenge generic scaffold output and validate repository adherence"
  - type: "feature-developer"
    role: "Apply the minimal .context structure changes needed for the first pass"
  - type: "test-writer"
    role: "Define operational validation criteria for future fills"
docs:
  - "project-overview.md"
  - "development-workflow.md"
  - "testing-strategy.md"
  - "tooling.md"
  - "context-audit.md"
phases:
  - id: "phase-1"
    name: "Audit the scaffold"
    prevc: "P"
    agent: "documentation-writer"
  - id: "phase-2"
    name: "Prioritize the first fill set"
    prevc: "E"
    agent: "feature-developer"
  - id: "phase-3"
    name: "Validate adherence criteria"
    prevc: "V"
    agent: "code-reviewer"
---

# Auditoria inicial de aderencia do .context Plan

> Auditar a aderencia entre o scaffold do `ai-context` e a realidade do repositorio `quality-service`, definindo quais docs, agents e skills devem ser priorizados e quais sao genericos demais para o primeiro ciclo de preenchimento.

## Task Snapshot

- **Primary goal:** produce a precise, repository-grounded first-fill plan for `.context`, without trying to fill every generated artifact.
- **Success signal:** the repository has an explicit audit of scaffold adherence plus a short, ordered fill strategy rooted in the real codebase and MCP evidence.
- **In scope:** `.context/docs`, `.context/agents`, `.context/skills`, `.context/workflow`, MCP map quality, first-pass priorities.
- **Out of scope:** filling every skill, inventing missing repository process, or introducing decorative documentation.

## Codebase Context

- The repository is a Go workspace with multiple service entrypoints in `cmd/`.
- The runtime model depends on Docker Compose, NATS, Kafka, JSONC service configs, and HTTP readiness.
- The repository also contains `tools/raccoon-cli`, a Rust CLI that acts as the engineering quality platform for the Go system.
- The actual architecture is layered under `internal/domain`, `internal/application`, `internal/adapters`, `internal/actors`, and `internal/interfaces/http`.

## Agent Lineup

| Agent | Role in this plan | Playbook | Focus |
| --- | --- | --- | --- |
| Documentation Writer | Convert MCP scaffold into repository-specific context | [Documentation Writer](../agents/documentation-writer.md) | Audit docs and prioritize operational knowledge |
| Code Reviewer | Check for generic or incorrect scaffold assumptions | [Code Reviewer](../agents/code-reviewer.md) | Compare MCP outputs with the real repo |
| Feature Developer | Apply the minimum file changes for the first-pass context structure | [Feature Developer](../agents/feature-developer.md) | Create audit artifacts and first-fill plan |
| Test Writer | Define how future context fills will be validated | [Test Writer](../agents/test-writer.md) | Establish lightweight completion criteria |

## Documentation Touchpoints

| Guide | File | Why it matters now |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | Keeps the repo-level purpose grounded |
| Development Workflow | [development-workflow.md](../docs/development-workflow.md) | Captures the true execution discipline of the repo |
| Testing Strategy | [testing-strategy.md](../docs/testing-strategy.md) | Anchors future validation and fill completion criteria |
| Tooling | [tooling.md](../docs/tooling.md) | Connects the Go runtime and `raccoon-cli` workflows |
| Context Audit | [context-audit.md](../docs/context-audit.md) | Records the gap between scaffold and reality |

## Initial Audit Findings

### MCP-derived findings

- The scaffold exists and is usable.
- `listToFill` remains conservative and still points at already-populated files.
- The automatic codebase map is materially incomplete for this repository.
- Generic skill and agent suggestions should not be accepted blindly.

### Repository-derived findings

- The repository is not just a CLI project.
- `raccoon-cli` is a first-class subsystem.
- Runtime topology and contract alignment matter more than generic product docs.
- A valid `.context` here must preserve operational truth about the cluster and validation pipeline.

## Working Phases

### Phase 1 — Audit the scaffold
> **Primary Agent:** `documentation-writer`

**Objective:** identify where MCP output matches the repository and where it drifts into generic scaffolding.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | Compare `context.getMap` output with the real repository layout | `documentation-writer` | pending | Gap list between MCP map and repo reality |
| 1.2 | Classify current docs, agents, and skills as keep, defer, or replace | `documentation-writer` | pending | Prioritized inventory |
| 1.3 | Record the audit in `.context/docs/context-audit.md` | `documentation-writer` | pending | Audit artifact |

### Phase 2 — Prioritize the first fill set
> **Primary Agent:** `feature-developer`

**Objective:** define the minimum next fill cycle that improves operational usefulness without expanding scope too early.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | Prioritize missing docs for runtime topology, architecture layers, and `raccoon-cli` role | `feature-developer` | pending | Ordered doc backlog |
| 2.2 | Prioritize skills tied to real workflows | `feature-developer` | pending | Ordered skill backlog |
| 2.3 | Identify whether new repo-specific agents are needed | `feature-developer` | pending | Agent proposal |

### Phase 3 — Validate adherence criteria
> **Primary Agent:** `code-reviewer`

**Objective:** verify that the proposed plan is grounded in repository evidence and does not drift into ornamental context.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | Confirm every proposed artifact maps to a real repo concern | `code-reviewer` | pending | Adherence check |
| 3.2 | Reject generic-first items that do not support runtime, architecture, or tooling | `code-reviewer` | pending | Scope discipline |
| 3.3 | Publish the first-fill order | `code-reviewer` | pending | Final prioritized plan |

## First Fill Order

1. Add operational docs:
   - `runtime-topology.md`
   - `architecture-layers.md`
   - `raccoon-cli-role.md`
2. Fill priority skills:
   - `bug-investigation`
   - `code-review`
   - `documentation`
   - `refactoring`
   - `test-generation`
3. Review whether new repo-specific agents should be added before deepening low-priority generic agents.

## Validation Criteria

- The audit must cite MCP findings and repository evidence.
- The first-fill order must preserve cluster/runtime truth, Go layering, and the Rust tooling role.
- The plan must explicitly defer overly generic skills and docs.
- No proposed artifact should assume a frontend, mobile app, or database-centric architecture that the repository does not have.

## Rollback

- If a proposed context artifact proves generic or inaccurate, remove it from the first-fill set rather than forcing it into the repository.
- Prefer a smaller, correct `.context` over a larger, generic one.

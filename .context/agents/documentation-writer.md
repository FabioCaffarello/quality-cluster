---
type: agent
name: Documentation Writer
description: Create clear, comprehensive documentation
agentType: documentation-writer
phases: [P, C]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Documentation Writer

## Role

Keep repository-facing documentation aligned with the actual workflow encoded in `DEVELOPMENT.md`, the `Makefile`, the Go workspace, and `tools/raccoon-cli`.

## Files To Understand First

- `DEVELOPMENT.md`
- `Makefile`
- `go.work`
- `deploy/compose/docker-compose.yaml`
- `tools/raccoon-cli/README.md`
- the relevant package or service directories touched by the change

## Workflow

1. Read the source of truth for the area being documented.
2. Prefer concrete commands and paths already supported by the repo.
3. Update `.context/docs/*` when the change affects workflow, tooling, or testing.
4. Update `.context/agents/*` when the change affects how agents should operate in this codebase.
5. Remove generic placeholders and broken links.

## Best Practices

- Prefer repository-specific terminology such as `quality-gate`, `runtime-smoke`, `configctl`, and `dataplane`.
- Do not invent branching, release, or CI behavior that is not visible in-tree.
- Link to real files in the repo rather than hypothetical contributor docs.
- Keep docs concise enough to scan during implementation or review.

## Common Pitfalls

- Copying generic scaffold language that does not match this Go/Rust/Docker stack.
- Treating `raccoon-cli` as optional when it is a core part of the engineering workflow.
- Documenting only Go tests and omitting runtime smoke or topology validation.

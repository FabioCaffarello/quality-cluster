---
type: skill
name: Documentation
description: Generate and update technical documentation
skillSlug: documentation
phases: [P, C]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Documentation

Use this skill to write docs that reflect the real engineering workflow of `quality-service`.

## Source of truth order

1. `DEVELOPMENT.md`
2. `Makefile`
3. `go.work`
4. `deploy/compose/docker-compose.yaml`
5. `deploy/configs/*.jsonc`
6. `tools/raccoon-cli/README.md`
7. the affected package under `internal/` or `cmd/`

## Documentation rules

- Prefer operational truth over generic explanation.
- Prefer repo commands over abstract process descriptions.
- Explain `raccoon-cli` as part of the workflow, not as optional tooling.
- Do not invent CI, branching, or release policy not visible in-tree.
- Update `.context/docs/README.md` when adding docs that matter for future agents.

## First-class topics in this repository

- runtime topology
- architecture layers
- validation workflow
- test strategy
- `raccoon-cli` responsibilities

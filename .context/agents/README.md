# Agent Handbook

This directory contains ready-to-customize playbooks for AI agents collaborating on the repository.

## Relation To Root AGENTS

- [`../../AGENTS.md`](../../AGENTS.md) is the repository-wide operating contract.
- `.context/agents/*.md` specializes that contract into task playbooks.
- When a playbook and the root file overlap, keep repository rules from `AGENTS.md` and runtime truth from `.context/docs/` as the baseline.

## Available Agents
- [Cluster Architect](./cluster-architect.md) — Decide service ownership, cluster topology, and runtime boundaries
- [Runtime Validator](./runtime-validator.md) — Prove real cluster behavior through smoke, readiness, and troubleshooting flows
- [CLI Evolution](./cli-evolution.md) — Evolve `raccoon-cli` as the repository quality control plane
- [Contract Guardian](./contract-guardian.md) — Protect message, binding, and query contracts across the cluster
- [TDD Coordinator](./tdd-coordinator.md) — Choose the validation ladder before implementation starts
- [Code Reviewer](./code-reviewer.md) — Review code changes for quality, style, and best practices
- [Bug Fixer](./bug-fixer.md) — Analyze bug reports and error messages
- [Runtime Topology Auditor](./runtime-topology-auditor.md) — Audit cluster wiring, compose profiles, and runtime data flow
- [Contract Auditor](./contract-auditor.md) — Audit control, event, and dataplane contracts
- [Raccoon CLI Maintainer](./raccoon-cli-maintainer.md) — Maintain the Rust quality platform and analyzer contracts
- [Feature Developer](./feature-developer.md) — Implement new features according to specifications
- [Refactoring Specialist](./refactoring-specialist.md) — Identify code smells and improvement opportunities
- [Test Writer](./test-writer.md) — Write comprehensive unit and integration tests
- [Documentation Writer](./documentation-writer.md) — Create clear, comprehensive documentation
- [Performance Optimizer](./performance-optimizer.md) — Identify performance bottlenecks

## Primary Daily Agents

- `cluster-architect`
- `runtime-validator`
- `cli-evolution`
- `contract-guardian`
- `tdd-coordinator`

Use these first for cluster, runtime, contract, `raccoon-cli`, and validation-planning work.

## Secondary Agents

- `runtime-topology-auditor`
  - narrow audit helper for topology reviews; do not use as a substitute for `cluster-architect`
- `contract-auditor`
  - narrow audit helper for transport review; do not use as a substitute for `contract-guardian`
- `raccoon-cli-maintainer`
  - maintenance-focused helper; prefer `cli-evolution` for day-to-day CLI changes
- generic development agents
  - use only when the task is not primarily about cluster behavior, contracts, or repository guard rails

## How To Use These Playbooks
1. Pick the agent that matches your task.
2. Read the matching source-of-truth files first (`DEVELOPMENT.md`, `Makefile`, `go.work`, and the affected package).
3. Prefer repo-specific agents when the task touches cluster topology, contracts, or `tools/raccoon-cli`.
4. Share the final prompt with your AI assistant.
5. Capture learnings in the relevant documentation file so future runs improve.

## Overlap Policy
- `cluster-architect` decides where behavior belongs across services and profiles.
- `runtime-validator` proves the cluster actually behaves correctly after a change.
- `cli-evolution` owns `tools/raccoon-cli` checks and operator-facing diagnostics.
- `contract-guardian` owns message, binding, and query compatibility.
- `tdd-coordinator` chooses the cheapest reliable validation sequence before coding.
- `runtime-topology-auditor`, `contract-auditor`, and `raccoon-cli-maintainer` remain audit or maintenance helpers, not primary daily-role agents.

## Related Resources
- [Documentation Index](../docs/README.md)
- [Context Maintenance Plan](../workflow/plan.md)
- [Agent Knowledge Base](../../AGENTS.md)
- [Development Workflow](../../DEVELOPMENT.md)

# Skills

On-demand expertise for AI agents. Skills are task-specific procedures that get activated when relevant.

> Project: quality-service

## How Skills Work

1. **Discovery**: AI agents discover available skills
2. **Matching**: When a task matches a skill's description, it's activated
3. **Execution**: The skill's instructions guide the AI's behavior

## Available Skills

### Repository Skills

| Skill | Description | Phases |
|-------|-------------|--------|
| [Cluster Debugging](./cluster-debugging/SKILL.md) | Debug multi-service runtime failures across Compose, NATS, Kafka, and validator result flow | E, V |
| [Contract Audit](./contract-audit/SKILL.md) | Audit transport, binding, payload, and query contracts across the cluster | R, E, V |
| [Scenario Design](./scenario-design/SKILL.md) | Choose the smallest scenario-smoke path that proves a runtime claim | P, E, V |
| [CLI Quality Gate](./cli-quality-gate/SKILL.md) | Apply the repository's canonical raccoon-cli and Make validation ladder | E, V |
| [Semantic Drift Review](./semantic-drift-review/SKILL.md) | Review whether code, configs, docs, and analyzers still describe the same system | R, V |
| [Runtime Validation](./runtime-validation/SKILL.md) | Prove runtime-significant changes through static gate, live smoke, and result evidence | E, V |

### Built-in Skills

| Skill | Description | Phases |
|-------|-------------|--------|
| [Commit Message](./commit-message/SKILL.md) | Generate commit messages following conventional commits with scope detection | E, C |
| [Pr Review](./pr-review/SKILL.md) | Review pull requests against team standards and best practices | R, V |
| [Code Review](./code-review/SKILL.md) | Review code quality, patterns, and best practices | R, V |
| [Test Generation](./test-generation/SKILL.md) | Generate comprehensive test cases for code | E, V |
| [Documentation](./documentation/SKILL.md) | Generate and update technical documentation | P, C |
| [Refactoring](./refactoring/SKILL.md) | Safe code refactoring with step-by-step approach | E |
| [Bug Investigation](./bug-investigation/SKILL.md) | Systematic bug investigation and root cause analysis | E, V |
| [Feature Breakdown](./feature-breakdown/SKILL.md) | Break down features into implementable tasks | P |
| [Api Design](./api-design/SKILL.md) | Design RESTful APIs following best practices | P, R |
| [Security Audit](./security-audit/SKILL.md) | Security review checklist for code and infrastructure | R, V |

## Fill Policy For This Repository

Not every generated skill should be filled immediately. In `quality-service`, a skill should only be filled when it has immediate operational use in the current work.

### Filled now because they have direct repository use

- `cluster-debugging`
- `contract-audit`
- `scenario-design`
- `cli-quality-gate`
- `semantic-drift-review`
- `runtime-validation`
- `bug-investigation`
- `code-review`
- `documentation`
- `refactoring`
- `test-generation`

### Deferred until a concrete task requires them

- `security-audit`
  - Fill when a task explicitly focuses on threat surface, hardening, auth, network exposure, or secrets/config risk.
- `feature-breakdown`
  - Fill when a real feature requires multi-step planning across runtime, contracts, and validation.
- `api-design`
  - Fill only when an actual HTTP or contract-facing API redesign is on the table.
- `commit-message`
  - Fill only when commit generation becomes part of the working loop.
- `pr-review`
  - Fill only when there is a real review or PR workflow to encode from repository practice.

The default rule is: prefer a smaller, accurate skill set over a larger generic one.

## Alignment Rules For This Repository

- Prefer repository skills when a task touches cluster behavior, contracts, smoke scenarios, drift, or `raccoon-cli`.
- Use built-in skills as secondary helpers, not as the primary source of runtime truth.
- When MCP-generated maps conflict with runtime code, configs, or canonical docs, prefer the repository truth and then update the context.

## Cross-Reference Pattern

- Use `.context/docs/cluster-quality.md` for the validation ladder and escalation rules.
- Use `.context/docs/messaging-contracts.md` for contract surfaces and scope defaults.
- Use `.context/docs/tooling-raccoon-cli.md` for command families and tool boundaries.
- Use `.context/workflow/plan.md` and `.context/workflow/status.yaml` for continuous maintenance triggers and handoff state.
- Use `.context/agents/runtime-validator.md`, `.context/agents/contract-guardian.md`, and `.context/agents/tdd-coordinator.md` when a skill needs a task owner.

## Creating Custom Skills

Create a new skill by adding a directory with a `SKILL.md` file:

```
.context/skills/
└── my-skill/
    ├── SKILL.md          # Required: skill definition
    └── templates/        # Optional: helper resources
        └── checklist.md
```

### SKILL.md Format

```yaml
---
name: my-skill
description: When to use this skill
phases: [P, E, V]  # Optional: PREVC phases
mode: false        # Optional: mode command?
---

# My Skill

## When to Use
[Description of when this skill applies]

## Instructions
1. Step one
2. Step two

## Examples
[Usage examples]
```

## PREVC Phase Mapping

| Phase | Name | Skills |
|-------|------|--------|
| P | Planning | feature-breakdown, documentation, api-design |
| R | Review | pr-review, code-review, api-design, security-audit |
| E | Execution | commit-message, test-generation, refactoring, bug-investigation |
| V | Validation | pr-review, code-review, test-generation, security-audit |
| C | Confirmation | commit-message, documentation |

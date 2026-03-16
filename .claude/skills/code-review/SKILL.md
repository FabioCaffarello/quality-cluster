---
name: Code Review
description: Review code quality, patterns, and best practices
phases: [R, V]
---

# Code Review

Use this skill for repository-aware review, not generic style feedback.

## Review priorities

1. Behavior regressions
2. runtime or contract drift
3. architecture boundary violations
4. missing tests or missing verification evidence
5. only then smaller maintainability concerns

## Review checklist for this repository

- Did the change preserve the `internal/domain -> application -> adapters -> actors/interfaces` boundary rules?
- Did any deploy or config change get validated with `make check` at minimum?
- Did runtime-significant changes get deeper proof with `make check-deep` or `scenario-smoke`?
- If `tools/raccoon-cli` changed, was `make raccoon-test` considered?
- Do new tests live near the changed package?

## High-signal findings

Prefer findings about:

- incorrect service dependency assumptions,
- drift between compose/config/source,
- subject or envelope mismatches,
- leaked infrastructure types in domain/application layers,
- unproven runtime changes,
- output contract changes in `raccoon-cli`.
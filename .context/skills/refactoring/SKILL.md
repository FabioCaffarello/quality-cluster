---
type: skill
name: Refactoring
description: Safe code refactoring with step-by-step approach
skillSlug: refactoring
phases: [E]
generated: 2026-03-16
status: filled
scaffoldVersion: "2.0.0"
---

# Refactoring

Use this skill when improving structure without changing intended behavior.

## Repository-specific rules

- preserve layer boundaries enforced by `arch-guard`
- keep transport details inside adapters
- keep domain and application contracts clean
- preserve stable `raccoon-cli` output behavior when touching Rust tooling

## Safe sequence

1. Run `make check` before refactoring.
2. Make one structural change at a time.
3. Keep or improve nearby tests.
4. Run `make verify`.
5. If runtime wiring moved, run `make check-deep` or `scenario-smoke`.

## Common refactor targets

- duplicated lifecycle or mapper logic in `internal/application`
- repeated wiring across `cmd/*`
- duplicated registry or gateway patterns in `internal/adapters/nats`
- repeated analyzer logic in `tools/raccoon-cli`

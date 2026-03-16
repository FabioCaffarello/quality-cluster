---
name: Semantic Drift Review
description: Review whether source, configs, docs, Compose topology, and raccoon-cli analyzers still describe the same system after a change.
phases: [R, V]
---

# Semantic Drift Review

## When to use

Use this skill when a change risks making the repository internally inconsistent even if tests still pass.

## Input signals

- edits across code and `deploy/configs` or `deploy/compose`
- changes to docs, analyzer expectations, or service ownership
- refactors that move logic across packages or binaries
- `drift-detect`, `arch-guard`, or `doctor` warnings

## Canonical steps

1. Compare the changed code path with the docs and configuration that claim to describe it.
2. Check whether service ownership, contract surfaces, and startup assumptions still align.
3. Run architecture and drift analyzers before broad review conclusions.
4. If analyzer output is weak or stale, confirm against the code and update the source of truth.
5. Record the specific mismatch that was fixed or intentionally accepted.

## Relevant commands

- `make drift-detect`
- `make arch-guard`
- `make check`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- doctor`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- symbol-trace --symbol <name>`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- snapshot`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- snapshot-diff`

## Common risks

- trusting generated maps or stale docs over the code that actually runs
- updating code without updating the analyzer or docs that define repository truth
- collapsing architecture review into style review and missing boundary erosion
- accepting drift because the live cluster still happens to boot

## Acceptance criteria

- code, configs, docs, and analyzer assumptions tell the same story
- drift findings are either fixed or explicitly justified
- architecture boundaries still match the documented runtime topology
- reviewers can point to the command output that supports the conclusion
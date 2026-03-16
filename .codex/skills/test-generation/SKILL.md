---
name: Test Generation
description: Generate comprehensive test cases for code
phases: [E, V]
---

# Test Generation

Use this skill to add tests that match the blast radius of the change.

## Test layers in this repository

- Go package tests in `_test.go`
- HTTP handler, route, and webserver tests
- Rust tests under `tools/raccoon-cli/tests`
- runtime validation via `make check-deep` and `scenario-smoke`

## Placement rules

- keep tests in the same package directory as the changed code
- prefer package-level behavior tests over isolated mocks when the workflow spans multiple steps
- if the change is in `raccoon-cli`, add Rust tests instead of only changing docs

## Verification ladder

1. `make test` or `make test MODULE=...`
2. `make verify`
3. `make check` for config/topology/contract-sensitive work
4. `make check-deep` or `make scenario-smoke` for runtime-significant work
5. `make raccoon-test` for Rust tooling changes
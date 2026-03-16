---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-10-refresh-degradation-diagnostics"
phase: "phase-3"
---

# Block 10 Phase 3 Validation

## Validation Goal

Provar que o `trace-pack` agora diferencia modos de degradação do refresh do dataplane sem depender de interpretação manual de `jsz.json`.

## Evidence

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml trace_pack`
- `make check`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- trace-pack --output-dir /tmp/quality-service-traces`

## Result

Todos os checks acima passaram.

O `trace-pack` gerado em [`/tmp/quality-service-traces/trace-pack-20260316-162048`](/tmp/quality-service-traces/trace-pack-20260316-162048) agora inclui no `SUMMARY.md`:

- `refresh mode` como detalhamento explícito do ramo degradado
- counters e progresso de `delivered` e `ack_floor` para os durables de refresh
- `diagnosis` e `next step` específicos para cada modo

Nesta execução real o cluster estava indisponível, e o `trace-pack` classificou corretamente o estado como `refresh mode: telemetry-unavailable`, em vez de tratar a ausência de monitor state como lag genérico.

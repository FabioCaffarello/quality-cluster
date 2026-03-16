---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-8-refresh-sla-observability"
phase: "phase-3"
---

# Block 8 Phase 3 Validation

## Validation Goal

Provar que o `raccoon-cli` agora expõe sinais reutilizáveis da saúde do refresh do dataplane sem depender de interpretação manual de config e JetStream.

## Evidence

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml topology`
- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml trace_pack`
- `make raccoon-test`
- `make check`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- trace-pack --output-dir /tmp/quality-service-traces`

## Result

Todos os checks acima passaram.

O `trace-pack` gerado em [`/tmp/quality-service-traces/trace-pack-20260316-160003`](/tmp/quality-service-traces/trace-pack-20260316-160003) agora inclui no `SUMMARY.md`:

- `bootstrap.reconcile_interval` de `consumer` e `emulator`
- counters de `pending`, `ack_pending` e `redelivered` para `consumer-runtime-refresh-v1`
- counters equivalentes para `emulator-runtime-refresh-v1`

Isso fecha a prova mínima do bloco: o motor de qualidade passou a expor o SLA observável do refresh, não apenas sua semântica estática.

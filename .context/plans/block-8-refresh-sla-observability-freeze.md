---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-8-refresh-sla-observability"
phase: "phase-1"
---

# Block 8 Refresh SLA Observability Freeze

## Canonical Gap

Depois do Bloco 7, o runtime já se autocura, mas o operador ainda precisava combinar `deploy/configs/*` com `nats/jsz.json` para entender cadence e lag do refresh.

## Frozen Decisions

- `bootstrap.reconcile_interval` passa a ser parte do contrato congelado de `consumer` e `emulator`.
- o resumo de refresh do `trace-pack` usa `CONFIGCTL_EVENTS` e os durables `consumer-runtime-refresh-v1` e `emulator-runtime-refresh-v1`.
- o bloco é exclusivamente de observabilidade do motor de qualidade; não muda a semântica do runtime.

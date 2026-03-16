---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-10-refresh-degradation-diagnostics"
phase: "phase-1"
---

# Block 10 Refresh Degradation Diagnostics Freeze

## Canonical Gap

Depois do Bloco 9, o operador já via `healthy|degraded`, mas ainda precisava interpretar sozinho se a degradação era ausência de telemetria, lag transitório ou refresh realmente preso.

## Frozen Decisions

- `refresh status` continua binário: `healthy` ou `degraded`.
- o `trace-pack` passa a expor `refresh mode` para detalhar o ramo degradado.
- os modos canônicos deste bloco são `telemetry-unavailable`, `durable-missing`, `cadence-mismatch`, `transient-lag`, `stalled-refresh` e `redelivery-detected`.
- a heurística usa apenas `bootstrap.reconcile_interval` e dados já coletados de `nats/jsz.json`; não há endpoint novo nem dependência de logs para classificar o estado.

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cli-evolution"
    role: "Transformar refresh degradado em diagnóstico operacional mais específico no trace-pack"
  - type: "runtime-validator"
    role: "Validar a nova heurística contra estados reais e ausentes do cluster"
  - type: "contract-guardian"
    role: "Preservar os sinais canônicos usados para distinguir lag transitório de refresh parado"
  - type: "documentation-writer"
    role: "Consolidar os modos de degradação do refresh no contexto vivo"
docs:
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Degradation Modes"
    prevc: "P"
    agent: "cli-evolution"
  - id: "phase-2"
    name: "Implement Diagnostic Modes"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Validate And Document"
    prevc: "V"
    agent: "documentation-writer"
---

# Block 10 - Refresh Degradation Diagnostics

> Endurecer o `trace-pack` para que `refresh status: degraded` deixe de ser um único balde e passe a distinguir indisponibilidade de telemetria, mismatch de cadence, lag transitório, refresh parado e redelivery relevante.

## Task Snapshot

- **Primary goal:** tornar o diagnóstico de refresh degradado mais específico sem criar endpoints novos nem sair dos sinais já canônicos do repositório.
- **Success signal:** `SUMMARY.md` expõe `refresh mode` e usa `delivered`, `ack_floor`, `last_active`, `ts` e `bootstrap.reconcile_interval` para diferenciar degradação em progresso de refresh possivelmente preso.
- **Out of scope:** mudar o runtime Go, adicionar alerting externo, ou promover novos estados além de `healthy|degraded`.

## Working Phases

### Phase 1 - Freeze Degradation Modes

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | congelar os modos de degradação canônicos do `trace-pack` | `cli-evolution` | completed | taxonomia operacional explícita |
| 1.2 | decidir quais campos de `jsz` entram na heurística | `contract-guardian` + `cli-evolution` | completed | `delivered`, `ack_floor`, `last_active`, `ts` e cadence |

### Phase 2 - Implement Diagnostic Modes

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | estender o parser de durables para carregar sinais de progresso | `cli-evolution` | completed | leitura mais rica de `CONFIGCTL_EVENTS` |
| 2.2 | distinguir `telemetry-unavailable`, `cadence-mismatch`, `transient-lag`, `stalled-refresh` e `redelivery-detected` | `cli-evolution` | completed | `refresh mode` no `SUMMARY.md` |
| 2.3 | ajustar `diagnosis` e `next step` por modo degradado | `cli-evolution` | completed | troubleshooting mais curto e específico |
| 2.4 | cobrir com testes do Rust | `cli-evolution` | completed | suíte `trace_pack` verde |

### Phase 3 - Validate And Document

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | validar com testes focados, `make check` e `trace-pack` real | `runtime-validator` | completed | prova operacional do CLI |
| 3.2 | atualizar docs canônicas | `documentation-writer` | completed | contexto vivo coerente |

## Done Definition

Este bloco só termina quando:

- o `trace-pack` mostra `refresh mode` além de `refresh status`;
- `diagnosis` e `next step` mudam conforme o tipo de degradação;
- a heurística continua usando apenas sinais já canônicos do repositório;
- o CLI continua verde na suíte focada e no guard rail rápido;
- o `.context` registra esses modos como parte do troubleshooting canônico.

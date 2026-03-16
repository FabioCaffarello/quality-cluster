---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cli-evolution"
    role: "Transformar refresh self-healing em sinal observável no raccoon-cli"
  - type: "runtime-validator"
    role: "Provar que as novas leituras do motor de qualidade refletem o cluster real"
  - type: "contract-guardian"
    role: "Garantir que durables e stream monitorados continuam sendo os canônicos"
  - type: "documentation-writer"
    role: "Consolidar a nova camada de observabilidade no contexto vivo"
docs:
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Refresh SLA Signals"
    prevc: "P"
    agent: "cli-evolution"
  - id: "phase-2"
    name: "Implement CLI Observability"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Validate And Document"
    prevc: "V"
    agent: "documentation-writer"
---

# Block 8 - Refresh SLA Observability

> Endurecer o `raccoon-cli` para que a saúde do refresh do dataplane deixe de depender de leitura manual de configs e `jsz`, e passe a aparecer como sinal consolidado e reutilizável no workflow normal do repositório.

## Task Snapshot

- **Primary goal:** tornar `bootstrap.reconcile_interval` e o estado dos durables de refresh parte explícita do motor de qualidade e do `trace-pack`.
- **Success signal:** `topology-doctor` falha ou avisa em drift relevante de `bootstrap.reconcile_interval`, e `trace-pack` resume cadence configurada e lag dos durables `consumer-runtime-refresh-v1` e `emulator-runtime-refresh-v1`.
- **Out of scope:** alterar o runtime Go, redefinir contratos NATS, ou mudar o intervalo de reconciliacao em produção.

## Working Phases

### Phase 1 - Freeze Refresh SLA Signals

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | congelar `bootstrap.reconcile_interval` como invariante de config do dataplane | `cli-evolution` | completed | seam operacional explícito |
| 1.2 | decidir a leitura mínima de JetStream para resumo de lag | `contract-guardian` + `cli-evolution` | completed | `CONFIGCTL_EVENTS` + durables de refresh |

### Phase 2 - Implement CLI Observability

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | estender parser/topology doctor para `bootstrap.reconcile_interval` | `cli-evolution` | completed | checks estáticos atualizados |
| 2.2 | resumir cadence e lag no `trace-pack` | `cli-evolution` | completed | `SUMMARY.md` com refresh observability |
| 2.3 | cobrir comportamento com testes do Rust | `cli-evolution` | completed | suíte focada verde |

### Phase 3 - Validate And Document

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | validar com `make raccoon-test`, `make check`, `topology-doctor` e `trace-pack` | `runtime-validator` | completed | prova operacional do CLI |
| 3.2 | atualizar docs canônicas | `documentation-writer` | completed | contexto vivo coerente |

## Done Definition

Este bloco só termina quando:

- `bootstrap.reconcile_interval` aparece como contrato congelado no `topology-doctor`;
- `trace-pack` mostra cadence e lag de refresh sem exigir leitura manual de `jsz`;
- o workflow padrão do `raccoon-cli` continua verde;
- o `.context` registra essa observabilidade como parte da verdade operacional do repositório.

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cli-evolution"
    role: "Transformar counters de refresh em diagnóstico operacional acionável no raccoon-cli"
  - type: "runtime-validator"
    role: "Validar que a classificação do trace-pack representa o estado real do cluster"
  - type: "contract-guardian"
    role: "Preservar os durables e sinais canônicos usados na classificação"
  - type: "documentation-writer"
    role: "Consolidar a nova semântica de saúde do refresh no contexto vivo"
docs:
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Refresh Health Contract"
    prevc: "P"
    agent: "cli-evolution"
  - id: "phase-2"
    name: "Implement Health Classification"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Validate And Document"
    prevc: "V"
    agent: "documentation-writer"
---

# Block 9 - Refresh Health Classification

> Endurecer o `trace-pack` para que a observabilidade de refresh do dataplane deixe de ser apenas leitura de cadence e counters, e passe a produzir um sinal operacional explícito de `healthy` ou `degraded`, com diagnóstico e próximo passo.

## Task Snapshot

- **Primary goal:** transformar o resumo de refresh do `trace-pack` em uma leitura acionável do estado dos durables e da cadence de reconciliação.
- **Success signal:** `SUMMARY.md` expõe `refresh status`, `diagnosis` e `next step` quando necessário, sem exigir que o operador interprete manualmente `jsz.json`.
- **Out of scope:** alterar a semântica do runtime Go, redefinir thresholds de SLA no cluster, ou criar alerting externo.

## Working Phases

### Phase 1 - Freeze Refresh Health Contract

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | congelar `healthy` vs `degraded` como a leitura canônica do `trace-pack` para refresh | `cli-evolution` | completed | contrato operacional explícito |
| 1.2 | decidir os sinais mínimos da classificação | `contract-guardian` + `cli-evolution` | completed | cadence alinhada + durables visíveis e sem lag |

### Phase 2 - Implement Health Classification

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | classificar refresh como `healthy` ou `degraded` a partir de cadence e counters | `cli-evolution` | completed | `trace-pack` com leitura consolidada |
| 2.2 | adicionar `diagnosis` e `next step` quando houver degradação | `cli-evolution` | completed | troubleshooting guiado no `SUMMARY.md` |
| 2.3 | cobrir comportamento com testes focados do Rust | `cli-evolution` | completed | suíte `trace_pack` verde |

### Phase 3 - Validate And Document

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | validar com testes do Rust, `make check` e `trace-pack` real | `runtime-validator` | completed | prova operacional do CLI |
| 3.2 | atualizar docs canônicas | `documentation-writer` | completed | contexto vivo coerente |

## Done Definition

Este bloco só termina quando:

- o `trace-pack` expõe `refresh status` como leitura primária do refresh do dataplane;
- `diagnosis` e `next step` aparecem quando o estado é degradado;
- os sinais usados na classificação continuam ancorados em `bootstrap.reconcile_interval` e nos durables de refresh em `CONFIGCTL_EVENTS`;
- o workflow padrão do `raccoon-cli` continua verde;
- o `.context` registra a classificação como parte da verdade operacional do repositório.

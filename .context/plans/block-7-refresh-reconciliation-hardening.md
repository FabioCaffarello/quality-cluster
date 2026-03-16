---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Congelar a semantica do fallback de reconciliacao sem rebaixar o refresh por evento"
  - type: "runtime-validator"
    role: "Provar a convergencia do dataplane com e sem o gatilho imediato de runtime-change"
  - type: "cli-evolution"
    role: "Manter o raccoon-cli e os guard rails coerentes com o novo seam operacional"
  - type: "tdd-coordinator"
    role: "Escolher a escada minima de testes e smoke para o bloco"
  - type: "documentation-writer"
    role: "Consolidar a nova verdade operacional no contexto vivo"
docs:
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Reconciliation Seam"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Implement Self-Healing Fallback"
    prevc: "E"
    agent: "runtime-validator"
  - id: "phase-3"
    name: "Validate And Update Workflow"
    prevc: "V"
    agent: "documentation-writer"
---

# Block 7 - Refresh Reconciliation Hardening Plan

> Endurecer a convergencia do dataplane para que `consumer` e `emulator` se recuperem de eventos `config.ingestion_runtime_changed` perdidos ou atrasados sem abandonar o modelo event-driven como caminho primario.

## Task Snapshot

- **Primary goal:** introduzir um fallback de reconciliacao leve, explicito e compartilhado no seam de bootstrap agregado, de modo que o runtime nao fique indefinidamente stale quando o gatilho por evento falhar no processo.
- **Success signal:** `consumer` e `emulator` continuam reagindo a `config.ingestion_runtime_changed`, mas tambem voltam a convergir sozinhos via `bootstrap.reconcile_interval`; `make verify`, `happy-path` e `check-deep` permanecem verdes.
- **Out of scope:** trocar de JetStream para outro transporte, reintroduzir polling pesado por scope, paralelizar smoke, ou remodelar contratos HTTP/NATS.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 5 Event Driven Dataplane Refresh](./block-5-event-driven-dataplane-refresh.md)
  - [Block 6 Event Refresh Diagnostics Hardening](./block-6-event-refresh-diagnostics-hardening.md)

## Codebase Context

### Current Diagnostic

- o `consumer` hoje carrega bootstrap agregado no startup e depois depende do consumer JetStream em [`internal/actors/scopes/consumer/bootstrap_actor.go`](../../internal/actors/scopes/consumer/bootstrap_actor.go).
- o `emulator` segue o mesmo desenho: bootstrap agregado inicial, consumer dedicado de runtime-change e refresh sob demanda em [`cmd/emulator/run.go`](../../cmd/emulator/run.go).
- ambos preservam o bootstrap agregado como fonte de verdade do estado efetivo, mas nao possuem mecanismo de auto-cura se o evento for perdido localmente depois do startup.
- o Bloco 6 fechou diagnostico e prova; o gap remanescente agora e de resiliencia, nao de observabilidade.

### Preserve These Decisions

- `config.ingestion_runtime_changed` continua sendo o gatilho primario e imediato do refresh do dataplane.
- `/runtime/ingestion/bindings` continua sendo a fonte de verdade do conjunto efetivo de bindings.
- a assinatura do bootstrap agregado continua sendo o guard rail para evitar reload desnecessario.
- `raccoon-cli` continua provando a saude do runtime por smoke e quality-gate, nao dirigindo o refresh em si.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | congelar a semantica do fallback e sua fronteira com o refresh por evento | [cluster-architect](../agents/cluster-architect.md) | definir que reconciliacao e fallback bounded, nao novo primario |
| Runtime Validator | implementar e provar a recuperacao do dataplane | [runtime-validator](../agents/runtime-validator.md) | `consumer`, `emulator`, runtime proof |
| CLI Evolution | manter guard rails coerentes com a nova verdade operacional | [cli-evolution](../agents/cli-evolution.md) | verificar se docs e comandos do `raccoon-cli` continuam descrevendo o fluxo real |
| TDD Coordinator | fechar a ladder certa para o corte | [tdd-coordinator](../agents/tdd-coordinator.md) | testes focados + `verify` + smoke/deep |
| Documentation Writer | atualizar o contexto vivo do repositorio | [documentation-writer](../agents/documentation-writer.md) | docs canônicas e artefato de validacao |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Runtime Topology | [architecture-runtime.md](../docs/architecture-runtime.md) | registrar que o dataplane agora combina gatilho por evento com reconciliacao periodica bounded |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | deixar claro quando `bootstrap.reconcile_interval` entra como fallback e como validar isso |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar a narrativa operacional do motor de qualidade com o novo seam |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| fallback virar polling primario e mascarar falha de evento | Medium | High | reconciliacao com intervalo explicito, leve e documentado como secondary path | `cluster-architect` |
| reload periodico reintroduzir churn desnecessario no dataplane | Medium | Medium | preservar assinatura do bootstrap agregado antes de reiniciar runtime ou atualizar topics | `runtime-validator` |
| docs e tooling continuarem contando uma historia de refresh puramente por evento | Low | Medium | atualizar docs canônicas e artefato do bloco no mesmo change set | `documentation-writer` |

### Dependencies

- **Internal:** `internal/shared/settings`, `internal/actors/scopes/consumer`, `cmd/emulator`, `internal/application/runtimebootstrap`
- **External:** cluster local com NATS/Kafka para smoke e deep gate
- **Technical:** o bootstrap agregado e a assinatura do binding set precisam permanecer estaveis

### Assumptions

- perder um evento de runtime-change continua sendo um risco operacional plausivel mesmo com durables dedicados.
- o menor corte seguro e um fallback bounded no bootstrap compartilhado, nao uma nova camada de reconciliacao distribuida.

## Working Phases

### Phase 1 - Freeze Reconciliation Seam
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar a decisao arquitetural do bloco antes de tocar runtime.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | confirmar o gap real entre refresh por evento e auto-cura por bootstrap | `cluster-architect` | completed | diagnostico do seam |
| 1.2 | decidir onde o fallback vive e como e configurado | `cluster-architect` + `runtime-validator` | completed | `bootstrap.reconcile_interval` como seam compartilhado |

### Phase 2 - Implement Self-Healing Fallback
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** adicionar reconciliacao periodica bounded ao dataplane sem rebaixar o gatilho por evento.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | estender config compartilhada de bootstrap com intervalo de reconciliacao | `runtime-validator` | completed | schema e defaults coerentes |
| 2.2 | aplicar fallback de reconciliacao no `consumer` | `runtime-validator` | completed | bootstrap actor com auto-cura bounded |
| 2.3 | aplicar fallback de reconciliacao no `emulator` | `runtime-validator` | completed | loop de publicacao com refresh bounded |
| 2.4 | cobrir o seam com testes focados | `tdd-coordinator` | completed | testes de settings/runtime |

### Phase 3 - Validate And Update Workflow
> **Primary Agent:** `documentation-writer` - [Playbook](../agents/documentation-writer.md)

**Objective:** provar o bloco no workflow real e consolidar a nova verdade operacional.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | validar com testes focados e `make verify` | `tdd-coordinator` | completed | baseline verde |
| 3.2 | provar comportamento no cluster com `happy-path` e `check-deep` | `runtime-validator` | completed | evidência operacional |
| 3.3 | atualizar docs e artefato de validacao do bloco | `documentation-writer` | completed | docs canônicas + validation artifact |

## Validation Ladder

**Before**

- `raccoon-cli recommend internal/shared/settings internal/actors/scopes/consumer cmd/emulator`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`

**During**

- `go test ./internal/shared/settings ./internal/actors/scopes/consumer ./cmd/emulator`
- `make verify`

**After**

- `make scenario-smoke SCENARIO=happy-path`
- `make check-deep`

## Done Definition

Este bloco so termina quando:

- `consumer` e `emulator` conseguem reconciliar bootstrap agregado sem depender exclusivamente do evento;
- o caminho por evento continua sendo o gatilho primario e mais rapido;
- a assinatura do bootstrap segue evitando reload desnecessario;
- `verify`, `happy-path` e `check-deep` continuam verdes;
- o `.context` registra a nova semantica como parte da verdade operacional do repositorio.

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Definir a troca de polling por refresh orientado a eventos sem duplicar ownership entre configctl e dataplane"
  - type: "contract-guardian"
    role: "Proteger o uso canônico de config.ingestion_runtime_changed e evitar drift entre evento, registry e runtimebootstrap"
  - type: "cli-evolution"
    role: "Manter o raccoon-cli como prova principal de que o refresh dirigido a eventos não quebrou o runtime"
  - type: "runtime-validator"
    role: "Provar no cluster real que consumer e emulator reagem ao sinal de refresh sem regressão de smoke"
  - type: "tdd-coordinator"
    role: "Fechar a escada de testes e smoke antes de mexer em consumers NATS e wiring cross-layer"
  - type: "documentation-writer"
    role: "Atualizar docs e planos com o novo mecanismo canônico de refresh"
docs:
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Event Signal & Runtime Boundaries"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Event Driven Refresh Implementation"
    prevc: "E"
    agent: "cluster-architect"
  - id: "phase-3"
    name: "Runtime Proof & Canonical Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Block 5 - Event Driven Dataplane Refresh Plan

> Trocar o refresh contínuo por polling do dataplane por reconciliação orientada ao evento `config.ingestion_runtime_changed`, preservando o bootstrap agregado como seam de verdade e o `raccoon-cli` como motor de prova operacional.

## Task Snapshot

- **Primary goal:** fazer `consumer` e `emulator` reagirem ao evento canônico de mudança de runtime de ingestão publicado por `configctl`, removendo a dependência operacional do polling periódico para refresh.
- **Success signal:** o dataplane continua convergindo para o conjunto ativo de bindings, mas agora dirigido por evento explícito de runtime-change; `runtime-bindings`, `verify`, `check-deep` e `scenario-smoke` permanecem verdes.
- **Out of scope:** troca da API HTTP pública, redesign do `runtimebootstrap` agregado, mudança do ownership do `validator`, paralelismo irrestrito de smoke ou remoção do seam de bootstrap por scope para troubleshooting.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 4 Dataplane Multiscope Hardening](./block-4-dataplane-multiscope-hardening.md)
  - [Block 4 Phase 3 Validation](./block-4-phase-3-validation.md)

## Codebase Context

### Current Diagnostic

- `configctl` já publica `config.ingestion_runtime_changed` e esse evento existe no domínio, no publisher e no registry NATS.
- o bloco 4 fechou a principal lacuna funcional com bootstrap agregado e refresh por assinatura, e o bloco 5 removeu o polling como mecanismo primário de refresh do dataplane.
- `validator` já continua dirigido a eventos `config.activated` e `config.deactivated`, então o runtime do repositório já aceita consumers JetStream como mecanismo canônico de coerência.
- `consumer` e `emulator` agora usam consumers dedicados de `config.ingestion_runtime_changed`, com durables distintos e reload sob demanda do bootstrap agregado.
- o evento `config.ingestion_runtime_changed` já carrega exatamente o tipo de mudança que interessa ao dataplane: mudança de runtime de ingestão por scope, com `change_type` e payload opcional de runtime.
- `deploy/configs/emulator.jsonc` e `deploy/compose/docker-compose.yaml` foram alinhados para refletir a dependência operacional explícita de NATS nesse refresh dirigido a evento.

### Preserve These Decisions

- `runtimebootstrap` agregado permanece como fonte para carregar o estado efetivo do dataplane; evento não substitui bootstrap, ele dispara refresh.
- `config.ingestion_runtime_changed` é o sinal canônico deste bloco; não duplicar semântica concorrente em `config.activated`/`config.deactivated` se o evento mais específico já existe.
- o `raccoon-cli` continua provando comportamento por smoke e checks estáticos, não participando do runtime.
- `global/default` segue sendo baseline simples de operador local, mas não regra exclusiva de convergência do dataplane.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | fechar o desenho de refresh orientado a evento | [cluster-architect](../agents/cluster-architect.md) | seam entre registry NATS, actors e loop do emulator |
| Contract Guardian | proteger o evento `config.ingestion_runtime_changed` como contrato único | [contract-guardian](../agents/contract-guardian.md) | evitar sobreposição com activation/deactivation |
| CLI Evolution | manter a prova operacional do bloco no raccoon-cli | [cli-evolution](../agents/cli-evolution.md) | `runtime-bindings`, `recommend`, smoke e deep gate |
| Runtime Validator | provar mudança no cluster real | [runtime-validator](../agents/runtime-validator.md) | `readiness-probe`, `happy-path`, `invalid-payload`, `check-deep` |
| TDD Coordinator | definir a escada mínima de testes | [tdd-coordinator](../agents/tdd-coordinator.md) | tests de consumer/adapters + smoke certo |
| Documentation Writer | consolidar a nova verdade operacional | [documentation-writer](../agents/documentation-writer.md) | docs canônicas, artefatos e tracking |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | registrar que o refresh do dataplane passa a ser disparado por evento, não por polling como modelo primário |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | atualizar a prova mínima para mudanças em runtime-change consumers |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | explicitar o papel operacional de `config.ingestion_runtime_changed` |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar o que o CLI prova quando o refresh do dataplane for orientado a eventos |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| criar mais um consumer de evento e duplicar semântica já coberta por polling | Medium | High | usar o evento como gatilho de reload, mantendo `runtimebootstrap` como fonte de estado | `cluster-architect` |
| alterar interfaces exportadas de durable consumer e quebrar implementors | Medium | High | evitar mudanças destrutivas em interfaces; preferir adicionar consumidor específico ou adapter fino | `contract-guardian` |
| refresh dirigido a evento perder mudanças por erro de durable/ack | Medium | High | cobrir registry, durable e comportamento de retry com testes focados | `tdd-coordinator` |
| o `emulator` ficar com dois caminhos de refresh competindo | Medium | Medium | congelar um único mecanismo primário e manter fallback só se explicitamente necessário | `cluster-architect` |

### Dependencies

- **Internal:** `internal/adapters/nats/durable_consumer.go`, `internal/adapters/nats/configctl_registry.go`, `internal/actors/scopes/consumer/*`, `cmd/emulator/run.go`, `internal/domain/configctl/events.go`
- **External:** JetStream saudável no cluster local para prova final
- **Technical:** preservar o bootstrap agregado do bloco 4 como fallback de verdade do estado, mesmo quando o gatilho vier por evento

### Assumptions

- o evento `config.ingestion_runtime_changed` é suficientemente específico para ser o gatilho único de refresh do dataplane.
- a troca do mecanismo de refresh pode ser feita sem reabrir a estrutura de topologia e contracts do bloco 4.
- se o wiring do `emulator` ficar grande demais, o bloco ainda deve ao menos remover o polling contínuo do `consumer` e deixar o `emulator` com uma ponte explícita, documentada e testável.

## Working Phases

### Phase 1 - Freeze Event Signal & Runtime Boundaries
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar o sinal canônico de refresh e os limites entre bootstrap, evento e reconciliação do dataplane.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | inventariar todos os sinais possíveis de refresh do dataplane | `cluster-architect` | completed | freeze de sinais e limites |
| 1.2 | escolher `config.ingestion_runtime_changed` como gatilho canônico ou rejeitá-lo com motivo explícito | `cluster-architect` + `contract-guardian` | completed | decisão registrada |
| 1.3 | fechar a escada de validação do bloco com `raccoon-cli` | `tdd-coordinator` + `cli-evolution` | completed | baseline de checks |

**Acceptance Criteria**

- o bloco não mistura polling, activation/deactivation e runtime-change sem critério
- o contrato canônico do refresh fica congelado antes da implementação

---

### Phase 2 - Event Driven Refresh Implementation
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** substituir o polling como mecanismo primário por refresh orientado a evento, sem quebrar o seam agregado de bootstrap.

**Front 1: NATS Contract Wiring**

- adicionar ou reutilizar consumer spec específico para `config.ingestion_runtime_changed`
- manter durables, ack policy e stream corretos para refresh do dataplane

**Front 2: Consumer Refresh Model**

- trocar o loop periódico do `bootstrap_actor` por gatilho sob demanda via evento
- manter proteção contra reload desnecessário quando a assinatura do binding set não mudou

**Front 3: Emulator Refresh Model**

- alinhar o `emulator` ao mesmo gatilho de refresh, sem criar dois loops concorrentes de reconciliação
- preservar publish loop simples para operador local

**Front 4: Safety And Tests**

- cobrir consumers/adapters, deduplicação por assinatura e comportamento de runtime generation
- manter bootstrap agregado como verdade do estado mesmo quando o trigger vier por evento

**Acceptance Criteria**

- polling deixa de ser o mecanismo primário de refresh do dataplane
- consumer e emulator reagem ao evento correto sem regressão de startup
- o runtime não faz reload desnecessário quando o binding set efetivo permanece igual

**Status:** completed

---

### Phase 3 - Runtime Proof & Canonical Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar a nova convergência dirigida a evento no cluster real e consolidar a mudança no `.context`.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | rodar testes focados e guard rails estáticos | `runtime-validator` + `tdd-coordinator` | completed | `go test`, `runtime-bindings`, `contract-audit`, `verify` |
| 3.2 | provar os cenários de runtime afetados | `runtime-validator` + `cli-evolution` | completed | `readiness-probe`, `happy-path`, `invalid-payload`, `check-deep` |
| 3.3 | atualizar docs e artefato de validação | `documentation-writer` | completed | docs canônicas + validation artifact |

**Acceptance Criteria**

- o refresh dirigido a evento fica provado no cluster real
- o `.context` e as docs deixam clara a troca de polling para evento
- o `raccoon-cli` continua sendo a prova canônica do bloco

**Status:** completed

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B5-P1-S1 | congelar o diagnóstico do refresh atual por polling | `cluster-architect` | nenhuma | freeze doc |
| B5-P1-S2 | decidir o uso canônico de `config.ingestion_runtime_changed` | `cluster-architect` + `contract-guardian` | `B5-P1-S1` | decisão registrada |
| B5-P1-S3 | fechar a ladder de validação com `raccoon-cli` | `tdd-coordinator` + `cli-evolution` | `B5-P1-S2` | baseline before/after |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B5-P2-S1 | adicionar wiring NATS do runtime-change para o dataplane | `feature-developer` + `contract-guardian` | `B5-P1-S2` | registry/consumer definidos |
| B5-P2-S2 | substituir o refresh primário do consumer por trigger de evento | `cluster-architect` | `B5-P2-S1` | actor e tests ajustados |
| B5-P2-S3 | alinhar o emulator ao mesmo mecanismo | `feature-developer` | `B5-P2-S1` | refresh sem polling primário |
| B5-P2-S4 | validar deduplicação por assinatura e comportamento de reload | `test-writer` + `tdd-coordinator` | `B5-P2-S2`, `B5-P2-S3` | testes focados |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B5-P3-S1 | rodar baseline estática e `verify` | `runtime-validator` | todos os steps da phase 2 | logs de verificação |
| B5-P3-S2 | rodar smoke e deep gate no cluster real | `runtime-validator` + `cli-evolution` | `B5-P3-S1` | smoke + deep gate |
| B5-P3-S3 | atualizar docs e tracking | `documentation-writer` | `B5-P3-S2` | docs + validation artifact |

## Validation Ladder

**Before**

- `raccoon-cli recommend internal/actors/scopes/consumer/bootstrap_actor.go internal/actors/scopes/consumer/supervisor.go cmd/emulator/run.go internal/adapters/nats/durable_consumer.go internal/adapters/nats/configctl_registry.go`
- `raccoon-cli arch-guard`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`

**During**

- `go test ./internal/adapters/nats ./internal/actors/scopes/consumer ./cmd/emulator`
- `make verify`

**After**

- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

## Done Definition

Este bloco só termina quando:

- o refresh primário do dataplane for disparado por `config.ingestion_runtime_changed`
- consumer e emulator continuarem convergindo para o binding set ativo sem reload desnecessário
- o polling anterior não permanecer como mecanismo principal escondido
- o `.context` e o `ai-context` refletirem o novo modelo operacional

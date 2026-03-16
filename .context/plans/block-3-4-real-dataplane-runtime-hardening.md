---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Fechar ownership entre emulator, consumer, validator, server e registry sem reabrir configctl"
  - type: "contract-guardian"
    role: "Proteger contracts Kafka, JetStream, bootstrap e ValidationResult ao longo do data plane"
  - type: "runtime-validator"
    role: "Provar o fluxo e2e local e a robustez operacional do runtime com smoke, inspect e trace"
  - type: "tdd-coordinator"
    role: "Definir a menor escada de validacao para changes cross-service e cross-layer"
  - type: "cli-evolution"
    role: "Manter raccoon-cli, smoke e diagnostics como motor de prova do plano"
  - type: "documentation-writer"
    role: "Atualizar docs canonicas e contexto vivo com a nova verdade operacional"
docs:
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze, Sequencing And Boundaries"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Block 3 Real Dataplane Foundation"
    prevc: "E"
    agent: "cluster-architect"
  - id: "phase-3"
    name: "Block 4 Runtime Ownership Hardening"
    prevc: "E"
    agent: "cluster-architect"
  - id: "phase-4"
    name: "Integrated Runtime Proof"
    prevc: "V"
    agent: "runtime-validator"
---

# Block 3-4 Real Dataplane And Runtime Hardening Plan

> Construir a primeira linha real do data plane do motor de qualidade e, em seguida, endurecer o runtime para que consumer e validator tenham ownership claro, wiring reduzido e prova e2e local confiavel.

## Planning Note

- Este plano usa a semantica nova dos blocos 3 e 4 descrita na conversa atual.
- O repositorio ja possui artefatos antigos chamados "Block 3" e "Block 4" com outro escopo (`smoke isolation` e `dataplane multiscope`).
- Para evitar colisao semantica, este documento cobre os dois blocos juntos em um plano novo, sem sobrescrever o historico anterior.

## Task Snapshot

- **Primary goal:** entregar um data plane minimo, real e observavel no Bloco 3, e endurecer consumer e validator no Bloco 4 com decomposicao por actors, registry forte e run.go fino.
- **Success signal:** o fluxo `emulator -> Kafka -> consumer -> JetStream -> validator -> ValidationResult -> server` roda localmente com bootstrap por bindings ativos, resultados pequenos e legiveis, e o runtime deixa de depender de wiring procedural espalhado.
- **Out of scope:** reabrir lifecycle do `configctl`, inflar a DSL, introduzir regras complexas cedo demais, persistencia nova fora do desenho atual, ou criar APIs paralelas no `server` so para compensar ownership difuso.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 2 Ingestion Runtime Contract](./block-2-ingestion-runtime-contract.md)
  - [Block 3 Contract And Flow Freeze](./artifacts/block-3-4-phase-1/block-3-contract-and-flow-freeze.md)
  - [Block 4 Runtime Ownership Matrix](./artifacts/block-3-4-phase-1/block-4-runtime-ownership-matrix.md)
  - [Block 3 Work Packages](./artifacts/block-3-4-phase-1/block-3-work-packages.md)
  - [Block 4 Work Packages](./artifacts/block-3-4-phase-1/block-4-work-packages.md)

## Desired Outcome By Block

### Block 3

- `emulator` publica carga sintetica controlada no Kafka por binding ativo.
- `consumer` consome do Kafka, normaliza para o contrato interno do dataplane e publica no JetStream.
- `validator` consome do JetStream, aplica regras minimas e grava `ValidationResult`.
- `server` continua como facade fina para leitura operacional de bindings ativos, runtime carregado e resultados.
- o bootstrap do dataplane depende de bindings ativos e nao de configuracao local implícita.

### Block 4

- `consumer` e `validator` ganham topologia de actors mais explicita, com ownership separado entre consume, route, work, store e query.
- registry passa a ser a fonte operacional forte para topics, subjects, streams, durables e pontos de roteamento.
- `run.go` perde logica procedural e fica limitado a startup, config validation e handoff para wiring.
- o runtime fica pronto para reload futuro, incidentes e evolucoes do motor sem duplicar codigo morto ou wiring disperso.

## Preserve These Decisions

- `configctl` continua dono do lifecycle e da verdade de runtime ativo.
- `server` continua sendo facade HTTP fina sobre contracts existentes.
- `consumer` continua sendo a ponte Kafka -> JetStream, nao o dono de regra de negocio.
- `validator` continua dono da avaliacao minima, estado carregado e leitura de resultados.
- `raccoon-cli` continua sendo o motor canonico de `check`, `verify`, `check-deep`, `scenario-smoke`, `results-inspect` e `trace-pack`.
- contracts de bootstrap e dataplane devem continuar pequenos, explicitos e auditaveis por `contract-audit` e `runtime-bindings`.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | fechar boundaries entre data plane, runtime ownership e topology | [cluster-architect](../agents/cluster-architect.md) | separar responsabilidades entre emulator, consumer, validator, server e registry |
| Contract Guardian | proteger subjects, streams, payloads e queries | [contract-guardian](../agents/contract-guardian.md) | boundary Kafka adapter -> contrato interno -> ValidationResult |
| Runtime Validator | provar e2e local e resiliencia minima | [runtime-validator](../agents/runtime-validator.md) | `happy-path`, `invalid-payload`, `results-inspect`, `trace-pack` |
| TDD Coordinator | escolher a escada de validacao antes de cada frente | [tdd-coordinator](../agents/tdd-coordinator.md) | `make check`, testes focados, `make verify`, `check-deep` |
| CLI Evolution | alinhar smoke e diagnostics com a nova verdade operacional | [cli-evolution](../agents/cli-evolution.md) | `runtime-bindings`, `contract-audit`, smoke e observabilidade |
| Documentation Writer | consolidar a verdade final no contexto vivo | [documentation-writer](../agents/documentation-writer.md) | docs canônicas e tracking do `.context` |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| bloco 3 inflar DSL ou dominio do `configctl` para resolver problemas de dataplane | Medium | High | manter bootstrap pequeno e puxar complexidade para adapters/runtime do dataplane | `cluster-architect` |
| `consumer` misturar adaptacao Kafka com contrato interno e dificultar validacao | High | High | explicitar boundary de normalizacao e testar mapper separado do transporte | `contract-guardian` |
| `validator` crescer como loop procedural unico e dificultar Block 4 | Medium | High | introduzir regras minimas e separar desde cedo consume, evaluate, store e query | `cluster-architect` |
| observabilidade insuficiente gerar smoke verde sem leitura operacional real | Medium | High | exigir `results-inspect`, endpoints do `server` e `trace-pack` como parte do aceite | `runtime-validator` |
| Block 4 virar refactor abstrato sem ganho operacional | Medium | Medium | amarrar cada refatoracao a ownership, supervisao, reducao de wiring e readiness para reload futuro | `tdd-coordinator` |

### Dependencies

- **Internal:** `internal/application/dataplane/*`, `internal/application/runtimebootstrap/*`, `internal/application/validatorresults/*`, `internal/actors/scopes/consumer/*`, `internal/actors/scopes/validator/*`, `internal/adapters/kafka/*`, `internal/adapters/nats/*`, `cmd/consumer/run.go`, `cmd/validator/run.go`, `cmd/emulator/run.go`, `cmd/server/run.go`
- **External:** Docker Compose local com `nats` e `kafka` saudaveis
- **Technical:** manter `make check`, `make verify`, `make check-deep`, `make scenario-smoke`, `make results-inspect` e `make trace-pack` coerentes com o novo fluxo

### Assumptions

- o endpoint `/runtime/ingestion/bindings` continua sendo o seam canonico de bootstrap ativo do dataplane.
- o primeiro data plane real do bloco 3 pode operar com regras simples e `ValidationResult` pequeno sem exigir engine de regras expandida.
- o endurecimento do bloco 4 deve se apoiar em ownership e registry, nao em novos atalhos procedurais em `run.go`.

## Working Phases

### Phase 1 - Freeze, Sequencing And Boundaries
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar o recorte dos blocos 3 e 4, definindo o que precisa existir no primeiro data plane real antes da refatoracao de runtime mais profunda.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | mapear o fluxo alvo ponta a ponta, do emulator ate o `ValidationResult` | `cluster-architect` + `contract-guardian` | pending | diagrama logico e seams explicitos |
| 1.2 | congelar os contracts minimos de Bloco 3: Kafka payload, mensagem interna canonical, JetStream subject e `ValidationResult` | `contract-guardian` | pending | freeze de payloads, metadata e invariantes |
| 1.3 | congelar o alvo de ownership do Bloco 4 para `consumer` e `validator` | `cluster-architect` | pending | matriz `consume / route / work / store / query / supervise` |
| 1.4 | fechar a escada de validacao de cada bloco antes de implementacao | `tdd-coordinator` + `runtime-validator` | pending | ladder before/during/after para B3 e B4 |

**Acceptance Criteria**

- o boundary entre adapter Kafka e contrato interno fica explicito antes de qualquer edit.
- o plano separa claramente "data plane minimo funcional" de "runtime hardening".
- o bloco 4 nao depende de reabrir contratos ou ownership do `configctl`.

---

### Phase 2 - Block 3 Real Dataplane Foundation
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** introduzir o primeiro data plane real e minimamente observavel do motor de qualidade.

**Front 1: Synthetic Generation**

- `emulator` deve produzir payloads controlados por binding ativo, incluindo pelo menos um caso valido e um invalido.
- a geracao deve ser suficientemente deterministica para smoke local, sem criar DSL extra.

**Front 2: Kafka To Canonical Dataplane Boundary**

- `consumer` deve isolar adaptacao Kafka, parsing e normalizacao do contrato interno.
- o contrato interno precisa carregar apenas o necessario para roteamento, correlacao, scope, binding, runtime e payload.

**Front 3: Minimal Validation**

- `validator` deve consumir do JetStream, resolver runtime ativo, aplicar regras simples e produzir `ValidationResult` pequeno e consistente.
- passed e failed devem ter contrato claro e validavel.

**Front 4: Operational Read Path**

- `server` deve permitir ler bindings ativos, runtime carregado e resultados sem virar owner do estado.
- o fluxo precisa ser observavel por operador local com `results-inspect` e endpoint HTTP.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | implementar ou endurecer o contrato interno do dataplane e seus mappers | `contract-guardian` + `feature-developer` | pending | boundary testado entre Kafka e JetStream |
| 2.2 | ligar `emulator` ao bootstrap ativo e publicacao controlada em Kafka | `feature-developer` | pending | geracao sintetica por binding |
| 2.3 | ligar `consumer` ao consumo Kafka e publicacao canonical no JetStream | `cluster-architect` | pending | bridge Kafka -> contrato interno -> JetStream |
| 2.4 | ligar `validator` ao consumo JetStream e `ValidationResult` minimo | `feature-developer` | pending | avaliacao simples com persistencia/query atual |
| 2.5 | fechar a superficie operacional minima via `server` e tooling | `runtime-validator` + `cli-evolution` | pending | leitura de resultados e evidencias coerentes |

**Acceptance Criteria**

- existe um caminho e2e real de ingestao ate resultado no cluster local.
- bootstrap depende de bindings ativos, nao de wiring manual escondido.
- `ValidationResult` e pequeno, observavel e suficiente para smoke e diagnostico.

---

### Phase 3 - Block 4 Runtime Ownership Hardening
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** reduzir acoplamento e logica procedural em `consumer` e `validator`, tornando o runtime mais supervisionado, registry-driven e preparado para evolucao.

**Front 1: Actor Topology**

- decompor `consumer` e `validator` em ownership explicito para consume, route, work, store e query.
- usar principios de MarketMonkey/Hollywood para que os actors tenham fronteiras operacionais claras.

**Front 2: Strong Registry**

- consolidar em registry os topics, subjects, streams, durables e bindings operacionais.
- impedir wiring disperso em `run.go`, supervisors e adapters.

**Front 3: Startup And Supervision**

- startup deve ser guiado por registry e supervisors, com menos logica procedural nos entrypoints.
- supervisao deve deixar claras as fronteiras entre bootstrap, workers, responders e stores.

**Front 4: Runtime Hygiene**

- remover codigo morto, wiring duplicado e branches nao usados.
- preparar o terreno para reload futuro, incidentes e evolucoes do motor sem empurrar funcionalidade prematura.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | desenhar a topologia alvo de actors para `consumer` | `cluster-architect` | pending | mapa de actors e ownership |
| 3.2 | desenhar a topologia alvo de actors para `validator` | `cluster-architect` | pending | mapa de actors e ownership |
| 3.3 | consolidar registry de routing/runtime para startup e wiring | `contract-guardian` + `feature-developer` | pending | registry forte para topics/subjects/streams/durables |
| 3.4 | refatorar `run.go` para handoff fino e supervisionado | `feature-developer` | pending | entrypoints magros e sem logica procedural dispersa |
| 3.5 | remover codigo morto e estabilizar readiness/restart | `runtime-validator` + `code-reviewer` | pending | runtime mais previsivel e sem wiring sobrando |

**Acceptance Criteria**

- `consumer` e `validator` possuem ownership explicito por responsabilidade.
- startup e wiring passam a depender de registry e supervisao, nao de montagem procedural espalhada.
- o runtime fica objetivamente mais facil de evoluir sem reabrir contracts estabilizados.

---

### Phase 4 - Integrated Runtime Proof
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar o conjunto dos blocos 3 e 4 com guard rails estaticos, smoke real e observabilidade operacional suficiente.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 4.1 | rodar baseline estatica e testes focados por frente | `tdd-coordinator` + `runtime-validator` | pending | `make check`, testes focados, `make verify` |
| 4.2 | provar smoke e2e do data plane completo | `runtime-validator` | pending | `happy-path`, `invalid-payload`, `check-deep` |
| 4.3 | inspecionar resultados e coletar evidencia operacional | `runtime-validator` + `cli-evolution` | pending | `results-inspect`, `trace-pack`, endpoints HTTP |
| 4.4 | atualizar docs e contexto vivo | `documentation-writer` | pending | docs canonicas e tracking do `.context` |

**Acceptance Criteria**

- o e2e local passa de forma deterministica com leitura operacional suficiente para diagnostico.
- `contract-audit`, `runtime-bindings` e smoke convergem para a mesma historia operacional.
- docs e tooling refletem a nova base real do motor de qualidade.

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B34-P1-S1 | congelar o fluxo e2e alvo e seus seams | `cluster-architect` | nenhuma | flow map |
| B34-P1-S2 | congelar payloads e invariantes minimos do dataplane | `contract-guardian` | `B34-P1-S1` | contract freeze |
| B34-P1-S3 | fechar matriz de ownership do Block 4 | `cluster-architect` | `B34-P1-S1` | ownership matrix |
| B34-P1-S4 | fechar validation ladder | `tdd-coordinator` | `B34-P1-S2`, `B34-P1-S3` | before/after commands |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B34-P2-S1 | endurecer contrato interno do dataplane e mapeamento Kafka | `contract-guardian` | `B34-P1-S2` | testes de contrato |
| B34-P2-S2 | plugar `emulator` no bootstrap ativo e publicacao controlada | `feature-developer` | `B34-P1-S4` | smoke input real |
| B34-P2-S3 | plugar `consumer` em Kafka e JetStream com boundary claro | `cluster-architect` | `B34-P2-S1` | bridge em runtime |
| B34-P2-S4 | plugar `validator` em consume/evaluate/store/query minimos | `feature-developer` | `B34-P2-S1` | `ValidationResult` funcional |
| B34-P2-S5 | alinhar server/tooling para leitura operacional minima | `cli-evolution` + `runtime-validator` | `B34-P2-S4` | results inspect + endpoint |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B34-P3-S1 | definir topologia alvo de actors de `consumer` | `cluster-architect` | `B34-P2-S3` | actor map |
| B34-P3-S2 | definir topologia alvo de actors de `validator` | `cluster-architect` | `B34-P2-S4` | actor map |
| B34-P3-S3 | consolidar registry e wiring compartilhado | `feature-developer` | `B34-P3-S1`, `B34-P3-S2` | registry forte |
| B34-P3-S4 | emagrecer `run.go` e estabilizar startup/supervisao | `feature-developer` | `B34-P3-S3` | entrypoints finos |
| B34-P3-S5 | remover codigo morto e fechar gaps de ownership | `code-reviewer` + `runtime-validator` | `B34-P3-S4` | review e prova local |

### Phase 4 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B34-P4-S1 | rodar baseline estatica e testes focados | `runtime-validator` | todas as etapas anteriores | `make check`, `make verify` |
| B34-P4-S2 | provar smoke e2e do data plane | `runtime-validator` | `B34-P4-S1` | `happy-path`, `invalid-payload`, `check-deep` |
| B34-P4-S3 | coletar evidencia operacional final | `runtime-validator` + `cli-evolution` | `B34-P4-S2` | `results-inspect`, `trace-pack` |
| B34-P4-S4 | atualizar docs e tracking | `documentation-writer` | `B34-P4-S3` | docs atualizadas |

## Validation Ladder

**Before**

- `make check`
- `make tdd`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `raccoon-cli recommend internal/application/dataplane internal/application/runtimebootstrap internal/application/validatorresults internal/actors/scopes/consumer internal/actors/scopes/validator`

**During Block 3**

- `go test ./internal/application/dataplane ./internal/application/validatorresults`
- `go test ./internal/adapters/... ./internal/actors/scopes/validator/... ./internal/actors/scopes/consumer/...`
- `make verify`

**During Block 4**

- `go test ./internal/actors/scopes/consumer ./internal/actors/scopes/validator`
- `go test ./cmd/consumer ./cmd/validator`
- `make verify`
- `make arch-guard`

**After**

- `make up-dataplane`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`
- `make results-inspect`
- `make trace-pack`

## Done Definition

Este plano so termina quando:

- o primeiro data plane real do motor de qualidade estiver operando ponta a ponta no cluster local.
- `consumer` e `validator` tiverem ownership, supervisao e wiring mais claros do que o estado procedural atual.
- os contracts centrais do dataplane estiverem pequenos, auditaveis e alinhados com `server`, `results-inspect` e smoke.
- a prova operacional do repositorio continuar passando pelo workflow canonico baseado em `make` e `raccoon-cli`, sem sequencias ad hoc.

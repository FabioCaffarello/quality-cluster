---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Congelar ownership e fronteiras entre configctl, server, validator e futuro consumer"
  - type: "contract-guardian"
    role: "Fechar as queries, payloads e seams entre estado de runtime, bootstrap e evento de refresh"
  - type: "runtime-validator"
    role: "Definir a prova local e runtime-sensitive para o contract consolidado"
  - type: "cli-evolution"
    role: "Manter contract-audit, runtime-bindings, smoke e trace-pack coerentes com o novo contract"
  - type: "tdd-coordinator"
    role: "Escolher a menor escada de validacao que ainda prove a verdade do runtime"
  - type: "code-reviewer"
    role: "Revisar risco de acoplamento, duplicacao de source of truth e leak de dominio"
  - type: "test-writer"
    role: "Cobrir use cases, handlers, gateways e cenarios do contract consolidado"
  - type: "documentation-writer"
    role: "Atualizar docs, .http e contexto canonico com o runtime contract final"
docs:
  - "project-overview.md"
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Runtime Contract Freeze"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Contract Consolidation & HTTP Surface"
    prevc: "E"
    agent: "contract-guardian"
  - id: "phase-3"
    name: "Validation & Handoff"
    prevc: "V"
    agent: "runtime-validator"
---

# Block 2 - Ingestion Runtime Contract Plan

> Consolidar o runtime contract da ingestao com configctl como fonte de verdade, explicitando bindings ativos, topicos governados, artifacts associados, queries canonicas e superficies HTTP operacionais estaveis para preparar consumer futuro sem transferir dominio para os servicos de ingestao.

## Task Snapshot

- **Primary goal:** sair do Bloco 2 com um contract de runtime de ingestao pequeno, explicito e estavel, onde `configctl` projeta o que esta ativo, `server` apenas expoe a leitura operacional necessaria, `validator` segue limitado ao estado runtime ja carregado e o futuro `consumer` passa a depender de um bootstrap canonico em vez de logica de dominio propria.
- **Success signal:** existe uma matriz clara entre queries de estado, evento de runtime e read models HTTP; `ListActiveRuntimeProjections` e `ListActiveIngestionBindings` contam a mesma historia operacional; `/runtime/ingestion/bindings` continua estavel para inspecao local e bootstrap; `validator` nao vira dono do runtime da ingestao; `contract-audit`, `runtime-bindings`, smoke e docs ficam alinhados ao mesmo seam.
- **Out of scope:** multiscope hardening, refresh event-driven, persistencia nova, horizontalizacao, mudanca do actor model, redesign do dataplane message contract e ampliacao do `server` para orquestrador de dominio.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Bloco 1 - Lifecycle Hardening](./block-1-lifecycle-hardening.md)
  - [Block 2 Runtime Contract Freeze](./block-2-ingestion-runtime-contract-freeze.md)

## Codebase Context

### What Already Exists And Must Be Preserved

- `configctl` ja expoe as queries canonicas de runtime por NATS em [`internal/adapters/nats/configctl_registry.go`](../internal/adapters/nats/configctl_registry.go):
  - `configctl.control.list_active_runtime_projections`
  - `configctl.control.list_active_ingestion_bindings`
- os contracts ja sao relativamente pequenos e uteis:
  - `RuntimeProjectionRecord` agrega scope, config, artifact, checksum, bindings, fields e rules
  - `ActiveIngestionBindingRecord` agrega binding, fields e metadata compacta de runtime
- `server` ja e uma borda fina que usa gateways e use cases em [`cmd/server/run.go`](../cmd/server/run.go) e hoje expoe:
  - `/runtime/ingestion/bindings`
  - `/runtime/validator/active`
  - `/runtime/validator/results`
- o bootstrap do dataplane ja depende de `/runtime/ingestion/bindings` em [`internal/application/runtimebootstrap/client.go`](../internal/application/runtimebootstrap/client.go), derivando assinatura, indice e topologia sem ler dominio bruto.
- `validator` ja carrega runtime a partir de projecoes de `configctl` e responde somente o estado carregado localmente em [`internal/actors/scopes/validator/runtime_cache.go`](../internal/actors/scopes/validator/runtime_cache.go) e [`internal/actors/scopes/validator/runtime_query_responder.go`](../internal/actors/scopes/validator/runtime_query_responder.go).
- o evento `config.ingestion_runtime_changed` ja existe como contract de refresh, mas ele e trigger, nao fonte de verdade, em [`internal/adapters/nats/configctl_registry.go`](../internal/adapters/nats/configctl_registry.go).

### Gaps Driving This Block

- a separacao entre query de estado (`list_active_runtime_projections`, `list_active_ingestion_bindings`) e evento de runtime (`config.ingestion_runtime_changed`) existe no codigo, mas ainda nao esta congelada como regra operacional para a proxima evolucao.
- a superficie HTTP estavel para inspecao local da ingestao ainda e implicita demais: `/runtime/ingestion/bindings` existe, mas o papel dela versus `configctl` queries e `validator` runtime nao esta suficientemente explicito.
- o `server` ja conhece a porta `ListActiveRuntimeProjections` via gateway, porem ainda nao existe decisao fechada sobre quanto dessa query deve virar HTTP e quanto deve permanecer apenas no seam NATS/control-plane.
- o futuro `consumer` precisa de um contract estavel de leitura operacional que cubra binding, topico, scope, config version, checksum e artifact sem acoplar a servico de ingestao a lifecycle ou parsing de evento.
- sem esse freeze, existe risco real de espalhar logica de dominio de config para `consumer`, `validator` ou handlers HTTP.

### Decisions To Preserve

- `configctl` continua owner de lifecycle, projecoes ativas, bindings ativos e artifacts associados.
- `server` continua sendo facade HTTP fina e nao vira orquestrador nem fonte paralela de runtime.
- `validator` continua com responsabilidade limitada a runtime carregado localmente e results query.
- `consumer` e `emulator` devem depender de queries de runtime projetadas por `configctl`, nunca de logica de dominio replicada.
- evento de runtime continua sendo trigger de convergencia; query de runtime continua sendo a fonte de verdade do estado ativo.

## Diagnostic Summary

### Current Strengths

- a camada de contracts ja carrega os campos certos para um bootstrap operacional: binding, topic, scope, config version, checksum, artifact e activation time.
- o gateway de `server` ja suporta `ListActiveRuntimeProjections`, entao a borda HTTP pode evoluir sem mover dominio para outro servico.
- o bootstrap client ja usa um seam pequeno e pragmatico, baseado em bindings ativos e assinatura de runtime.
- `validator` ja esta corretamente posicionado como consumidor de runtime e nao como produtor de truth.

### Current Distortions

- o contract canonico de runtime da ingestao ainda esta mais espalhado entre codigo, docs e inferencia do que deveria.
- a leitura operacional de runtime ainda mistura tres visoes diferentes sem uma hierarquia fechada:
  - truth de `configctl`
  - estado carregado de `validator`
  - leitura HTTP do `server`
- o evento `config.ingestion_runtime_changed` ainda pode ser interpretado equivocadamente como estado efetivo se essa separacao nao for congelada antes dos proximos blocos.

## Priority Problems

| Priority | Problem | Why It Matters Now | Preserve Or Remove |
| --- | --- | --- | --- |
| P1 | Source of truth de runtime da ingestao ainda implicito | o futuro `consumer` precisa bootstrap estavel antes de qualquer evolucao de dataplane | preservar `configctl` como owner e explicitar isso |
| P1 | Estado e evento de runtime ainda nao estao congelados como seams separados | sem isso o proximo bloco pode mover dominio para `consumer` ou `validator` | preservar evento, remover ambiguidade |
| P1 | Superficie HTTP de inspecao local ainda nao esta fechada como contract | aumenta risco de aliases, wrappers ad hoc e acoplamento indevido | endurecer e estabilizar |
| P2 | `validator` pode virar leitura de runtime "quase canonica" por conveniencia | cria fonte paralela de verdade e acoplamento transversal | preservar leitura minima, remover sobrecarga |
| P2 | `RuntimeProjectionRecord` e `ActiveIngestionBindingRecord` podem crescer sem fronteira | degrada contracts operacionais pequenos | preservar seam pequeno e explicito |
| P3 | numeracao antiga do bloco 2 nao representa mais esta fundacao | pode gerar rastreamento confuso entre planos | documentar supersessao no ai-context |

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | congelar ownership e a matriz truth vs edge vs loaded-state | [cluster-architect](../agents/cluster-architect.md) | fronteiras entre `configctl`, `server`, `validator` e futuro `consumer` |
| Contract Guardian | fechar queries, payloads e semantica entre estado, bindings e evento | [contract-guardian](../agents/contract-guardian.md) | NATS subjects, payloads, wrappers HTTP e contracts operacionais |
| Runtime Validator | desenhar a prova local e runtime-sensitive do bloco | [runtime-validator](../agents/runtime-validator.md) | smoke, readiness, runtime queries e evidencia operacional |
| CLI Evolution | manter analyzers e smoke contando a historia certa | [cli-evolution](../agents/cli-evolution.md) | `contract-audit`, `runtime-bindings`, `trace-pack` e smoke |
| TDD Coordinator | escolher a escada de validacao por blast radius | [tdd-coordinator](../agents/tdd-coordinator.md) | baseline before/after e menor prova confiavel |
| Code Reviewer | revisar risco de acoplamento e fonte paralela de verdade | [code-reviewer](../agents/code-reviewer.md) | regressao arquitetural e behavioural drift |
| Test Writer | cobrir handlers, use cases e gateways tocados | [test-writer](../agents/test-writer.md) | testes de contract e integracao local |
| Documentation Writer | sincronizar docs e contexto do repo | [documentation-writer](../agents/documentation-writer.md) | docs canonicas, `.http` e `.context` |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | refletir que o runtime da ingestao tem seams explicitos entre truth, loaded-state e bootstrap |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | registrar onde termina `configctl` e onde comecam `server`, `validator` e futuro `consumer` |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | congelar `list_active_runtime_projections`, `list_active_ingestion_bindings` e `config.ingestion_runtime_changed` |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | ajustar a escada de prova para o contract de runtime da ingestao |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar analyzers e smoke com a distincao entre query e trigger |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| Expor `validator` como runtime canonico por conveniencia | Medium | High | congelar que `validator.runtime.get_active` e loaded-state local, nao query de truth | `cluster-architect` |
| Transformar evento de runtime em payload de estado efetivo | Medium | High | tratar `config.ingestion_runtime_changed` apenas como trigger; truth continua em queries do `configctl` | `contract-guardian` |
| Aumentar demais o payload de bindings e gerar acoplamento com dominio | Medium | Medium | preservar payload pequeno e operacional; so adicionar campo com justificativa de bootstrap/operabilidade | `contract-guardian` |
| Criar superficie HTTP duplicada e voltar a espalhar aliases | Medium | Medium | qualquer endpoint novo so entra se fechar um gap real de inspecao; nao espelhar NATS por espelhar | `code-reviewer` |
| Escopo crescer para refresh, multiscope ou persistencia | Medium | High | manter este bloco restrito ao contract fundacional; deixar convergencia avancada para blocos posteriores | `tdd-coordinator` |

### Dependencies

- **Internal:** `internal/application/configctl/contracts`, `internal/adapters/nats/configctl_registry.go`, `cmd/server`, `internal/interfaces/http/handlers/runtime.go`, `internal/application/runtimebootstrap/client.go`, `internal/application/dataplane`, `internal/actors/scopes/validator`
- **External:** nenhuma dependencia externa nova deve ser introduzida
- **Technical:** NATS request/reply, JetStream contracts, Make targets `check`, `verify`, `check-deep`, `scenario-smoke`, `trace-pack`, `results-inspect`

### Assumptions

- o runtime atual ainda e simples o suficiente para manter `configctl` como unica fonte de verdade sem criar nova store dedicada
- `/runtime/ingestion/bindings` ja e suficiente como bootstrap local base; endpoint novo so faz sentido se um gap operacional concreto permanecer depois do freeze
- o bloco nao precisa mover ou reescrever o consumer; ele precisa preparar o contract que esse consumer vai usar
- se essas premissas falharem, o trabalho deve ser quebrado e reenquadrado antes de tocar blocos de dataplane mais pesados

## Working Phases

### Phase 1 - Runtime Contract Freeze
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar a hierarquia de ownership e a matriz entre truth, bootstrap operacional, loaded-state e evento de runtime antes de qualquer implementacao.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | Fechar a matriz canonica de runtime state: `list_active_runtime_projections`, `list_active_ingestion_bindings`, `validator.runtime.get_active` e `config.ingestion_runtime_changed` | `cluster-architect` + `contract-guardian` | pending | tabela source-of-truth vs read-model vs trigger |
| 1.2 | Congelar o payload minimo necessario para bootstrap e inspecao local sem leak de dominio | `contract-guardian` | pending | freeze de campos obrigatorios e limites do payload |
| 1.3 | Decidir se a superficie HTTP atual basta ou se existe um gap real que justifique um read model adicional em `/runtime/*` | `cluster-architect` + `code-reviewer` | pending | decisao explicita sobre superficie HTTP |
| 1.4 | Fechar o limite de responsabilidade do `validator` no bloco | `cluster-architect` + `runtime-validator` | pending | nota arquitetural de loaded-state minimo |

**Acceptance Criteria**

- ownership entre `configctl`, `server`, `validator` e futuro `consumer` fica explicito
- estado e evento de runtime deixam de competir como seams equivalentes
- payloads operacionais minimos ficam congelados
- qualquer evolucao de HTTP fica justificada por necessidade operacional real, nao por espelhamento cego

---

### Phase 2 - Contract Consolidation & HTTP Surface
> **Primary Agent:** `contract-guardian` - [Playbook](../agents/contract-guardian.md)

**Objective:** aplicar a consolidacao do contract sem mover dominio para o dataplane e sem inflar a borda HTTP.

**Front 1: Configctl Runtime Truth**

- manter `ListActiveRuntimeProjections` e `ListActiveIngestionBindings` como queries canonicas de runtime no `configctl`
- revisar normalizacao de scope e filtros para que os dois seams contem a mesma historia operacional
- garantir que bindings ativos continuem carregando topic, scope, config version, checksum, artifact e activation metadata suficientes para bootstrap

**Front 2: Thin HTTP Operational Surface**

- preservar `/runtime/ingestion/bindings` como bootstrap seam primario para inspecao local e futuro consumer
- manter `/runtime/validator/active` e `/runtime/validator/results` como visoes do validator, nao do `configctl`
- se a phase 1 provar necessidade, adicionar ou ajustar um read model HTTP pequeno e operacional, usando gateway de `configctl` sem expor dominio bruto nem replicar `/configctl/*`

**Front 3: Validator Minimal Runtime Contract**

- manter o `validator` respondendo apenas o runtime carregado localmente
- revisar handlers, gateways e docs para que ninguem passe a trata-lo como source of truth da ingestao
- preservar o bootstrap por projecoes de `configctl` e nao por store propria do validator

**Front 4: State vs Event Seam**

- alinhar contracts, docs e tooling para que `config.ingestion_runtime_changed` seja explicitamente tratado como trigger
- manter query de estado como fonte de leitura canonica para bootstrap e inspecao
- evitar que payload do evento carregue responsabilidade de snapshot de runtime

**Front 5: Future Consumer Readiness**

- garantir que o seam de bootstrap continue pequeno e suficiente para construir `BindingIndex`, `RuntimeTopology` e assinatura de runtime
- revisar onde o contract atual ainda exige inferencia ou composicao excessiva e simplificar isso sem mover dominio

**Acceptance Criteria**

- a hierarquia entre query de truth, read model HTTP e loaded-state do validator fica codificada e documentada
- o contract de bootstrap da ingestao continua pequeno, explicito e suficiente para o futuro consumer
- nenhuma logica de lifecycle ou ownership de config e empurrada para `server`, `validator` ou dataplane clients
- qualquer ajuste de HTTP continua fino e estavel

---

### Phase 3 - Validation & Handoff
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar que o contract consolidado funciona no repositorio real e preparar o terreno para a evolucao do consumer sem regressao de arquitetura, tooling ou smoke.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | Revalidar `make check`, `make verify`, `contract-audit`, `runtime-bindings`, `arch-guard` e `drift-detect` | `runtime-validator` + `cli-evolution` | pending | baseline estatica e arquitetural verde |
| 3.2 | Provar a superficie HTTP e a leitura operacional com os cenarios minimos certos | `runtime-validator` + `tdd-coordinator` | pending | `config-lifecycle`, `missing-binding` e cenarios adicionais conforme blast radius |
| 3.3 | Rodar `check-deep` e `happy-path` se o bloco tocar bootstrap efetivo ou dataplane path | `runtime-validator` | pending | evidencia runtime-sensitive quando o diff exigir |
| 3.4 | Atualizar docs, `.http` e `.context` com a hierarquia final do contract | `documentation-writer` | pending | contexto vivo alinhado ao resultado do bloco |

**Acceptance Criteria**

- static checks e contracts continuam verdes
- a leitura operacional local de runtime da ingestao fica inequivoca
- o validator continua minimo e coerente com o truth do `configctl`
- docs, smoke e contracts passam a contar exatamente a mesma historia

## Executable Backlog

### Backlog Rules

- nenhum endpoint ou payload novo entra antes do freeze do ownership e da hierarquia truth vs trigger
- o `server` nao deve espelhar NATS control surface por conveniencia; toda exposicao HTTP precisa de justificativa operacional
- o futuro `consumer` e cliente do contract consolidado, nao argumento para espalhar dominio
- quando houver conflito entre docs antigas e code/contracts atuais, a verdade e o seam efetivo do repositorio

### Phase 1 Backlog - Runtime Contract Freeze

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P1-S1 | Fechar a matriz de ownership entre `configctl`, `server`, `validator` e futuro `consumer` | `cluster-architect` | nenhuma | matriz truth vs edge vs loaded-state |
| B2-P1-S2 | Congelar os payloads minimos de `RuntimeProjectionRecord` e `ActiveIngestionBindingRecord` | `contract-guardian` | `B2-P1-S1` | freeze dos campos obrigatorios e limites do contract |
| B2-P1-S3 | Decidir a superficie HTTP operacional canonica do bloco | `cluster-architect` + `code-reviewer` | `B2-P1-S1`, `B2-P1-S2` | decisao registrada sobre endpoints a manter/adicionar |
| B2-P1-S4 | Definir a escada de validacao do bloco | `tdd-coordinator` | `B2-P1-S2`, `B2-P1-S3` | sequencia before/after explicita |

### Phase 2 Backlog - Contract Consolidation & HTTP Surface

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P2-S1 | Consolidar use cases, gateways e registries para que as queries canonicas de runtime fiquem explicitas e coerentes | `contract-guardian` | `B2-P1-S1`, `B2-P1-S2` | diffs em contracts, gateways ou use cases |
| B2-P2-S2 | Endurecer a superficie HTTP operacional escolhida na phase 1 | `contract-guardian` + `feature-developer` | `B2-P1-S3` | handlers/routes/read models alinhados |
| B2-P2-S3 | Revisar `runtimebootstrap` e dataplane contracts para dependerem apenas do seam canonico | `cluster-architect` | `B2-P2-S1` | bootstrap contract pequeno e suficiente |
| B2-P2-S4 | Garantir que `validator` permaneça leitura minima do estado carregado | `code-reviewer` + `feature-developer` | `B2-P1-S4`, `B2-P2-S2` | revisao/ajuste de handlers e docs do validator |
| B2-P2-S5 | Alinhar analyzers, smoke, `.http` e docs a distincao entre query e trigger | `cli-evolution` + `documentation-writer` | `B2-P2-S1`, `B2-P2-S2`, `B2-P2-S3`, `B2-P2-S4` | tooling e docs sem drift |

### Phase 3 Backlog - Validation & Handoff

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P3-S1 | Rodar baseline estatica e arquitetural completa apos a consolidacao | `runtime-validator` | todos os steps da phase 2 | `make check`, `make verify`, `contract-audit`, `runtime-bindings`, `arch-guard`, `drift-detect` |
| B2-P3-S2 | Provar a leitura operacional do runtime com os cenarios minimos corretos | `runtime-validator` | `B2-P3-S1` | `config-lifecycle`, `missing-binding` e testes/HTTP proof correspondentes |
| B2-P3-S3 | Escalar para `happy-path` e `check-deep` se o seam de bootstrap afetar dataplane real | `runtime-validator` | `B2-P3-S2` | evidencia runtime-sensitive adicional |
| B2-P3-S4 | Atualizar `.context/docs`, `.context/plans` e `.http` com a verdade final do bloco | `documentation-writer` | `B2-P3-S1`, `B2-P3-S2`, `B2-P3-S3` | contexto canonico sincronizado |

## Validation Ladder

**Before**

- `make check`
- `make tdd`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`

**During**

- testes de use case e gateway em `configctlclient`, handlers runtime e contracts tocados
- testes de rota/handler para qualquer ajuste em `/runtime/*`
- testes focados no bootstrap client quando o payload de bindings mudar

**After**

- `make verify`
- `make scenario-smoke SCENARIO=config-lifecycle`
- `make scenario-smoke SCENARIO=missing-binding`
- `make scenario-smoke SCENARIO=happy-path` quando o seam atingir dataplane
- `make check-deep` quando o seam atingir bootstrap/convergencia real

## Order Of Execution

1. congelar ownership e a hierarquia query vs event
2. congelar payloads minimos do contract
3. decidir a superficie HTTP operacional necessaria
4. consolidar contracts, gateways, handlers e bootstrap seam
5. provar a nova fundacao por checks estaticos e smoke

## Expected Gains

- o futuro `consumer` passa a depender de um contract fundacional pequeno e confiavel
- `configctl` fica explicitamente reconhecido como runtime truth do dataplane
- `validator` para de competir semanticamente com `configctl`
- a borda HTTP vira inspecao operacional estavel, nao espaco para acoplamento extra
- os proximos blocos podem evoluir refresh e dataplane sem reabrir ownership basico

## Done Definition

Este bloco so termina quando:

- a hierarquia entre query de estado, loaded-state do validator e evento de runtime estiver explicita e aplicada
- `ListActiveRuntimeProjections` e `ListActiveIngestionBindings` tiverem fronteiras pequenas e operacionais bem definidas
- a superficie HTTP local de runtime estiver estavel e coerente com a truth do `configctl`
- `server`, `configctl`, `validator`, `.http`, smoke e docs contarem a mesma historia
- os checks relevantes do `raccoon-cli` continuarem verdes

## Evidence To Collect

- saida de `make check`
- saida de `make verify`
- saida de `raccoon-cli contract-audit`
- saida de `raccoon-cli runtime-bindings`
- saida de `make arch-guard`
- saida de `make drift-detect`
- evidencia HTTP e/ou `scenario-smoke` para a superficie operacional do runtime
- `trace-pack` e `results-inspect` se houver regressao de bootstrap ou drift entre state e loaded-state

## Evidence & Follow-up

### Artifacts to Collect

- freeze do contract em `.context/plans/block-2-ingestion-runtime-contract-freeze.md`
- evidencias de validacao em artefatos do bloco quando houver execucao
- links/diffs de docs e `.http` atualizados junto do contract final

### Success Metrics

- zero fontes paralelas de truth para runtime da ingestao
- zero ambiguidades documentadas entre query de estado e trigger de refresh
- bootstrap seam do futuro consumer descrito por um contract pequeno e suficiente
- checks estaticos e runtime relevantes verdes no fim do bloco

### Follow-up Actions

| Action | Owner (Agent) | Due |
|--------|---------------|-----|
| Usar este contract como precondicao para o bloco que endurece dataplane/consumer | `cluster-architect` | proximo bloco de execucao |
| Revalidar se algum endpoint HTTP adicional ainda e necessario apos a implementacao | `code-reviewer` | fechamento da phase 2 |

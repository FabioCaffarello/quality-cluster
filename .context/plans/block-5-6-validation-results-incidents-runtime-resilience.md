---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Separar ownership entre resultado tecnico, incidente operacional e alerting futuro sem inflar o validator"
  - type: "contract-guardian"
    role: "Definir contratos claros para ValidationResult e ValidationIncident e proteger suas consultas e publicacoes"
  - type: "runtime-validator"
    role: "Provar idempotencia, restart, reload, readiness e diagnostico no cluster real"
  - type: "tdd-coordinator"
    role: "Fechar a escada minima de testes, smoke e deep gate para a evolucao do motor"
  - type: "cli-evolution"
    role: "Manter o raccoon-cli aderente as novas superficies de resultado, incidente e observabilidade operacional"
  - type: "documentation-writer"
    role: "Atualizar docs canonicas e registrar a nova semantica operacional"
docs:
  - "project-overview.md"
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Ownership, Semantics And Success Signals"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Validation Result And Incident Contracts"
    prevc: "E"
    agent: "contract-guardian"
  - id: "phase-3"
    name: "Operational Resilience Hardening"
    prevc: "E"
    agent: "runtime-validator"
  - id: "phase-4"
    name: "Runtime Proof And Canonical Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Blocks 5-6 - Validation Results, Incidents, And Runtime Resilience Plan

> Este plano segue a numeracao pedida nesta conversa e nao substitui os blocos 5 e 6 historicos ja existentes em `.context/plans/`.

> Consolidar `ValidationResult` como resultado tecnico por mensagem, introduzir `ValidationIncident` como sinal operacional minimo e endurecer o motor de qualidade para comportamento previsivel sob replay, restart, reload e diagnostico real.

## Task Snapshot

- **Primary goal:** separar o contrato de resultado tecnico do contrato de incidente operacional sem empurrar alerting para dentro do `validator`, e em seguida tornar o ciclo ingestao -> JetStream -> validacao -> resultado/incidente mais idempotente, reiniciavel e observavel.
- **Success signal:** `validator` continua dono da validacao e dos read models operacionais pequenos; `server` expoe consultas minimas de resultados e incidentes; o runtime se comporta de forma previsivel sob redelivery e restart; `make verify`, `make check-deep`, `make scenario-smoke`, `make results-inspect` e `make trace-pack` continuam sendo prova suficiente.
- **Out of scope:** construir o futuro `alerter`, definir politicas de notificacao, introduzir workflow humano de incident management, transformar o `validator` em servico de analytics, ou deslocar truth de runtime para `server` ou `raccoon-cli`.
- **Key references:**
  - [Project Overview](../docs/project-overview.md)
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 3-4 Real Dataplane And Runtime Hardening](./block-3-4-real-dataplane-runtime-hardening.md)
  - [Block 5 Event Driven Dataplane Refresh](./block-5-event-driven-dataplane-refresh.md)
  - [Block 6 Event Refresh Diagnostics Hardening](./block-6-event-refresh-diagnostics-hardening.md)

## Codebase Context

### Current Diagnostic

- `internal/application/validatorresults/contracts/results.go` ja define `ValidationResultRecord` como contrato tecnico estrito, com metadados de mensagem, binding, scope, config, status e violacoes.
- `internal/application/validatorresults/evaluate.go` produz esse resultado diretamente a partir de `RuntimeProjection` e `dataplane.Message`; hoje nao existe um contrato separado para incidente operacional nem um limite explicito entre falha tecnica e futuro alerting.
- `internal/actors/scopes/validator/results_store.go` materializa resultados em memoria, com capacidade fixa e deduplicacao apenas por `MessageID`; isso e suficiente para consulta local recente, mas ainda nao fecha sozinho a semantica de replay, restart e incidente.
- `internal/actors/scopes/validator/runtime_cache.go` ja faz bootstrap do runtime ativo a partir de `configctl` e reage a eventos de ativacao/desativacao; o loaded-state do `validator` continua sendo derivado, nao fonte de verdade.
- `cmd/server/readiness.go` ja trata resultado e runtime do `validator` como parte do estado operacional; qualquer nova superficie de incidente, idempotencia ou reload precisa continuar coerente com esse readiness.
- `validator.results.list` e `/runtime/validator/results` ja sao superfices de consulta consolidadas; incidentes ainda nao possuem contrato equivalente.

### Preserve These Decisions

- `validator` continua responsavel por validar mensagens, produzir resultados tecnicos e sinalizar incidentes minimos; ele nao faz policy engine, escalacao, roteamento de notificacao ou correlacao de alerting.
- `server` permanece uma facade fina de consulta; ele nao passa a ser dono de estado operacional nem do lifecycle de incidente.
- `configctl` continua fonte de verdade para runtime ativo e bindings; `validator` continua derivando loaded-state e read models a partir dessa verdade e do dataplane.
- os read models de resultados e incidentes devem permanecer pequenos, operacionais e explicitamente limitados; este bloco nao cria um historico analitico sem fim.
- evolucao de contrato deve ser aditiva e provavel pelo `raccoon-cli`; nao vale introduzir semantica escondida apenas em logs ou wiring de actor.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | fechar ownership entre resultado, incidente, query e runtime | [cluster-architect](../agents/cluster-architect.md) | impedir que `validator`, `server` e futuro `alerter` misturem papeis |
| Contract Guardian | desenhar contratos e seams de consumo e consulta | [contract-guardian](../agents/contract-guardian.md) | separar `ValidationResult` de `ValidationIncident` sem drift |
| Runtime Validator | provar comportamento sob replay, restart e reload | [runtime-validator](../agents/runtime-validator.md) | runtime real, readiness, smoke e troubleshooting |
| TDD Coordinator | definir a ladder minima por blast radius | [tdd-coordinator](../agents/tdd-coordinator.md) | testes focados antes de deep gate |
| CLI Evolution | manter analyzers, smoke e diagnostico aderentes | [cli-evolution](../agents/cli-evolution.md) | `results-inspect`, `trace-pack`, contract checks e gaps de observabilidade |
| Documentation Writer | consolidar a nova verdade operacional | [documentation-writer](../agents/documentation-writer.md) | docs canonicas e artefatos de validacao |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | registrar que o runtime passa a ter contratos separados de resultado e incidente |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | explicitar ownership entre validator, server e futuro consumidor de incidentes |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | definir quando evolucoes de resultado/incidente exigem verify, smoke, deep gate e diagnostico |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | documentar as novas superfices de consulta e/ou publicacao de incidente e o contrato consolidado de resultado |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar resultados, incidentes, readiness e evidencias operacionais ao fluxo canonico de prova |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| inflar `ValidationIncident` com semantica de alerting precoce | Medium | High | congelar um schema minimo e proibitivo para notificacao, policy e escalacao | `cluster-architect` |
| duplicar o mesmo problema em resultado e incidente sem fronteira clara | Medium | High | definir relacao explicita entre resultado tecnico por mensagem e incidente operacional agregado | `contract-guardian` |
| manter idempotencia baseada apenas em `MessageID` e multiplicar efeito de redelivery | High | High | introduzir chave de processamento mais forte e testes de replay, restart e dedupe | `runtime-validator` |
| prometer resiliencia de restart sem retention semantics explicita | Medium | High | decidir e documentar o que reconstrui, o que persiste e o que expira | `cluster-architect` + `runtime-validator` |
| adicionar novas consultas que parecem saudaveis, mas nao refletem estado real do runtime | Medium | Medium | amarrar readiness, query responders, trace-pack e smoke ao mesmo modelo de estado | `cli-evolution` + `runtime-validator` |

### Dependencies

- **Internal:** `internal/application/validatorresults/contracts/results.go`, `internal/application/validatorresults/evaluate.go`, `internal/actors/scopes/validator/results_store.go`, `internal/actors/scopes/validator/runtime_cache.go`, `internal/actors/scopes/validator/supervisor.go`, `internal/interfaces/http/handlers/runtime.go`, `cmd/server/readiness.go`, `tools/raccoon-cli/src/results_inspect/*`, `tools/raccoon-cli/src/trace_pack/*`
- **External:** NATS/JetStream, Kafka e o cluster local precisam continuar disponiveis para prova final de runtime
- **Technical:** a identidade de mensagem do dataplane precisa permanecer estavel o bastante para compor uma chave deterministica de processamento e deduplicacao

### Assumptions

- `ValidationResult` continuara sendo o artefato tecnico por mensagem e nao precisa absorver lifecycle de incidente.
- `ValidationIncident` pode permanecer um contrato operacional minimo, derivado de resultado e de falhas de runtime claramente delimitadas, sem precisar de motor completo de correlacao.
- os read models continuam pequenos; a semantica desejada e previsibilidade operacional, nao historico ilimitado.
- este ciclo pode endurecer restart e reload sem reabrir o ownership basico de `configctl`, `server`, `consumer`, `validator` e `raccoon-cli`.

## Working Phases

### Phase 1 - Freeze Ownership, Semantics And Success Signals
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar o que pertence a resultado, o que pertence a incidente e quais garantias reais de resiliencia este ciclo precisa entregar.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | fechar a matriz de ownership entre `validator`, `server`, `configctl` e futuro `alerter` | `cluster-architect` | pending | ownership matrix |
| 1.2 | definir a semantica canonica de `ValidationResult` e `ValidationIncident` | `cluster-architect` + `contract-guardian` | pending | contract freeze |
| 1.3 | decidir a promessa minima de retention, replay e restart para resultados, incidentes e loaded-state | `runtime-validator` + `cluster-architect` | pending | resilience freeze |
| 1.4 | fixar a escada de validacao e os cenarios obrigatorios | `tdd-coordinator` + `runtime-validator` | pending | validation ladder |

**Acceptance Criteria**

- resultado e incidente possuem propositos diferentes e nao competem pela mesma semantica
- nenhum campo ou comportamento de alerting entra neste bloco por conveniencia
- as alegacoes de idempotencia e restart ficam explicitas antes da implementacao

---

### Phase 2 - Validation Result And Incident Contracts
> **Primary Agent:** `contract-guardian` - [Playbook](../agents/contract-guardian.md)

**Objective:** consolidar `ValidationResult` como contrato tecnico estavel e introduzir `ValidationIncident` como contrato operacional minimo, com seams claros de armazenamento, consulta e consumo posterior.

**Front 1: Result Contract Consolidation**

- manter `ValidationResult` focado em avaliacao tecnica por mensagem, com identidade, binding, scope, config/runtime e violacoes
- tornar explicita a chave deterministica usada para processamento idempotente e para diferenciacao entre redelivery, replay util e nova mensagem legitima
- revisar se `ValidationResult` precisa expor referencia util para incidente sem absorver lifecycle operacional

**Front 2: Incident Contract Introduction**

- definir `ValidationIncident` como signal operacional minimo, por exemplo com `incident_key`, `kind`, `scope`, `binding`, `status`, `first_seen_at`, `last_seen_at`, `count` e referencia de evidencia
- separar incidentes de falha de regra, falha de runtime carregado e outras anomalias tecnicas sem introduzir policy de notificacao
- manter a semantica simples: incidente representa problema operacional observavel, nao workflow de resposta

**Front 3: Query And Consumption Seams**

- preservar `validator.results.list` e `/runtime/validator/results`
- adicionar superfice minima e aditiva para incidentes, preferencialmente `validator.incidents.list` e `/runtime/validator/incidents`
- decidir a seam de consumo posterior do futuro `alerter` de forma explicita e additive, sem acoplar o consumidor ao estado interno do actor

**Front 4: Small Operational Read Models**

- manter resultados e incidentes em read models pequenos, previsiveis e explicitamente limitados
- evitar duplicar store, responder e regra de limpeza de modo disperso entre actors
- deixar claro se a estrategia de restart e reconstruir, reaproveitar ou persistir estes read models

**Acceptance Criteria**

- `ValidationResult` e `ValidationIncident` ficam separados por contrato e por responsabilidade
- consultas minimas via `server` continuam finas e coerentes com os responders do `validator`
- o futuro `alerter` passa a depender de contrato explicito, nao de detalhe interno do runtime

---

### Phase 3 - Operational Resilience Hardening
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** endurecer o motor para comportamento previsivel sob redelivery, restart, reload e degradacao operacional real.

**Front 1: Idempotence And Replay**

- substituir a deduplicacao fraca baseada apenas em `MessageID` por uma chave de processamento mais defensavel
- garantir que replay e redelivery nao multipliquem resultados ou incidentes sem necessidade
- definir a relacao entre idempotencia do resultado tecnico e consolidacao do incidente operacional

**Front 2: Restart And Bootstrap Behavior**

- tornar explicito o que o `validator` reconstrui a partir de `configctl`, o que deriva do JetStream e o que precisa sobreviver a restart para manter previsibilidade
- endurecer o bootstrap de runtime, results store e incidents store para que o supervisor volte com estado coerente
- evitar wiring fragil ou codigo morto que esconda dependencia critica de ordem de startup

**Front 3: Reload, Readiness And Topology**

- observar `consumer`, `validator` e seus actors como runtime critico, com topologia mais explicita
- alinhar readiness e health ao estado real de runtime carregado, consulta disponivel e caminho de validacao utilizavel
- garantir que reload de runtime nao deixe bindings ativos com loaded-state stale sem sinal diagnostico claro

**Front 4: Diagnostics And Observability**

- expor sinais minimos de diagnostico para resultado, incidente, dedupe, reload e ultimo processamento util
- ampliar `results-inspect`, `trace-pack` e, se necessario, uma superficie equivalente para incidentes
- manter a observabilidade util sem criar um subsistema paralelo de telemetria improvisada

**Front 5: Smoke And Operational Ritual**

- fortalecer a prova com `invalid-payload`, `happy-path`, `readiness-probe` e um corte de restart/recovery se o gap real exigir
- consolidar o ritual operador-dev para `make ps`, `make logs`, `make results-inspect`, `make trace-pack` e consultas via `server`
- tratar falha silenciosa, redelivery repetido e reload inconsistente como bugs de runtime, nao como ruido operacional aceitavel

**Acceptance Criteria**

- restart e redelivery nao geram estado ambiguo nem efeito cascata em incidentes
- readiness, diagnose e consultas contam a mesma historia sobre o estado do runtime
- a topologia critica fica explicita e sem wiring sobrando

---

### Phase 4 - Runtime Proof And Canonical Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar a separacao de contratos e a nova robustez operacional no workflow real do repositorio, depois consolidar a mudanca no `.context`.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 4.1 | rodar baseline estatica e testes focados | `tdd-coordinator` + `runtime-validator` | pending | `make check`, testes Go, analyzers |
| 4.2 | provar o runtime com smoke, deep gate e diagnostico | `runtime-validator` + `cli-evolution` | pending | `scenario-smoke`, `check-deep`, `results-inspect`, `trace-pack` |
| 4.3 | alinhar docs, contracts e workflow | `documentation-writer` + `contract-guardian` | pending | docs canonicas + tracking |

**Acceptance Criteria**

- os contratos de resultado e incidente ficam provados no runtime real, nao so em testes locais
- as regras de restart, reload e idempotencia tem evidencias repetiveis
- docs, analyzers, smoke e query surfaces contam a mesma historia

## Validation Ladder

**Before**

- `make check`
- `make tdd`
- `make recommend`
- se tocar `tools/raccoon-cli`, tambem `make raccoon-test`

**During**

- testes Go focados em `internal/application/validatorresults`, `internal/actors/scopes/validator`, `internal/interfaces/http/handlers` e responders/gateways afetados
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `raccoon-cli arch-guard`
- `raccoon-cli drift-detect`

**After**

- `make verify`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=readiness-probe`
- se houver corte explicito de restart/recovery, rodar o cenario dedicado ou prova equivalente documentada
- `make check-deep`
- `make results-inspect`
- `make trace-pack`
- se tocar `tools/raccoon-cli`, tambem `make quality-gate-ci`

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B56-P1-S1 | fechar ownership matrix entre resultado, incidente e alerting futuro | `cluster-architect` | nenhuma | matrix e decisao registrada |
| B56-P1-S2 | congelar schema minimo e relacao entre `ValidationResult` e `ValidationIncident` | `contract-guardian` | `B56-P1-S1` | contract freeze |
| B56-P1-S3 | decidir retention, replay e restart semantics | `runtime-validator` | `B56-P1-S2` | resilience freeze |
| B56-P1-S4 | fechar validation ladder e smoke obrigatorio | `tdd-coordinator` | `B56-P1-S3` | before/after plan |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B56-P2-S1 | consolidar o contrato de resultado e sua chave deterministica de processamento | `contract-guardian` | `B56-P1-S2` | contracts e testes |
| B56-P2-S2 | introduzir `ValidationIncident` e suas regras minimas de derivacao | `feature-developer` + `contract-guardian` | `B56-P2-S1` | schema e store inicial |
| B56-P2-S3 | adicionar seams minimas de consulta e consumo posterior | `feature-developer` | `B56-P2-S2` | responder, route e docs |
| B56-P2-S4 | manter read models pequenos e coerentes com restart semantics | `cluster-architect` | `B56-P2-S3` | wiring e limits explicitos |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B56-P3-S1 | endurecer idempotencia, replay e redelivery | `runtime-validator` + `test-writer` | `B56-P2-S1` | testes e diagnosticos |
| B56-P3-S2 | endurecer bootstrap, restart e reload dos actors criticos | `cluster-architect` + `runtime-validator` | `B56-P2-S4` | startup path coerente |
| B56-P3-S3 | alinhar readiness, query e loaded-state ao estado real | `runtime-validator` | `B56-P3-S2` | readiness evidence |
| B56-P3-S4 | ampliar observabilidade e troubleshooting para resultados e incidentes | `cli-evolution` + `runtime-validator` | `B56-P3-S3` | inspect e trace-pack melhores |

### Phase 4 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B56-P4-S1 | rodar guard rails e testes focados | `runtime-validator` | todos os steps da phase 3 | logs de verificacao |
| B56-P4-S2 | rodar smoke, diagnostico e deep gate | `runtime-validator` + `cli-evolution` | `B56-P4-S1` | evidence pack |
| B56-P4-S3 | atualizar docs, artifacts e tracking | `documentation-writer` | `B56-P4-S2` | docs canonicas e plan tracking |

## Done Definition

Este plano so termina quando:

- `ValidationResult` e `ValidationIncident` estiverem separados por contrato, responsabilidade e consulta;
- o `validator` continuar simples: valida, materializa resultado e sinaliza incidente minimo, sem virar alerter;
- idempotencia, restart, reload e readiness tiverem comportamento explicito e provado;
- `raccoon-cli`, smoke, docs e queries via `server` refletirem a mesma verdade operacional.

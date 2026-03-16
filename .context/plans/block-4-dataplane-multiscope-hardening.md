---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Definir o seam de multiscope no dataplane sem reabrir ownership já estabilizado em configctl e validator"
  - type: "cli-evolution"
    role: "Usar o raccoon-cli como motor de prova e diagnóstico do novo comportamento"
  - type: "runtime-validator"
    role: "Provar em cluster real que consumer e emulator deixam de depender de um único scope fixo"
  - type: "contract-guardian"
    role: "Proteger contracts HTTP, bootstrap e bindings enquanto o dataplane evolui"
  - type: "tdd-coordinator"
    role: "Fechar a menor escada de validação para changes cross-layer em bootstrap e runtime"
  - type: "documentation-writer"
    role: "Atualizar docs canônicas e workflow vivo com a nova verdade operacional"
docs:
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze & Multiscope Boundaries"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Bootstrap And Dataplane Runtime Hardening"
    prevc: "E"
    agent: "cluster-architect"
  - id: "phase-3"
    name: "Runtime Proof & Workflow Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Block 4 - Dataplane Multiscope Hardening Plan

> Hardening cirúrgico do dataplane para que `consumer` e `emulator` deixem de depender do snapshot único em `global/default` e passem a acompanhar o conjunto ativo de scopes sem regredir o `raccoon-cli` como motor de qualidade.

## Task Snapshot

- **Primary goal:** introduzir um modelo explícito de bootstrap e refresh de runtime para `consumer` e `emulator` compatível com múltiplos scopes ativos, preservando o contrato atual de `configctl`, o cache multiscope do `validator` e a API pública do `server`.
- **Success signal:** o dataplane consegue refletir mais de um scope ativo sem exigir reconfiguração manual por processo, e o `raccoon-cli` consegue provar isso com smoke e checks de bindings sem inventar contratos paralelos.
- **Out of scope:** persistência nova, redesenho de `configctl`, mudança do ownership de contracts do `validator`, paralelismo irrestrito de smoke ou troca do baseline local `global/default` como comportamento default de operador.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 3 Smoke Isolation Freeze](./block-3-smoke-isolation-freeze.md)

## Codebase Context

### Current Diagnostic

- `configctl` já projeta runtimes por scope e expõe bindings ativos por `scope_kind/scope_key`, além de listagem agregada de runtime projections em `internal/application/configctl/list_active_runtime_projections.go`.
- `validator` já opera com cache multiscope e bootstrap híbrido via `configctl`, então a camada de leitura/runtime query não é o gargalo deste bloco.
- `runtimebootstrap` já expõe bootstrap agregado explícito além do caminho por scope, sem API nova.
- `consumer` já sobe pelo bootstrap agregado e faz refresh contínuo por polling, trocando a runtime só quando a assinatura do conjunto de bindings muda.
- `emulator` já usa o mesmo seam agregado no startup e no loop de refresh do snapshot de bindings.
- `deploy/configs/consumer.jsonc` e `deploy/configs/emulator.jsonc` ainda mantêm `global/default` como baseline simples de operador, mas o dataplane não depende mais exclusivamente desse scope para funcionar.
- `raccoon-cli runtime-bindings` está verde e deve continuar sendo o guard rail estático para provar que as mudanças não quebram o mapeamento config -> kafka -> jetstream -> validator.

### Preserve These Decisions

- `server` continua sendo a superfície pública do runtime; não criar endpoint operacional ad hoc só para o Bloco 4.
- `configctl` segue como fonte de verdade do lifecycle e das projeções ativas.
- `validator` continua separado do dataplane e já pode ser tratado como referência do desenho multiscope correto.
- `runtimebootstrap.Client` continua sendo o seam externo do dataplane com o cluster; o bloco deve ampliar sua utilidade, não removê-lo.
- o baseline `global/default` continua existindo como default operacional local, mas deixa de ser a única forma suportada de bootstrap do dataplane.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | fechar o desenho multiscope do dataplane sem reabrir os blocos 1-3 | [cluster-architect](../agents/cluster-architect.md) | ownership entre `consumer`, `emulator`, `runtimebootstrap` e configs |
| Contract Guardian | proteger contracts de bindings, runtime query e eventos | [contract-guardian](../agents/contract-guardian.md) | evitar drift entre listagem agregada e consultas por scope |
| CLI Evolution | manter o `raccoon-cli` como prova canônica do bloco | [cli-evolution](../agents/cli-evolution.md) | `runtime-bindings`, `recommend`, smoke e diagnósticos |
| Runtime Validator | provar comportamento real no cluster | [runtime-validator](../agents/runtime-validator.md) | smoke sequencial, trace-pack e results inspection |
| TDD Coordinator | definir a escada mínima de testes antes de alterar bootstrap/runtime | [tdd-coordinator](../agents/tdd-coordinator.md) | `go test` focado + `make verify` + smoke certo |
| Documentation Writer | refletir a nova verdade operacional | [documentation-writer](../agents/documentation-writer.md) | docs canônicas e workflow de manutenção |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | registrar que `consumer` e `emulator` deixam de ser bootstrapped por um único scope fixo |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | documentar quando multiscope exige escalada para `check-deep` e smoke específicos |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | alinhar o contrato de bindings ativos e a relação entre scope, binding e roteamento |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | deixar claro como o CLI prova a paridade multiscope do dataplane |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| ampliar bootstrap sem freeze claro acabar duplicando ownership já resolvido em `configctl` | Medium | High | usar `configctl` como única fonte de verdade e restringir o bloco ao dataplane | `cluster-architect` |
| `consumer` passar a observar vários scopes e duplicar consumo/publicação para o mesmo topic | Medium | High | congelar invariantes do `BindingIndex` e provar unicidade por topic/scope/binding em testes e smoke | `contract-guardian` |
| `emulator` continuar publicando só para o snapshot inicial e mascarar falsos verdes | High | Medium | endurecer refresh/reload explícito e validar com smoke real | `runtime-validator` |
| o `raccoon-cli` continuar assumindo baseline simples e não conseguir provar o novo desenho | Medium | Medium | usar `runtime-bindings`, `recommend` e smoke como critérios de aceite do bloco | `cli-evolution` |
| mudança cross-layer gerar regressão silenciosa de startup | Medium | High | exigir `make verify`, `make check-deep` e `scenario-smoke` após cada frente | `tdd-coordinator` |

### Dependencies

- **Internal:** `internal/actors/scopes/consumer/*`, `cmd/emulator/run.go`, `internal/application/runtimebootstrap/*`, `internal/application/dataplane/bootstrap.go`, `deploy/configs/consumer.jsonc`, `deploy/configs/emulator.jsonc`, `tools/raccoon-cli/src/smoke/*`
- **External:** NATS, Kafka e Compose locais saudáveis para prova final
- **Technical:** preservar o contrato atual de `runtime/ingestion/bindings` e das projeções ativas do `configctl`

### Assumptions

- a listagem agregada de runtimes ativos já existente em `configctl` é suficiente para ancorar um bootstrap multiscope do dataplane sem criar API nova.
- `consumer` e `emulator` podem evoluir de snapshot único para conjunto reconciliado de bindings sem exigir persistência fora do processo.
- se a reconciliação contínua se mostrar grande demais, o bloco ainda precisa entregar ao menos bootstrap multiscope determinístico por startup, com refresh explícito planejado e documentado.

## Working Phases

### Phase 1 - Freeze & Multiscope Boundaries
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar o desenho alvo do dataplane multiscope e separar claramente o que muda no bloco do que permanece estável.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | inventariar como `consumer` e `emulator` resolvem bootstrap, refresh e lifecycle hoje | `cluster-architect` | completed | freeze com pontos single-scope reais |
| 1.2 | decidir qual será a fonte de bootstrap multiscope do dataplane | `cluster-architect` + `contract-guardian` | completed | freeze do seam canônico |
| 1.3 | fechar a escada de validação com `raccoon-cli` antes de implementação | `tdd-coordinator` + `cli-evolution` | completed | baseline de checks e smoke |

**Acceptance Criteria**

- fica explícito por que o runtime atual ainda depende de `global/default`
- o bloco tem seam canônico definido antes de qualquer edição de código
- os limites com `configctl`, `validator` e `raccoon-cli` ficam documentados

---

### Phase 2 - Bootstrap And Dataplane Runtime Hardening
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** introduzir suporte operacional a multiscope no dataplane com mudanças mínimas, testáveis e sem reabrir os contracts estabilizados.

**Front 1: Bootstrap Source**

- avaliar se `runtimebootstrap.Client` deve ganhar listagem agregada de bindings/runtimes ou se o `consumer`/`emulator` devem iterar scopes ativos a partir de projections já disponíveis
- preservar o caminho atual por scope para casos pontuais e troubleshooting

**Front 2: Consumer Runtime Model**

- trocar o modelo “um snapshot, uma geração” por um runtime capaz de representar múltiplos scopes ativos sem conflitar no mesmo topic
- manter deduplicação e roteamento corretos por binding/scope
- garantir que restart continue determinístico

**Front 3: Emulator Parity**

- fazer o `emulator` acompanhar o mesmo conjunto efetivo de bindings do `consumer`
- impedir que ele publique indefinidamente sobre um snapshot stale quando novas ativações entrarem ou saírem

**Front 4: Config And Operator Surface**

- revisar `deploy/configs/consumer.jsonc` e `deploy/configs/emulator.jsonc` para que o default local continue simples, mas sem travar o desenho em um único scope
- manter troubleshooting claro para operador local

**Acceptance Criteria**

- bootstrap do dataplane deixa de depender de um único scope hardcoded por processo
- `consumer` e `emulator` convergem para a mesma visão ativa de bindings
- os contracts públicos permanecem consistentes e o baseline `global/default` continua funcionando

---

### Phase 3 - Runtime Proof & Workflow Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar em runtime que o dataplane endurecido continua correto e atualizar o contexto operacional do repositório.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | rodar baseline estática e testes focados após as mudanças | `runtime-validator` + `tdd-coordinator` | completed | `go test`, `make verify`, `runtime-bindings`, `recommend` |
| 3.2 | provar o comportamento do dataplane com smoke e evidência de bindings/results | `runtime-validator` + `cli-evolution` | completed | smoke, `results-inspect`, `trace-pack` se necessário |
| 3.3 | atualizar docs e workflow de manutenção do `.context` | `documentation-writer` | completed | docs canônicas e artefato de validação |

**Acceptance Criteria**

- o `raccoon-cli` continua sendo o caminho canônico para provar o bloco
- a prova final mostra que o dataplane não está mais preso a um único snapshot scope-fixed
- docs e `.context` refletem a nova verdade operacional

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B4-P1-S1 | congelar o diagnóstico single-scope de `consumer` e `emulator` | `cluster-architect` | nenhuma | freeze doc |
| B4-P1-S2 | decidir seam canônico de bootstrap multiscope | `cluster-architect` + `contract-guardian` | `B4-P1-S1` | decisão registrada |
| B4-P1-S3 | fechar baseline de validação com `raccoon-cli` | `tdd-coordinator` + `cli-evolution` | `B4-P1-S2` | ladder before/after |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B4-P2-S1 | ampliar `runtimebootstrap` para suportar a visão de bootstrap necessária | `feature-developer` + `contract-guardian` | `B4-P1-S2` | API interna clara e testada |
| B4-P2-S2 | refatorar `consumer` para construir runtime sobre o conjunto ativo de bindings | `cluster-architect` | `B4-P2-S1` | supervisor/runtime revisados |
| B4-P2-S3 | alinhar `emulator` com o novo modelo de bootstrap | `feature-developer` | `B4-P2-S1` | emulator sem snapshot stale |
| B4-P2-S4 | revisar configs e guard rails do CLI para o novo comportamento | `cli-evolution` | `B4-P2-S2`, `B4-P2-S3` | `runtime-bindings`, `recommend` e smoke coerentes |
| B4-P2-S5 | fechar testes de unidade e integração local do bloco | `test-writer` + `tdd-coordinator` | `B4-P2-S2`, `B4-P2-S3` | cobertura focada em bootstrap, reconciliação e deduplicação |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B4-P3-S1 | rodar baseline estática e `verify` após o hardening | `runtime-validator` | todos os steps da phase 2 | logs de verificação |
| B4-P3-S2 | rodar smoke e inspeção de results no cluster vivo | `runtime-validator` + `cli-evolution` | `B4-P3-S1` | smoke + evidence bundle |
| B4-P3-S3 | atualizar docs canônicas e tracking do `.context` | `documentation-writer` | `B4-P3-S2` | docs + plano atualizado |

## Validation Ladder

**Before**

- `raccoon-cli recommend internal/actors/scopes/consumer/bootstrap_actor.go internal/actors/scopes/consumer/runtime.go internal/actors/scopes/consumer/supervisor.go internal/application/runtimebootstrap/client.go internal/application/dataplane/bootstrap.go`
- `raccoon-cli runtime-bindings`
- `raccoon-cli arch-guard`
- `raccoon-cli contract-audit`

**During**

- `go test ./internal/application/runtimebootstrap ./internal/application/dataplane ./internal/actors/scopes/consumer`
- `go test ./cmd/emulator`
- `make verify`

**After**

- `raccoon-cli runtime-bindings`
- `raccoon-cli topology-doctor`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

## Done Definition

Este bloco só termina quando:

- o gap entre runtime multiscope de `configctl`/`validator` e dataplane de `consumer`/`emulator` estiver fechado ou explicitamente reduzido ao seam documentado
- o dataplane não depender mais apenas de `bootstrap.scope_kind=global` e `bootstrap.scope_key=default` para funcionar corretamente
- `raccoon-cli` continuar provando o comportamento real sem checks paralelos improvisados
- o `.context` refletir a nova verdade operacional e deixar claro como manter o contexto vivo a partir daí

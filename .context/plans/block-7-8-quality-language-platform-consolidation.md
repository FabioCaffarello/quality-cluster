---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Congelar os limites de ownership e impedir que a DSL ou a plataforma crescam mais rapido que o runtime"
  - type: "contract-guardian"
    role: "Definir a evolucao aditiva dos contracts de DSL, projection, query e runtime sem drift silencioso"
  - type: "runtime-validator"
    role: "Provar que regras mais ricas continuam executaveis, observaveis e governaveis no cluster real"
  - type: "cli-evolution"
    role: "Transformar a consolidacao da plataforma em guard rails, diagnosticos e feedback loops acionaveis"
  - type: "tdd-coordinator"
    role: "Fechar a escada minima de testes, smoke e gates para cada corte de evolucao"
  - type: "documentation-writer"
    role: "Consolidar a nova verdade do bloco no contexto vivo e no workflow canonico"
docs:
  - "project-overview.md"
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
  - "raccoon-cli-role.md"
phases:
  - id: "phase-1"
    name: "Freeze Language And Ownership Boundaries"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Expand DSL And Runtime Contracts Carefully"
    prevc: "E"
    agent: "contract-guardian"
  - id: "phase-3"
    name: "Consolidate Platform Guard Rails And Diagnostics"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-4"
    name: "Runtime Proof And Canonical Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Blocks 7-8 - Quality Language Expansion And Platform Consolidation Plan

> Este plano segue a numeracao pedida nesta conversa e nao substitui os blocos 7 e 8 historicos ja existentes em `.context/plans/`.

> Expandir a linguagem de qualidade e consolidar a plataforma de engenharia sem mover ownership para fora de `configctl`, sem transformar `validator` ou `consumer` em autores de config, e sem abrir espaco prematuro para complexidade stateful, correlacao excessiva ou governanca paralela.

## Task Snapshot

- **Primary goal:** permitir evolucao controlada da DSL/config, das projections e dos contracts de runtime enquanto o repositorio ganha explicabilidade, auditoria, guard rails e feedback loops mais fortes via `raccoon-cli`, smoke e diagnosticos canonicos.
- **Success signal:** `configctl` continua dono de draft, validate, compile, activate e da semantica da linguagem; `validator` continua executando apenas runtime compilado e expondo resultados operacionais coerentes; `consumer` e `emulator` permanecem clientes do bootstrap de runtime; `make check`, `make tdd`, `make verify`, `make check-deep`, `make scenario-smoke`, `make trace-pack` e `make results-inspect` continuam suficientes para governar a evolucao.
- **Out of scope:** introduzir regras stateful ou correlacao multi-mensagem, transformar a DSL em linguagem geral de expressoes arbitrarias, empurrar autoria ou compilacao de config para `validator`/`consumer`, criar um segundo source of truth no `raccoon-cli`, ou abrir um subsistema de policy/alerting fora do corte real do runtime.
- **Key references:**
  - [Project Overview](../docs/project-overview.md)
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Raccoon CLI Role](../docs/raccoon-cli-role.md)
  - [Block 5-6 Validation Results Incidents Runtime Resilience](./block-5-6-validation-results-incidents-runtime-resilience.md)
  - [Block 9 Refresh Health Classification](./block-9-refresh-health-classification.md)
  - [Block 10 Refresh Degradation Diagnostics](./block-10-refresh-degradation-diagnostics.md)

## Codebase Context

### Current Diagnostic

- `internal/domain/configctl/document.go` hoje define uma DSL pequena e explicita: `metadata`, `bindings`, `fields` e `rules`, com operadores `required`, `not_empty` e `equals`. Esse corte e simples o bastante para o runtime executar sem ambiguidade, mas ainda e estreito para a proxima camada de linguagem.
- `internal/application/configctl/compile_config.go` e `internal/domain/configctl/runtime.go` ja deixam `configctl` como owner do lifecycle e da compilacao, incluindo `schema_version`, `runtime_loader`, `compiler_version`, checksum e artifact metadata. O bloco precisa endurecer esse seam, nao desloca-lo.
- `internal/application/configctl/mappers.go` e os contracts em `internal/application/configctl/contracts/` ja projetam `ConfigVersionDetail`, `RuntimeProjectionRecord` e `ActiveIngestionBindingRecord`, o que cria base para projections mais maduras sem inventar ownership novo.
- `internal/application/validatorresults/evaluate.go` executa o runtime projetado em cima de payload JSON e de `RuntimeProjection.Rules`; o `validator` hoje segue runtime-only e nao deve virar interpretador de autoria, migracao de DSL ou compilacao.
- `internal/actors/scopes/validator/runtime_cache.go` e os responders de runtime/resultados/incidentes ja demonstram o padrao desejado: loaded-state no `validator`, truth no `configctl`, e consultas finas via `server`.
- `tools/raccoon-cli/src/analyzers/*`, `tools/raccoon-cli/src/smoke/*`, `tools/raccoon-cli/src/results_inspect/*` e `tools/raccoon-cli/src/trace_pack/*` ja formam um control plane de qualidade capaz de suportar guard rails de linguagem, drift, contracts, proof e diagnostico, desde que as novas invariantes sejam explicitadas e nao inferidas no escuro.

### Preserve These Decisions

- `configctl` continua owner da autoria, validacao, compilacao e ativacao da config e da linguagem de qualidade.
- `validator` continua owner apenas de runtime loaded-state, execucao de regras compiladas, resultados e incidentes operacionais pequenos.
- `consumer` e `emulator` continuam clientes de bootstrap/projection; eles nao inferem nem alteram a semantica da DSL.
- `server` continua facade HTTP fina sobre NATS/contracts; ele nao vira owner de regra, projection ou governance.
- `raccoon-cli` continua observador e guard rail externo; ele nao substitui runtime truth nem vira interpretador secreto da linguagem.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | congelar limites entre linguagem, runtime e plataforma | [cluster-architect](../agents/cluster-architect.md) | proteger ownership de `configctl`, `validator`, `consumer`, `server` e `raccoon-cli` |
| Contract Guardian | evoluir contracts de DSL, projection e query sem quebrar consumidores | [contract-guardian](../agents/contract-guardian.md) | adicao versionada e traceavel de surfaces |
| Runtime Validator | provar que a evolucao continua executavel e inspecionavel | [runtime-validator](../agents/runtime-validator.md) | runtime proof, query surfaces, smoke e evidencia |
| CLI Evolution | transformar invariantes novas em checks e diagnosticos reais | [cli-evolution](../agents/cli-evolution.md) | analyzers, `trace-pack`, `results-inspect`, `recommend` |
| TDD Coordinator | calibrar a ladder certa por blast radius | [tdd-coordinator](../agents/tdd-coordinator.md) | testes focados antes de smoke/deep |
| Documentation Writer | atualizar docs, plano e workflow canonico | [documentation-writer](../agents/documentation-writer.md) | contexto vivo e criterio de done |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | registrar a nova etapa em que linguagem e plataforma evoluem juntas sem romper ownership |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | explicitar que `configctl` continua author/compile owner e que `validator` executa runtime compilado mais rico, nao autoria bruta |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | fixar quando evolucao de DSL/projection/contracts exige verify, smoke, deep gate, baseline e diagnostico |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | documentar surfaces de projection, result, incident e event/query contracts amadurecidos |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar analyzers, proof e traceability a linguagem mais rica e a plataforma mais auditavel |
| Raccoon CLI Role | [raccoon-cli-role.md](../docs/raccoon-cli-role.md) | consolidar o CLI como plano de engenharia da plataforma, nao utilitario acessorio |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| DSL crescer mais rapido que o runtime executa e diagnostica | Medium | High | liberar apenas operadores e expressoes que tenham semantica compilavel, prova e observabilidade no mesmo bloco | `cluster-architect` + `runtime-validator` |
| `validator` ou `consumer` absorverem semantica de autoria por conveniencia | Medium | High | congelar `configctl` como owner da linguagem e manter consumidores presos a runtime/projection compilada | `cluster-architect` |
| contracts de projection e query variarem sem versionamento ou sem compatibilidade aditiva | Medium | High | evolucao version-aware em `configctl/contracts`, responders e analyzers com coverage explicita | `contract-guardian` |
| `raccoon-cli` virar arquitetura paralela e especulativa | Low | High | toda regra nova no CLI precisa apontar para invariantes reais do runtime, docs e configs | `cli-evolution` |
| ganho de diagnostico gerar ruido em vez de feedback util | Medium | Medium | priorizar explainability acionavel em `trace-pack`, `results-inspect`, `recommend` e smoke, evitando telemetria ornamental | `cli-evolution` + `runtime-validator` |

### Dependencies

- **Internal:** `internal/domain/configctl/document.go`, `internal/domain/configctl/runtime.go`, `internal/application/configctl/compile_config.go`, `internal/application/configctl/mappers.go`, `internal/application/configctl/contracts/*`, `internal/application/validatorresults/*`, `internal/actors/scopes/validator/*`, `internal/interfaces/http/handlers/configctl.go`, `internal/interfaces/http/handlers/runtime.go`, `tools/raccoon-cli/src/analyzers/*`, `tools/raccoon-cli/src/smoke/*`, `tools/raccoon-cli/src/results_inspect/*`, `tools/raccoon-cli/src/trace_pack/*`
- **External:** NATS, Kafka, Docker Compose e o cluster local seguem sendo a prova final para runtime-significant work
- **Technical:** a evolucao da linguagem precisa continuar expressavel em projections e em payloads de resultado/incidente sem obrigar state stores, correlacao temporal ou runtime authoring fora do `configctl`

### Assumptions

- a proxima evolucao segura da linguagem e aditiva e fortemente tipada, nao uma reabertura completa da DSL.
- projections e contracts mais maduros podem crescer sem inflar o dominio se forem tratados como seams compilados e observaveis.
- consolidacao de plataforma significa reduzir drift e tempo de diagnostico, nao criar mais uma camada de abstracao sem owner.
- o bloco precisa preparar expansao funcional posterior sem deixar legado difuso em docs, runtime e tooling.

## Working Phases

### Phase 1 - Freeze Language And Ownership Boundaries
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar o envelope da evolucao antes de ampliar DSL, projections, contracts ou guard rails.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | fechar a matriz de ownership entre `configctl`, `validator`, `consumer`, `server` e `raccoon-cli` para a proxima etapa de linguagem | `cluster-architect` | completed | ownership matrix |
| 1.2 | definir o envelope permitido da DSL: operadores, expressoes e metadata que podem entrar sem abrir statefulness nem correlacao | `cluster-architect` + `contract-guardian` | completed | language freeze |
| 1.3 | fixar os contracts minimos de projection/query/result que precisam amadurecer junto com a DSL | `contract-guardian` | completed | contract freeze |
| 1.4 | fechar a ladder de validacao, smoke, baseline e evidencias obrigatorias | `tdd-coordinator` + `runtime-validator` | completed | validation ladder |

**Acceptance Criteria**

- o bloco comeca com ownership explicito e sem ambiguidade entre autoria, compilacao, execucao e observacao;
- a linguagem so pode crescer dentro de um envelope declarativo e per-record;
- toda ampliacao prevista ja aponta para uma prova real no runtime e no CLI.

---

### Phase 2 - Expand DSL And Runtime Contracts Carefully
> **Primary Agent:** `contract-guardian` - [Playbook](../agents/contract-guardian.md)

**Objective:** evoluir a linguagem de qualidade e as projections de runtime de forma versionada, explicavel e operacionalmente simples.

**Front 1: DSL Surface Discipline**

- tratar `ConfigDocument`, `Rule`, `Field` e metadata como contrato versionado, com crescimento aditivo e compativel;
- priorizar operadores e expressoes pequenas que continuem deterministicas por payload, por exemplo comparacoes adicionais, checks tipados e metadata de severidade/explicacao, sem abrir linguagem geral;
- tornar explicita a relacao entre `schema_version`, `runtime_loader`, `compiler_version` e o conjunto de capacidades suportadas pelo runtime.

**Front 2: Compilation And Projection Maturity**

- fazer `configctl` compilar a autoria em projections e artifacts que carreguem contexto suficiente para execucao, explainability e auditoria;
- amadurecer `RuntimeProjectionRecord`, `ActiveIngestionBindingRecord` e surfaces correlatas para expor o que o runtime realmente precisa e o que o operador realmente precisa inspecionar;
- evitar que `validator` ou `consumer` tenham de reinterpretar a config bruta para completar semantica faltante.

**Front 3: Runtime Execution Contract**

- manter `validator` como executor de runtime compilado, com superficie de execucao clara para operadores suportados e falhas explicitas para o que ainda nao e suportado;
- amadurecer `ValidationResultRecord` e `ValidationIncidentRecord` apenas no que melhora explainability e estabilidade contratual da nova linguagem;
- garantir que projections, resultados e incidentes preservem provenance util de artifact/config/rule sem inflar o payload operacional.

**Front 4: Compatibility And Migration**

- tratar toda ampliacao da DSL como adicao compativel ou como capacidade versionada, nunca como mutacao silenciosa de semantica;
- usar fixtures, snapshots e analyzers para capturar drift entre fonte, projection, responder e docs;
- definir desde ja como uma feature da linguagem e introduzida, provada e congelada antes de liberar a proxima.

**Acceptance Criteria**

- novas capacidades da DSL continuam pequenas, declarativas e compilaveis;
- `configctl` segue dono da semantica compilada e dos artifacts;
- `validator` executa projections mais ricas sem absorver autoria;
- contracts de projection, result e incident permanecem estaveis e aditivos.

---

### Phase 3 - Consolidate Platform Guard Rails And Diagnostics
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** transformar a nova etapa da linguagem em uma plataforma robusta de desenvolvimento e operacao, com feedback loops mais curtos e menos drift arquitetural.

**Front 1: Raccoon CLI As Engineering Control Plane**

- ampliar analyzers como `contract-audit`, `runtime-bindings`, `drift-detect`, `baseline-drift`, `recommend` e `coverage-map` para refletir as novas invariantes de DSL/projection/runtime;
- fazer o CLI explicar quais capacidades de linguagem estao congeladas, quais surfaces mudaram e qual prova adicional e necessaria;
- manter os checks ancorados em verdade do repositorio, nao em regras genericas ou heuristicas sem ownership.

**Front 2: Canonical Smoke, Explainability And Diagnostics**

- amadurecer `scenario-smoke`, `results-inspect` e `trace-pack` para inspecionar projections, loaded runtime, suportes de regra, evidencias de falha e drift de artifact/config;
- tornar o runtime e o deploy mais inspecionaveis sem depender de leitura manual dispersa de logs;
- preservar a regra de que falha silenciosa, explicacao pobre ou troubleshooting ad hoc sao deficits de plataforma, nao custo aceitavel de operacao.

**Front 3: Governance And Drift Prevention**

- consolidar baselines, snapshots, diff semantico e docs como parte do plano de engenharia da plataforma;
- reforcar `make check`, `make tdd`, `make recommend`, `make verify`, `make check-deep` e `make quality-gate-ci` como ritual oficial para evolucao de linguagem e runtime;
- reduzir drift entre code, config, compose, docs e tooling antes que a expansao funcional gere legado.

**Front 4: Developer Ergonomics And Auditability**

- deixar o caminho de desenvolvimento mais previsivel: briefing, recomendacao, prova minima, coleta de evidencia e atualizacao de baseline;
- tornar contracts, projections e diagnosticos legiveis para quem evolui o motor sem precisar reconstruir o sistema mentalmente a cada mudanca;
- preparar a base para blocos funcionais posteriores sem saturar o repositorio com regras implicitas.

**Acceptance Criteria**

- o `raccoon-cli` se torna o ponto mais curto entre mudanca e prova, nao mais uma camada para manter;
- smoke e diagnostico passam a explicar a linguagem e o runtime com mais previsibilidade;
- drift arquitetural e contratual passa a falhar mais cedo e com melhor acao corretiva.

---

### Phase 4 - Runtime Proof And Canonical Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar a evolucao conjunta de linguagem e plataforma no workflow real do repositorio e consolidar a verdade final no `.context`.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 4.1 | rodar baseline estatica e testes focados em `configctl`, `validator` e `raccoon-cli` afetados | `tdd-coordinator` + `runtime-validator` | pending | baseline verde |
| 4.2 | provar runtime e contracts com smoke, deep gate, queries e diagnosticos | `runtime-validator` + `contract-guardian` + `cli-evolution` | pending | evidence pack |
| 4.3 | atualizar docs canonicas, plano e tracking de workflow | `documentation-writer` | pending | contexto vivo alinhado |

**Acceptance Criteria**

- as novas capacidades de linguagem ficam provadas no runtime real, nao apenas em testes locais;
- o caminho de prova e troubleshooting fica menor e mais explicito do que antes do bloco;
- docs, analyzers, smoke, query surfaces e runtime contam a mesma historia.

## Validation Ladder

**Before**

- `make check`
- `make tdd`
- `make recommend`
- `make coverage-map`
- se houver baseline util do bloco, `make baseline-drift BASELINE=<baseline.json>`
- se tocar `tools/raccoon-cli`, tambem `make raccoon-test`

**During**

- testes Go focados em `internal/domain/configctl`, `internal/application/configctl`, `internal/application/validatorresults`, `internal/actors/scopes/validator`, `internal/interfaces/http/handlers`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `raccoon-cli arch-guard`
- `raccoon-cli drift-detect`
- se houver evolucao de analyzer ou diagnostico, testes Rust focados em `tools/raccoon-cli`

**After**

- `make verify`
- `make scenario-smoke SCENARIO=config-lifecycle`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make results-inspect`
- `make trace-pack`
- `make check-deep`
- se tocar `tools/raccoon-cli`, tambem `make quality-gate-ci`

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B78-P1-S1 | fechar ownership matrix para linguagem, projection, runtime, query e tooling | `cluster-architect` | nenhuma | matriz registrada |
| B78-P1-S2 | congelar envelope da DSL e o que permanece fora do bloco | `contract-guardian` | `B78-P1-S1` | language freeze |
| B78-P1-S3 | definir surfaces minimas de projection/result/incident/query que amadurecem junto com a linguagem | `contract-guardian` | `B78-P1-S2` | contract freeze |
| B78-P1-S4 | fechar a ladder de validacao, baseline e evidence pack | `tdd-coordinator` | `B78-P1-S3` | before/after plan |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B78-P2-S1 | evoluir a DSL de forma aditiva e version-aware em `configctl` | `contract-guardian` + `feature-developer` | `B78-P1-S2` | contracts, fixtures e testes |
| B78-P2-S2 | amadurecer artifact e projection metadata para explainability e compatibilidade | `cluster-architect` + `feature-developer` | `B78-P2-S1` | projections coerentes |
| B78-P2-S3 | manter `validator` como executor de runtime compilado com operadores e falhas explicitadas | `runtime-validator` | `B78-P2-S2` | runtime execution proof |
| B78-P2-S4 | alinhar result/incident/query contracts a nova linguagem sem inflar payloads | `contract-guardian` | `B78-P2-S3` | responders e queries estaveis |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B78-P3-S1 | transformar novas invariantes em analyzers e recomendacoes do `raccoon-cli` | `cli-evolution` | `B78-P2-S4` | checks acionaveis |
| B78-P3-S2 | reforcar `trace-pack`, `results-inspect` e smoke para projections e explainability de regra | `cli-evolution` + `runtime-validator` | `B78-P3-S1` | diagnostico melhor |
| B78-P3-S3 | consolidar baseline, drift e governanca de docs/config/runtime | `cli-evolution` + `documentation-writer` | `B78-P3-S2` | workflow mais previsivel |
| B78-P3-S4 | reduzir ergonomia ruim e pontos cegos no ciclo de desenvolvimento do motor | `tdd-coordinator` + `cli-evolution` | `B78-P3-S3` | feedback loop menor |

### Phase 4 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B78-P4-S1 | rodar guard rails e testes focados | `runtime-validator` | todos os steps da phase 3 | logs de verificacao |
| B78-P4-S2 | rodar smoke, diagnosticos, inspect e deep gate | `runtime-validator` + `cli-evolution` | `B78-P4-S1` | evidence pack |
| B78-P4-S3 | atualizar docs, plano e tracking | `documentation-writer` | `B78-P4-S2` | contexto canonico sincronizado |

## Done Definition

Este plano so termina quando:

- a linguagem de qualidade crescer sem quebrar o ownership de `configctl` nem obrigar runtime authoring fora dele;
- `validator`, `consumer` e `emulator` continuarem operando sobre runtime compilado e projections canonicas, nao sobre config bruta reinterpretada;
- contracts, projections, resultados e incidentes amadurecerem de forma aditiva e observavel;
- `raccoon-cli`, smoke, baselines e diagnosticos reduzirem drift e tempo de explicacao em vez de aumentar complexidade;
- o repositorio ficar melhor preparado para expansao funcional posterior sem carregar legado arquitetural escondido.

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cli-evolution"
    role: "Tornar o raccoon-cli deterministico, isolavel e confiavel como motor de qualidade"
  - type: "runtime-validator"
    role: "Provar em runtime que os cenarios continuam exercitando o cluster sem falso conflito"
  - type: "tdd-coordinator"
    role: "Definir a menor escada de validacao para mudancas no smoke engine"
  - type: "code-reviewer"
    role: "Revisar risco de acoplamento excessivo entre smoke, contracts e runtime real"
  - type: "documentation-writer"
    role: "Atualizar docs canonicas com a nova semantica operacional de smoke"
docs:
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
  - "development-workflow.md"
  - "testing-strategy.md"
phases:
  - id: "phase-1"
    name: "Identity Freeze & Scope Boundaries"
    prevc: "P"
    agent: "cli-evolution"
  - id: "phase-2"
    name: "Smoke Isolation Implementation"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Runtime Proof & Workflow Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Bloco 3 - Smoke Isolation Hardening Plan

> Tornar `runtime-smoke` e `scenario-smoke` determinísticos, isoláveis e reutilizáveis como motor de qualidade do repositório, sem falsos conflitos causados por chaves de config, bindings, correlação ou scope fixos.

## Task Snapshot

- **Primary goal:** remover o acoplamento implícito entre cenários de smoke e estado compartilhado do cluster, para que o `raccoon-cli` prove comportamento real sem depender de serialização manual ou de nomes fixos como `raccoon-smoke`.
- **Success signal:** os cenários de smoke passam com identidade própria, deixam evidência rastreável por execução e não geram `409` ou mistura de resultados por reaproveitar o mesmo config key, binding ou correlação.
- **Out of scope:** redesenho do runtime Go, mudança de ownership entre `server`, `configctl`, `consumer`, `validator` e `emulator`, ou troca dos contratos HTTP/NATS que já foram estabilizados nos blocos anteriores.
- **Key references:**
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Testing Strategy](../docs/testing-strategy.md)
  - [Block 2 Phase 3 Validation](./block-2-phase-3-validation.md)

## Codebase Context

### Current Diagnostic

- `tools/raccoon-cli/src/smoke/stages.rs` cria draft com config key fixa `raccoon-smoke`
- `tools/raccoon-cli/src/smoke/stages.rs` publica binding fixo `smoke_events` no topic `smoke.events.created`
- `tools/raccoon-cli/src/smoke/api.rs` consulta bindings, active config e results sempre em `scope_kind=global` e `scope_key=default`
- `tools/raccoon-cli/src/smoke/api.rs` gera `X-Correlation-ID` por PID, o que identifica processo mas nao separa cenarios ou execucoes concorrentes
- o Block 2 provou um falso negativo real: `happy-path` e `invalid-payload` em paralelo causam `409` no `configctl`, nao por regressao do cluster, mas por compartilharem a mesma identidade operacional de smoke

### Preserve These Decisions

- `raccoon-cli` continua como motor principal de `scenario-smoke`, `runtime-smoke`, `trace-pack` e `quality-gate`
- os cenarios continuam validando o cluster real por HTTP, Kafka e NATS; o objetivo nao e transformar smoke em mock
- `global/default` continua sendo o scope canônico do cluster local, mas o tooling nao deve depender cegamente dele quando precisar isolar execucoes
- a API pública do `server` permanece como ponto de entrada do smoke

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| CLI Evolution | desenhar e implementar identidade isolada para smoke | [cli-evolution](../agents/cli-evolution.md) | config key, binding name, topic, correlation e teardown/cleanup |
| Runtime Validator | provar que o isolamento nao degradou sinal nem cobertura real | [runtime-validator](../agents/runtime-validator.md) | `readiness-probe`, `happy-path`, `invalid-payload`, `check-deep` |
| TDD Coordinator | definir testes unitarios e cenarios necessarios antes de mexer no smoke engine | [tdd-coordinator](../agents/tdd-coordinator.md) | escada `cargo test -> topology/contract checks -> scenario-smoke` |
| Code Reviewer | evitar que a isolacao crie contratos falsos ou esconda regressao real | [code-reviewer](../agents/code-reviewer.md) | acoplamento com `configctl`, `results-inspect` e diagnósticos |
| Documentation Writer | atualizar contexto vivo e regras operacionais do smoke | [documentation-writer](../agents/documentation-writer.md) | docs canônicas e planos |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | formalizar quando smoke pode rodar em paralelo e quando precisa de identidade isolada |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | documentar identidade de execucao, isolamento e evidência |
| Development Workflow | [development-workflow.md](../docs/development-workflow.md) | ajustar a forma recomendada de usar `scenario-smoke` durante desenvolvimento |
| Testing Strategy | [testing-strategy.md](../docs/testing-strategy.md) | registrar cobertura do smoke engine e novos testes de isolamento |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| isolamento parcial resolver `config_key` mas continuar misturando results ou bindings | Medium | High | tratar identidade de smoke como pacote unico: config key, binding, topic, correlation e filtros de leitura | `cli-evolution` |
| isolamento excessivo quebrar assumptions do emulator ou das consultas de runtime/results | Medium | High | preservar o contrato do cluster e mudar primeiro o tooling, nao o runtime | `code-reviewer` |
| cleanup/teardown agressivo mascarar problemas reais de lifecycle | Low | High | preferir ids unicos por execucao antes de introduzir remocao destrutiva | `runtime-validator` |
| cobertura de teste insuficiente no smoke engine deixar regressao passar | Medium | Medium | adicionar testes unitarios em `smoke/` e prova sequencial/isolada em runtime | `tdd-coordinator` |

### Dependencies

- **Internal:** `tools/raccoon-cli/src/smoke/*`, `tools/raccoon-cli/src/trace_pack/*`, HTTP contract exposto por `server`, workflow do `Makefile`
- **External:** cluster local saudável com Docker Compose e imagens já congeladas
- **Technical:** manter `make verify`, `scenario-smoke`, `check-deep` e `results-inspect` coerentes com a nova identidade de smoke

### Assumptions

- o cluster local continua operando em `global/default` como baseline funcional
- o melhor primeiro passo é isolar o tooling, não introduzir APIs novas para cleanup
- config keys e binding names podem ser parametrizados sem quebrar o runtime Go

## Working Phases

### Phase 1 - Identity Freeze & Scope Boundaries
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** congelar a identidade operacional do smoke e o limite entre isolamento do tooling e verdade do cluster.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | inventariar todos os identificadores fixos usados por `runtime-smoke` e `scenario-smoke` | `cli-evolution` | completed | matriz de `config_key`, `binding`, `topic`, `correlation_id`, `scope`, consultas e artefatos |
| 1.2 | decidir quais identificadores passam a ser unicos por execucao e quais continuam canônicos do cluster | `cli-evolution` + `code-reviewer` | completed | freeze de identidade e limites |
| 1.3 | fechar a escada de validacao do bloco | `tdd-coordinator` | completed | baseline de `cargo test`, `make verify`, `scenario-smoke` e `check-deep` |

**Acceptance Criteria**

- conflito observado no Block 2 fica explicado por design e não por suposição
- identidade alvo de cada cenário está congelada antes de mudar código
- não há necessidade de alterar contracts Go para começar o bloco

---

### Phase 2 - Smoke Isolation Implementation
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** introduzir identidade isolada por execução no smoke engine com cobertura de teste e sem perder rastreabilidade operacional.

**Front 1: Identity Model**

- adicionar run id ou token equivalente ao `SmokeConfig`
- derivar dele `config_key`, binding name, topic derivado quando necessário e correlation id legível
- manter nomes humanos suficientes para troubleshooting e `trace-pack`

**Front 2: Query And Evidence Alignment**

- garantir que active config, bindings e validation results sejam consultados com filtros compatíveis com a identidade da execução
- evitar que um cenário leia resultados de outro ou conclua sucesso em evidência alheia
- alinhar `results-inspect` e `trace-pack` quando a nova identidade exigir filtros ou metadata melhores

**Front 3: Test Coverage**

- adicionar testes unitários para geração de identidade, naming, filtros e cenário sem colisão
- cobrir regressão de `409` por concorrência no nível possível do tooling
- atualizar testes existentes de `smoke/` para o novo modelo

**Acceptance Criteria**

- dois cenários consecutivos ou concorrentes deixam de compartilhar nomes fixos críticos
- mensagens de erro e evidência continuam legíveis para operador
- o `raccoon-cli` continua simples de usar a partir do `Makefile`

---

### Phase 3 - Runtime Proof & Workflow Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar que o smoke engine isolado continua exercitando o cluster real e atualizar a documentação operacional.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | rodar baseline estática e testes do raccoon-cli após as mudanças | `runtime-validator` + `cli-evolution` | completed | `cargo test`, `make verify`, checks do `raccoon-cli` |
| 3.2 | provar cenários de smoke com stack fresca e identidade isolada | `runtime-validator` | completed | `readiness-probe`, `happy-path`, `invalid-payload` verdes |
| 3.3 | coletar evidência e registrar a política final de paralelismo/isolamento | `runtime-validator` + `documentation-writer` | completed | artefato de validação + docs atualizadas |

**Acceptance Criteria**

- cenários passam sem depender de chave compartilhada
- um conflito entre execuções só ocorre se houver bug real no tooling ou no runtime
- `.context` e docs canônicas refletem a regra final de uso

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B3-P1-S1 | mapear todos os identificadores fixos do smoke engine | `cli-evolution` | nenhuma | inventário com arquivo e uso |
| B3-P1-S2 | congelar identidade alvo por execução e fronteiras que continuam canônicas | `code-reviewer` + `cli-evolution` | `B3-P1-S1` | freeze doc |
| B3-P1-S3 | fechar matriz de validação do bloco | `tdd-coordinator` | `B3-P1-S2` | sequência before/after |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B3-P2-S1 | introduzir run identity no `SmokeConfig` | `cli-evolution` | `B3-P1-S2` | API interna de identidade |
| B3-P2-S2 | parametrizar config key, binding e correlation id | `cli-evolution` | `B3-P2-S1` | inject e cenários sem nomes fixos críticos |
| B3-P2-S3 | alinhar consultas de runtime/results com a nova identidade | `cli-evolution` | `B3-P2-S2` | filtros corretos e evidência rastreável |
| B3-P2-S4 | adicionar cobertura de teste do smoke engine | `test-writer` + `cli-evolution` | `B3-P2-S3` | testes de naming, filtros e colisão |
| B3-P2-S5 | revisar risco de acoplamento com contracts reais | `code-reviewer` | `B3-P2-S2`, `B3-P2-S3` | review fechado |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B3-P3-S1 | rodar baseline estatica apos o isolamento | `runtime-validator` | todos os steps da phase 2 | `cargo test`, `make verify`, `doctor`, `topology-doctor`, `contract-audit`, `runtime-bindings` |
| B3-P3-S2 | rodar smoke com stack fresca | `runtime-validator` | `B3-P3-S1` | `readiness-probe`, `happy-path`, `invalid-payload` |
| B3-P3-S3 | validar política final de uso e documentar | `documentation-writer` | `B3-P3-S2` | docs e artefato de validação |

## Validation Ladder

**Before**

- `raccoon-cli doctor`
- `raccoon-cli topology-doctor`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `raccoon-cli recommend tools/raccoon-cli/src/smoke/mod.rs tools/raccoon-cli/src/smoke/stages.rs tools/raccoon-cli/src/smoke/scenarios.rs`

**During**

- `cargo test --manifest-path tools/raccoon-cli/Cargo.toml smoke`
- testes focados para identidade, naming e filtros

**After**

- `make verify`
- `make down`
- `make up-dataplane`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

## Done Definition

Este bloco só termina quando:

- `scenario-smoke` deixa de depender de nomes fixos compartilhados para provar o cluster
- a evidência de cada execução fica rastreável por id próprio
- os cenários passam de forma determinística na ordem recomendada e ficam preparados para isolamento futuro mais forte
- o `raccoon-cli` continua simples de operar como motor de qualidade do repositório

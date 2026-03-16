---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Preservar a topologia e o ownership do cluster enquanto reduz ruido operacional"
  - type: "cli-evolution"
    role: "Manter o raccoon-cli como motor de qualidade para signal, drift e smoke"
  - type: "runtime-validator"
    role: "Provar em runtime que o cluster continua observavel e com bom sinal"
  - type: "tdd-coordinator"
    role: "Escolher a menor escada de validacao entre testes estaticos, smoke e deep gate"
  - type: "code-reviewer"
    role: "Revisar regressao de signal, acoplamento e perda de diagnostico util"
  - type: "documentation-writer"
    role: "Atualizar docs e contexto canonico com o novo padrao operacional"
docs:
  - "project-overview.md"
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "tooling-raccoon-cli.md"
  - "development-workflow.md"
phases:
  - id: "phase-1"
    name: "Signal Baseline & Scope Freeze"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Noise Reduction & Static Guard Rails"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Runtime Proof & Context Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Bloco 2 - Operational Signal Hardening Plan

> Reduzir ruido operacional do cluster, transformar topologia Compose em contrato testavel fora do runtime e manter o raccoon-cli como motor de qualidade para signal, drift e smoke.

## Task Snapshot

- **Primary goal:** sair do bloco com logs de actor mais acionaveis, Compose mais protegido contra drift de topologia e imagem, e guard rails do `raccoon-cli` refletindo melhor a saude operacional do cluster sem depender apenas de runtime ao vivo.
- **Success signal:** o cluster sobe sem enxurrada de warnings nao-acionaveis, existe cobertura estatica para invariantes do Compose, e o fluxo `doctor -> topology-doctor -> runtime-bindings -> scenario-smoke -> quality-gate --profile deep` continua verde.
- **Out of scope:** persistencia nova, horizontalizacao, mudanca de ownership entre `server`, `configctl`, `consumer`, `validator` e `emulator`, redesign de contratos HTTP ou NATS, ou troca de stack de observabilidade.
- **Key references:**
  - [Project Overview](../docs/project-overview.md)
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Bloco 1 - Lifecycle Hardening](./block-1-lifecycle-hardening.md)
  - [Bloco 1 Phase 3 Validation](./block-1-phase-3-validation.md)

## Codebase Context

### What Block 1 Already Proved

- lifecycle canonico entre `server`, `configctl` e `validator` esta coerente e validado por `scenario-smoke` e `quality-gate --profile deep`
- o `raccoon-cli` ja consegue provar `config-lifecycle`, `happy-path`, `invalid-payload` e `readiness-probe`
- o dataplane Compose voltou a ficar executavel com `bitnamilegacy/kafka:3.9.0`
- o `readyz` composto do `server` e o guard do runtime no `validator` expuseram e resolveram uma race real, entao o fluxo atual nao deve ser reaberto neste bloco

### Current Gaps Driving Block 2

- varios actors de `configctl`, `consumer` e `validator` fazem `Warn("unknown message")` para mensagens normais do lifecycle de Hollywood (`actor.Initialized`, `actor.Started`), o que polui logs e reduz signal-to-noise
- a topologia Compose ainda depende demais de prova em runtime; o proprio `raccoon-cli recommend` apontou falta de cobertura estatica para regressao de `deploy/compose/docker-compose.yaml`
- a linha de validacao do cluster esta forte no runtime, mas ainda fraca para detectar drift de imagem, profiles e dependencias antes de subir containers
- o workflow do `ai-context` ficou atras da execucao real, entao este bloco tambem precisa manter contexto e evidencias mais sincronizados

### Preserve These Decisions

- `raccoon-cli` continua sendo o control plane de qualidade
- Compose continua sendo a descricao canonica do cluster local
- warnings operacionais continuam existindo para falhas reais; o alvo aqui e remover apenas ruido previsivel e nao diagnostico util
- as docs canonicas existentes continuam sendo a fonte de verdade, com ajuste pontual em vez de fragmentacao nova

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| Cluster Architect | congelar o que e ruido aceitavel vs sinal real e preservar o desenho do cluster | [cluster-architect](../agents/cluster-architect.md) | topologia, profiles, ownership e impacto transversal |
| CLI Evolution | transformar invariantes do Compose e da saude operacional em checks reutilizaveis | [cli-evolution](../agents/cli-evolution.md) | analyzers, quality-gate e contratos de diagnostico |
| Runtime Validator | provar que as mudancas melhoram logs e nao degradam smoke ou readiness | [runtime-validator](../agents/runtime-validator.md) | smoke, trace-pack, logs e validacao ao vivo |
| TDD Coordinator | escolher a menor combinacao de testes estaticos e live proof | [tdd-coordinator](../agents/tdd-coordinator.md) | matriz de validacao e cobertura por risco |
| Code Reviewer | revisar risco de esconder falhas reais ao reduzir warnings | [code-reviewer](../agents/code-reviewer.md) | regressao comportamental e perda de observabilidade |
| Documentation Writer | atualizar docs e contexto com os novos guard rails e signal rules | [documentation-writer](../agents/documentation-writer.md) | docs canonicas e artefatos do `.context` |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | refletir que o cluster agora tem guard rails estaticos adicionais para Compose e signal |
| Architecture Runtime | [architecture-runtime.md](../docs/architecture-runtime.md) | explicitar expectativa de lifecycle messages dos actors e o que e tratado como ruido normal |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | incluir cobertura estatica de Compose e o papel dela antes de `up-dataplane` |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | documentar os novos checks/analyzers e a ordem de uso |
| Development Workflow | [development-workflow.md](../docs/development-workflow.md) | ajustar a escada de validacao para incluir os novos guard rails estaticos |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| remover warnings e esconder falhas reais de actor wiring | Medium | High | distinguir explicitamente lifecycle messages esperadas de mensagens desconhecidas de verdade; revisar cada `default:` antes de reduzir ruido | `code-reviewer` |
| cobertura estatica de Compose ficar acoplada a detalhes irrelevantes e gerar falso positivo | Medium | Medium | testar apenas invariantes canonicamente importantes: imagens, profiles, depends_on, portas criticas, brokers e servicos obrigatorios | `cli-evolution` |
| checks novos no `raccoon-cli` divergirem do Compose real | Low | High | manter fixtures alinhados ao arquivo real e revalidar com `topology-doctor`, `drift-detect` e `scenario-smoke` | `cli-evolution` + `runtime-validator` |
| escopo crescer para refactor amplo de actor model | Medium | Medium | manter o bloco restrito a signal e guard rails, sem mexer em ownership ou contracts | `cluster-architect` |

### Dependencies

- **Internal:** `deploy/compose/docker-compose.yaml`, `Makefile`, actors em `internal/actors/scopes/*`, analyzers em `tools/raccoon-cli/src/analyzers`
- **External:** disponibilidade das imagens Docker ja referenciadas pelo cluster local
- **Technical:** manter `make verify`, `scenario-smoke` e `quality-gate --profile deep` como prova final

### Assumptions

- mensagens `actor.Initialized` e `actor.Started` sao parte normal do lifecycle do framework para varios actors deste repo
- invariantes de Compose relevantes para qualidade local sao estaveis o bastante para virar teste estatico
- o bloco nao precisa alterar payloads, bindings ou contracts ja estabilizados no Bloco 1

## Working Phases

### Phase 1 - Signal Baseline & Scope Freeze
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar a definicao do que e ruido operacional aceitavel e do que precisa continuar aparecendo como warning ou erro real.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | Inventariar todos os `unknown message` atuais por actor e classificar quais correspondem a mensagens normais do framework | `cluster-architect` + `code-reviewer` | completed | matriz actor -> message type -> tratamento desejado |
| 1.2 | Definir o contrato estatico minimo do Compose a proteger fora do runtime | `cluster-architect` + `cli-evolution` | completed | lista canonica de invariantes de topologia, imagem, profiles e depends_on |
| 1.3 | Fechar a escada de validacao do bloco | `tdd-coordinator` | completed | baseline de testes unitarios/analyzers + runtime proof final |

**Acceptance Criteria**

- matriz de warnings esperados vs anomalias reais fechada
- escopo de checks estaticos de Compose congelado
- nenhum item do bloco depende de redesign de arquitetura

---

### Phase 2 - Noise Reduction & Static Guard Rails
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** reduzir ruido nos actors e criar guard rails estaticos que peguem drift de Compose sem precisar subir o cluster.

**Front 1: Actor Signal Cleanup**

- revisar `default:` dos actors em `configctl`, `consumer` e `validator`
- deixar de logar como `WARN` mensagens normais do lifecycle do framework
- preservar `WARN` e `ERROR` apenas para estados realmente anormais

**Front 2: Compose Contract Tests**

- adicionar testes estaticos para o Compose ou analyzer coverage equivalente
- proteger:
  - servicos obrigatorios
  - profiles `core`, `runtime`, `dataplane`, `all`
  - imagens criticas como Kafka/NATS
  - portas externas e brokers canonicos
  - dependencias essenciais entre `configctl`, `server`, `validator`, `consumer`, `emulator`

**Front 3: Raccoon CLI Integration**

- garantir que `doctor`, `topology-doctor`, `drift-detect` ou analyzer novo reflitam essas invariantes
- manter mensagens do CLI objetivas e acionaveis
- evitar duplicar logica entre testes puros e analyzer do `raccoon-cli`

**Acceptance Criteria**

- logs de bootstrap do cluster ficam mais limpos sem perder sinal real
- regressao de Compose importante falha em validacao estatica antes de `make up-dataplane`
- `raccoon-cli recommend` e `quality-gate` continuam coerentes com a nova camada de guard rail

---

### Phase 3 - Runtime Proof & Context Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar que a reducao de ruido e os novos guard rails melhoram a operacao real sem degradar readiness, smoke ou diagnostico.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | Rodar validacao estatica e arquitetural apos as mudancas | `runtime-validator` + `cli-evolution` | completed | `doctor`, `topology-doctor`, `runtime-bindings`, `drift-detect`, `make verify` |
| 3.2 | Rodar os cenarios de smoke recomendados pelo `raccoon-cli` | `runtime-validator` | completed | `happy-path`, `invalid-payload`, `readiness-probe` e, se necessario, `config-lifecycle` |
| 3.3 | Comparar logs antes/depois e coletar evidencia com `trace-pack` se houver regressao | `runtime-validator` + `code-reviewer` | completed | evidencia de signal melhorado ou troubleshooting objetivo |
| 3.4 | Atualizar docs e `.context` com a nova politica de signal e guard rails | `documentation-writer` | completed | docs canonicas e plano marcados com a verdade final |

**Acceptance Criteria**

- baseline estatico e runtime ficam verdes
- warnings de actor deixam de poluir bootstrap normal
- o cluster continua diagnosticavel por logs e `trace-pack`
- `.context` reflete a nova camada de signal hardening

## Executable Backlog

### Phase 1 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P1-S1 | Inventariar todos os warnings `unknown message` e mapear quais sao lifecycle normal do framework | `cluster-architect` | nenhuma | tabela de actors e message types |
| B2-P1-S2 | Congelar a lista de invariantes do Compose que merecem teste estatico | `cli-evolution` | `B2-P1-S1` | checklist de imagem/profile/depends_on/brokers/ports |
| B2-P1-S3 | Definir a escada de validacao do bloco usando `raccoon-cli` | `tdd-coordinator` | `B2-P1-S2` | sequencia before/after clara |

### Phase 2 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P2-S1 | Reduzir warnings nao-acionaveis nos actors de `configctl` | `feature-developer` | `B2-P1-S1` | bootstrap de `configctl` sem `unknown message` espurio |
| B2-P2-S2 | Reduzir warnings nao-acionaveis nos actors de `consumer` e `validator` | `feature-developer` | `B2-P1-S1` | bootstrap de dataplane com melhor signal |
| B2-P2-S3 | Adicionar testes estaticos ou analyzer coverage para invariantes de Compose | `cli-evolution` + `test-writer` | `B2-P1-S2` | testes/analyzers falham em drift relevante |
| B2-P2-S4 | Integrar os novos guard rails ao fluxo do `raccoon-cli` e do `Makefile` quando fizer sentido | `cli-evolution` | `B2-P2-S3` | recomendacoes e gates atualizados |
| B2-P2-S5 | Revisar risco de mascarar diagnostico real | `code-reviewer` | `B2-P2-S1`, `B2-P2-S2`, `B2-P2-S4` | review de signal e regressao fechada |

### Phase 3 Backlog

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B2-P3-S1 | Rodar baseline estatica completa apos o cleanup | `runtime-validator` | todos os steps da phase 2 | `make verify`, `doctor`, `topology-doctor`, `runtime-bindings`, `drift-detect` |
| B2-P3-S2 | Rodar smoke recomendado para garantir que signal cleanup nao afetou comportamento | `runtime-validator` | `B2-P3-S1` | `happy-path`, `invalid-payload`, `readiness-probe` verdes |
| B2-P3-S3 | Coletar logs e `trace-pack` se a diferenca de signal precisar de evidencia anexada | `runtime-validator` | `B2-P3-S2` | artefatos em `.context/plans/artifacts/` |
| B2-P3-S4 | Atualizar docs e contexto vivo | `documentation-writer` | `B2-P3-S1`, `B2-P3-S2` | docs e plano sincronizados |

## Validation Ladder

**Before**

- `raccoon-cli doctor`
- `raccoon-cli topology-doctor`
- `raccoon-cli runtime-bindings`
- `raccoon-cli drift-detect`

**During**

- testes unitarios focados nos actors alterados
- testes/analyzers novos para Compose

**After**

- `make verify`
- `make up-dataplane`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make scenario-smoke SCENARIO=happy-path`
- `make check-deep`

## Done Definition

Este bloco so termina quando:

- bootstrap logs deixam de ser dominados por `unknown message` previsivel
- regressao relevante de Compose falha sem precisar subir o cluster
- `raccoon-cli` continua sendo a porta principal para descobrir e provar signal, drift e smoke
- docs canonicas e plano refletem o novo estado operacional

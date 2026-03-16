---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cluster-architect"
    role: "Definir ownership, topologia e fronteiras corretas entre server, configctl e validator"
  - type: "contract-guardian"
    role: "Congelar a superficie canonica de contracts HTTP e NATS sem espalhar aliases legados"
  - type: "runtime-validator"
    role: "Definir o caminho de prova operacional local e runtime-sensitive"
  - type: "cli-evolution"
    role: "Alinhar raccoon-cli smoke, trace-pack e quality-gate ao lifecycle real"
  - type: "tdd-coordinator"
    role: "Escolher a escada de validacao certa para cada frente do bloco"
  - type: "code-reviewer"
    role: "Revisar riscos de acoplamento, duplicacao e regressao comportamental"
  - type: "test-writer"
    role: "Cobrir contratos, handlers, smoke e cenarios de restart"
  - type: "documentation-writer"
    role: "Atualizar docs e .http para a superficie canonica escolhida"
docs:
  - "project-overview.md"
  - "architecture-runtime.md"
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Canonicalization & Scope Freeze"
    prevc: "P"
    agent: "cluster-architect"
  - id: "phase-2"
    name: "Lifecycle Hardening"
    prevc: "E"
    agent: "contract-guardian"
  - id: "phase-3"
    name: "Validation & Handoff"
    prevc: "V"
    agent: "runtime-validator"
---

# Bloco 1 - Lifecycle Hardening Plan

> Plano executivo e tecnico para endurecer e limpar o lifecycle atual entre `server`, `configctl` e `validator`, preservando o que ja esta correto e removendo duplicacao, legado e acoplamentos desnecessarios.

## Task Snapshot

- **Primary goal:** sair do Bloco 1 com um lifecycle canonico, observavel e consistente entre contracts, actors, runtime query, read models HTTP, `.http`, Compose, Makefile e smoke local, removendo legado morto, reduzindo ruido arquitetural e deixando uma fundacao limpa para o contract de runtime do Bloco 2.
- **Success signal:** a superficie canonica fica unica e clara, o `server` sinaliza prontidao de forma honesta, o `validator` fica mais robusto a restart e replay sem virar owner de dominio, o `raccoon-cli` passa a provar o lifecycle real em vez de um payload antigo, e o trilho para consolidar runtime/bindings no Bloco 2 fica destravado.
- **Key references:**
  - [Architecture Runtime](../docs/architecture-runtime.md)
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Agent Handbook](../agents/README.md)
  - [Lifecycle Freeze Artifact](./block-1-lifecycle-hardening-freeze.md)
  - [Readiness And Runtime Seam](./block-1-readiness-runtime-seam.md)
  - [Block 2 - Ingestion Runtime Contract](./block-2-ingestion-runtime-contract.md)

## Codebase Context

### What Is Already Solid And Must Be Preserved

- `server` e uma facade HTTP fina sobre gateways de `configctl`, runtime e results, sem dominio embutido em [`cmd/server/run.go`](../cmd/server/run.go).
- `configctl` e o dono do lifecycle e das projecoes de runtime e ingestion, com contracts NATS explicitos em [`internal/adapters/nats/configctl_registry.go`](../internal/adapters/nats/configctl_registry.go).
- `validator` ja esta separado em cache de runtime, consumo dataplane, roteamento e query responders em [`internal/actors/scopes/validator/supervisor.go`](../internal/actors/scopes/validator/supervisor.go).
- os contracts de aplicacao e de transporte estao bem normalizados, com defaults de scope e envelopes compactos.
- os guard rails estaticos do repositorio estao verdes:
  - `make check`
  - `make arch-guard`
  - `make drift-detect`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
  - `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- contract-audit`

### Where The Lifecycle Is Still Weak

- a superficie HTTP de `configctl` ainda carrega aliases legados e redundantes em [`internal/interfaces/http/routes/configctl.go`](../internal/interfaces/http/routes/configctl.go).
- o `readyz` do `server` so prova `configctl` hoje, apesar de o processo tambem servir runtime query e validation results em [`cmd/server/readiness.go`](../cmd/server/readiness.go).
- o smoke do `raccoon-cli` ainda assume shape antigo de resposta para draft e active config em [`tools/raccoon-cli/src/smoke/stages.rs`](../../tools/raccoon-cli/src/smoke/stages.rs) e [`tools/raccoon-cli/src/smoke/scenarios.rs`](../../tools/raccoon-cli/src/smoke/scenarios.rs).
- o runtime do `validator` e reidratado por consumo do evento `config.activated`, mas o comportamento esperado para restart, replay e limpeza ainda nao esta tratado como seam canonico.
- o results store do `validator` continua intencionalmente em memoria, o que e aceitavel para agora, mas exige fronteira melhor definida para os proximos blocos.

## Diagnostic Summary

### Current Strengths

- arquitetura em camadas esta correta e sem vazamento entre `domain`, `application`, `interfaces`, `cmd` e tooling.
- transport contracts e registries estao consistentes.
- Compose e Makefile espelham o cluster real.
- os `.http` e o raccoon mapeiam o lifecycle certo em alto nivel: draft -> validate -> compile -> activate -> runtime -> bindings -> results.

### Current Distortions

- duplicacao de rotas e aliases para a mesma intencao operacional.
- prontidao do `server` abaixo da responsabilidade real do processo.
- tooling local desatualizado em relacao ao shape HTTP.
- dependencias implicitas demais entre ativacao, cache de runtime e observabilidade de results.

### Dead Or Legacy Surfaces To Remove Or Deprecate

- aliases HTTP mantidos apenas por compatibilidade de costume:
  - `/configctl/configs/by-id`
  - `/configctl/active-config`
- expectativas antigas de payload no smoke do `raccoon-cli`:
  - leitura de `id` no topo da resposta de create-draft
  - leitura de `id` no topo da resposta de active config
- testes de rota que existirem apenas para alias legado, sem valor de compatibilidade deliberada

### Decisions To Preserve

- `server` continua facade e nao vira orchestrator de dominio.
- `configctl` continua owner de lifecycle e projections.
- `validator` continua consumindo contracts e respondendo queries separadas de runtime e results, sem virar fonte de verdade do runtime de ingestao.
- `runtime/ingestion/bindings` continua sendo a superficie de bootstrap para `consumer` e `emulator`.
- `raccoon-cli` continua sendo o control plane de qualidade, nao ferramenta opcional.

## Priority Problems

| Priority | Problem | Why It Matters Now | Preserve Or Remove |
| --- | --- | --- | --- |
| P1 | Surface HTTP duplicada no lifecycle | espalha legado em docs, tests, `.http` e smoke | remover aliases e congelar API canonica |
| P1 | `server` pronto demais cedo | readiness nao representa o que o processo realmente serve | preservar facade, endurecer cheque |
| P1 | Smoke do `raccoon-cli` com contract antigo | prova operacional local pode falhar mesmo com runtime correto | preservar smoke, corrigir contract |
| P2 | Runtime cache do `validator` com seam de restart pouco explicita | limita robustez para restart e proximos blocos | preservar actor model, endurecer lifecycle |
| P2 | Results store em memoria sem contrato de evolucao claro | aceitavel agora, fraco para horizontalizacao futura | preservar no bloco 1, explicitar limite |
| P3 | Read models HTTP proximos demais de contracts internos | pode aumentar acoplamento no proximo bloco | endurecer sem reescrever tudo |

## Working Phases

### Phase 1 - Canonicalization & Scope Freeze
> **Primary Agent:** `cluster-architect` - [Playbook](../agents/cluster-architect.md)

**Objective:** congelar a superficie canonica do lifecycle e explicitar o que fica, o que sai e o que vira compatibilidade temporaria.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | Definir os endpoints canonicos de lifecycle e read models | `cluster-architect` + `contract-guardian` | pending | matriz de rotas canonicas vs aliases a remover |
| 1.2 | Mapear aliases legados em handlers, `.http`, smoke e tooling | `contract-guardian` | pending | inventario de legado e plano de deprecacao |
| 1.3 | Definir o contrato de prontidao real do `server` | `cluster-architect` + `runtime-validator` | pending | criterio de `readyz` e dependencias downstream |
| 1.4 | Definir a estrategia de restart e reidratação do runtime do `validator` sem introduzir persistencia nova | `cluster-architect` + `runtime-validator` | pending | seam canonico para replay/bootstrap |

**Acceptance Criteria**

- uma unica superficie HTTP canonica definida para lifecycle e query
- aliases marcados como remover, manter temporariamente, ou documentar como compatibilidade
- criterio de prontidao do `server` aprovado
- comportamento esperado de restart/replay do `validator` definido

---

### Phase 2 - Lifecycle Hardening
> **Primary Agent:** `contract-guardian` - [Playbook](../agents/contract-guardian.md)

**Objective:** aplicar a limpeza e o endurecimento cirurgico sem reescrever a arquitetura correta.

**Front 1: HTTP and Read Models**

- remover ou deprecar aliases em [`internal/interfaces/http/routes/configctl.go`](../internal/interfaces/http/routes/configctl.go)
- manter apenas a superficie canonicamente suportada em handlers, `.http` e docs
- revisar onde read models HTTP precisam ficar mais estaveis e menos acoplados ao contract interno

**Front 2: Server Readiness**

- tornar `readyz` coerente com a responsabilidade real do `server`
- checar `configctl` e disponibilidade das consultas de runtime/results que o proprio `server` promete servir
- evitar transformar readiness em orchestration pesada ou bloqueio artificial de bootstrap

**Front 3: Validator Runtime Lifecycle**

- endurecer o seam entre evento `config.activated`, cache de runtime e query responder
- explicitar comportamento de replay e startup
- preparar o caminho para persistencia ou horizontalizacao futura sem introduzi-las agora

**Front 4: Local Smoke And Tooling**

- alinhar `raccoon-cli` smoke ao shape HTTP real
- alinhar `.http` ao contrato canonico
- manter `trace-pack`, `results-inspect` e `scenario-smoke` como prova operacional principal

**Front 5: Coherence Across Layers**

- revisar coerencia entre application contracts, NATS registries, routes HTTP, `.http`, Make targets e smoke
- garantir que `server`, `configctl` e `validator` contem a mesma historia operacional

**Acceptance Criteria**

- o contract HTTP do lifecycle fica unico e refletido em tests, `.http` e `raccoon-cli`
- `readyz` do `server` representa a disponibilidade real do processo
- lifecycle `activate -> runtime cache -> runtime query -> results query` fica explicito e mais robusto
- nenhuma camada nova ou acoplamento indevido e introduzido

---

### Phase 3 - Validation & Handoff
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** provar que o lifecycle endurecido funciona no repositorio real e deixar evidencias para os proximos blocos.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | Revalidar guard rails estaticos | `runtime-validator` + `cli-evolution` | completed | `make check`, `make verify`, `make check-deep`, `contract-audit`, `drift-detect`, `arch-guard` e `topology-doctor` verdes |
| 3.2 | Executar smoke local do lifecycle canonico | `runtime-validator` | completed | `scenario-smoke config-lifecycle`, `happy-path` e `readiness-probe` passaram |
| 3.3 | Coletar diagnostico e troubleshooting | `runtime-validator` + `documentation-writer` | completed | trace packs de outage e runtime-race coletados em `.context/plans/artifacts/block-1-phase-3/` |
| 3.4 | Atualizar docs e contexto | `documentation-writer` | completed | evidencias e decisoes finais registradas no plano e no tracking do `ai-context` |

**Acceptance Criteria**

- `make verify` passa
- `make check-deep` ou `make scenario-smoke SCENARIO=config-lifecycle` prova o lifecycle
- smoke do `raccoon-cli` representa a API real
- docs e `.context` ficam alinhados ao resultado final do bloco

## Executable Backlog

### Backlog Rules

- nenhum step de codigo comeca antes do freeze da superficie canonica do lifecycle
- cada step deve produzir evidencia objetiva em codigo, `.http`, `raccoon-cli`, docs ou saida de check
- `raccoon-cli` e os `.http` sao parte do produto operacional deste bloco, nao pos-processamento
- quando houver conflito entre runtime real e scaffold do MCP, a fonte de verdade e o repositorio

### Phase 1 Backlog - Canonicalization & Scope Freeze

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B1-P1-S1 | Fechar a matriz de endpoints canonicos de draft, version lookup, validate, compile, activate, active config, runtime query e results query | `cluster-architect` | nenhuma | tabela de rotas canonicas vs aliases a remover ou deprecar |
| B1-P1-S2 | Inventariar aliases, shapes antigos e testes/tooling que ainda dependem deles | `contract-guardian` | `B1-P1-S1` | lista de handlers, `.http`, smoke e docs afetados |
| B1-P1-S3 | Definir o contrato de `readyz` do `server` sem transformar readiness em orchestration pesada | `runtime-validator` | `B1-P1-S1` | regra objetiva de prontidao e dependencias downstream |
| B1-P1-S4 | Definir o seam de startup/replay do `validator` e o source of truth do runtime no restart | `cluster-architect` | `B1-P1-S1` | nota arquitetural de bootstrap/reidratação |

### Phase 2 Backlog - Lifecycle Hardening

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B1-P2-S1 | Remover ou deprecar aliases HTTP e consolidar a superficie canonica em handlers e read models | `contract-guardian` | `B1-P1-S1`, `B1-P1-S2` | diff em routes/handlers e ajuste dos testes de rota |
| B1-P2-S2 | Alinhar `.http` e docs operacionais aos endpoints e payloads congelados | `documentation-writer` | `B1-P2-S1` | `.http` e docs sem referencias legadas |
| B1-P2-S3 | Corrigir smoke e cenarios do `raccoon-cli` para o shape HTTP real | `cli-evolution` | `B1-P2-S1` | smoke local lendo payloads corretos |
| B1-P2-S4 | Endurecer `readyz` do `server` para refletir `configctl` e superficies de query realmente servidas | `runtime-validator` | `B1-P1-S3` | implementacao e teste de prontidao coerente |
| B1-P2-S5 | Endurecer startup/replay do `validator` sem adicionar persistencia nova | `cluster-architect` | `B1-P1-S4` | seam de runtime claro e testavel |
| B1-P2-S6 | Revisar coerencia entre contracts, registries, actors, read models e tooling | `code-reviewer` | `B1-P2-S1`, `B1-P2-S3`, `B1-P2-S4`, `B1-P2-S5` | checklist de coerencia fechado |

### Phase 3 Backlog - Validation & Handoff

| Step ID | Work Item | Primary Owner | Dependencies | Evidence |
| --- | --- | --- | --- | --- |
| B1-P3-S1 | Rodar guard rails estaticos e arquiteturais apos o cleanup | `runtime-validator` | todos os steps da phase 2 | `make check`, `arch-guard`, `drift-detect`, `contract-audit`, `topology-doctor` |
| B1-P3-S2 | Rodar `scenario-smoke` do lifecycle canonico e revisar `.http` correspondente | `runtime-validator` | `B1-P3-S1` | evidencia de smoke local coerente |
| B1-P3-S3 | Coletar diagnostico com `trace-pack` e `results-inspect` se houver falha ou regressao | `runtime-validator` | `B1-P3-S2` | pacote de troubleshooting anexado ao bloco |
| B1-P3-S4 | Atualizar `.context/docs`, `.context/agents` e `.context/skills` com a verdade final do bloco | `documentation-writer` | `B1-P3-S1`, `B1-P3-S2` | contexto vivo alinhado ao resultado final |

### Tracking Notes

- o MCP do `ai-context` esta rastreando este plano em nivel de fase
- os step IDs acima sao a referencia operacional canonica enquanto o parser do MCP nao materializa steps deste template
- qualquer handoff entre agents deve citar os `Step ID` afetados para manter rastreabilidade

## Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| Remocao prematura de alias quebrar smoke e `.http` ao mesmo tempo | Medium | High | congelar a API canonica antes de remover qualquer alias; migrar tooling primeiro | `contract-guardian` |
| `readyz` endurecido demais bloquear bootstrap local | Medium | Medium | definir cheque composto, mas barato; validar com compose e smoke de readiness | `runtime-validator` |
| Reidratação do runtime do `validator` introduzir replay duplicado ou runtime stale | Medium | High | tratar replay como seam explicito, com criterio de source of truth antes de mudar codigo | `cluster-architect` |
| Escopo expandir para persistencia real de runtime/results | Medium | High | manter o bloco focado em endurecimento e limpeza, nao em trocar o modelo operacional | `cluster-architect` |
| Ferramenta e runtime voltarem a divergir | Medium | High | obrigar alinhamento entre routes, `.http`, smoke e docs no mesmo bloco | `cli-evolution` |

## Dependencies

- **Internal**
  - contracts de `configctl`, runtime e results
  - registries NATS e actors de `configctl` e `validator`
  - handlers e routes HTTP
  - smoke e diagnostics em `tools/raccoon-cli`
- **Technical**
  - Compose profiles `core`, `runtime` e `dataplane`
  - NATS request/reply e JetStream durable consumer
  - Make targets `check`, `verify`, `check-deep`, `scenario-smoke`, `trace-pack`, `results-inspect`
- **External**
  - nenhuma dependencia externa nova deve ser introduzida neste bloco

## Assumptions

- o actor model atual deve ser preservado; o bloco nao vai substituir a topologia por outro modelo
- o results store em memoria pode permanecer neste bloco, desde que seus limites e seams fiquem explicitos
- a superficie `runtime/ingestion/bindings` continua sendo o bootstrap canonico de dataplane
- a fonte de verdade do lifecycle continua sendo `configctl`, nao o `server`

## Order Of Execution

1. congelar endpoints e payloads canonicos do lifecycle
2. alinhar `raccoon-cli` smoke, `.http` e docs a essa superficie
3. endurecer `readyz` do `server`
4. endurecer startup/replay do runtime do `validator`
5. revisar coerencia final entre contracts, read models, compose, Make e smoke

## Expected Gains

- menos superficie legada e menos ambiguidade para clientes internos
- smoke local e tooling realmente confiaveis para o lifecycle atual
- readiness mais honesta e mais util para compose e troubleshooting
- validator preparado para os proximos blocos sem aumento de acoplamento nem expansao de ownership
- base mais limpa para evoluir escalabilidade e persistencia depois

## Done Definition

O Bloco 1 so termina quando:

- a API canonica do lifecycle estiver definida e aplicada
- aliases legados estiverem removidos ou claramente deprecados
- `server`, `configctl` e `validator` tiverem coerencia operacional entre code, contracts, `.http`, smoke e docs
- os checks relevantes do `raccoon-cli` continuarem verdes
- a prova local do lifecycle estiver alinhada ao shape HTTP real

## Evidence To Collect

- saida de `make check`
- saida de `make verify`
- saida de `make arch-guard`
- saida de `make drift-detect`
- saida de `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- contract-audit`
- saida de `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- topology-doctor`
- evidencia de `make scenario-smoke SCENARIO=config-lifecycle`
- evidencias de `make trace-pack` e `make results-inspect` quando houver falha ou ajuste de runtime

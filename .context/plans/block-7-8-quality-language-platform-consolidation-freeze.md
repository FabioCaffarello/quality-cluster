---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-7-8-quality-language-platform-consolidation"
phase: "phase-1"
---

# Blocks 7-8 Quality Language And Platform Consolidation Freeze

## Objective

Congelar a proxima etapa do motor de qualidade antes da implementacao: a linguagem pode crescer, mas so dentro de um envelope que o runtime compila, executa, prova e diagnostica; a plataforma pode endurecer, mas sem virar arquitetura paralela.

## Stable Truth Before Execution

- `configctl` continua sendo owner exclusivo da autoria, validacao, compilacao e ativacao da config.
- `validator` continua executando apenas runtime compilado e materializando resultados e incidentes operacionais pequenos.
- `consumer` e `emulator` continuam clientes do bootstrap/projection; nao passam a interpretar config bruta.
- `server` continua facade HTTP fina sobre contracts NATS/query.
- `raccoon-cli` continua control plane de qualidade externo ao runtime, nunca source of truth paralelo.

## Canonical Gap To Close

### Block 7 Gap

- a DSL atual em [`document.go`](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/domain/configctl/document.go) ainda e pequena demais para a proxima camada de linguagem;
- `configctl` ja carrega `schema_version`, `runtime_loader` e `compiler_version`, mas o repositorio ainda nao usa esse seam como envelope explicito de capacidade da linguagem;
- projections, results e incidentes ainda precisam amadurecer de forma additiva e explicavel para suportar regras mais ricas sem redistribuir ownership.

### Block 8 Gap

- a plataforma ja tem analyzers, smoke, trace e inspect, mas ainda precisa fechar o ciclo entre linguagem, explainability, drift prevention e auditabilidade;
- o `raccoon-cli` precisa ficar mais forte exatamente onde a linguagem crescer, sem inventar governanca fora do runtime;
- baselines, snapshots, docs e evidence packs ainda nao estao fechados como ritual canonico para essa nova etapa de expansao.

## Freeze Decisions

### Decision 1

- o bloco nao libera semantica stateful, correlacao multi-mensagem, agregacao temporal, lookups externos nem linguagem geral de expressoes.
- tudo que entrar precisa ser deterministico por payload e compilavel em projection explicita.

### Decision 2

- `configctl` continua sendo o unico lugar onde a linguagem vira artifact e runtime executavel.
- `validator` recebe apenas runtime/projection compilada e deve falhar explicitamente se a capacidade ainda nao for suportada.

### Decision 3

- toda ampliacao de linguagem precisa amadurecer junto com:
  - contract de projection
  - explainability de resultado/incidente
  - smoke ou prova runtime equivalente
  - analyzer ou drift guard relevante no `raccoon-cli`

### Decision 4

- consolidacao de plataforma nao significa mais camadas; significa:
  - menos drift
  - feedback loop mais curto
  - melhor diagnostico
  - baseline e evidencia mais repetiveis

## Scope In

- envelope versionado da DSL e de capacidades do runtime
- projections mais maduras para explainability e auditoria
- evolucao additiva de result/incident/query contracts
- guard rails, snapshots, baselines, smoke e traceability alinhados a nova linguagem
- work packages que reduzam drift arquitetural e custo de operacao

## Scope Out

- runtime stateful ou correlacionado
- policy engine, alerting ou workflow humano de incidente
- autoria de config fora de `configctl`
- interpretacao de DSL dentro de `consumer`, `emulator` ou `server`
- analyzer generico desconectado da verdade do repositorio

## Immediate Execution Order

1. congelar ownership, envelope da DSL e contracts minimos
2. evoluir a linguagem em `configctl` com versionamento e projection explainable
3. endurecer `validator` para executar e explicar o runtime compilado
4. atualizar `raccoon-cli`, smoke e diagnosticos para refletir a nova capacidade
5. provar tudo no workflow canonico com baseline, verify, scenarios, inspect e trace-pack

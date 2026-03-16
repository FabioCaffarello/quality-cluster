---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-3-4-real-dataplane-runtime-hardening"
phase: "phase-1"
---

# Block 3-4 Real Dataplane And Runtime Hardening Freeze

## Objective

Congelar a sequencia correta dos blocos 3 e 4: primeiro entregar o primeiro data plane real e observavel do motor de qualidade; depois endurecer ownership, supervisao e registry do runtime sem reabrir `configctl`.

## Stable Truth Before Execution

- `configctl` continua sendo a fonte de verdade do lifecycle e do runtime ativo.
- `/runtime/ingestion/bindings` continua sendo o seam canonico de bootstrap do dataplane.
- `server` continua sendo facade HTTP fina para leitura operacional; nao vira owner de runtime, results ou bindings.
- `consumer` continua sendo a ponte Kafka -> JetStream.
- `validator` continua sendo owner de runtime loaded-state, avaliacao minima e query de resultados.
- `raccoon-cli` continua sendo o motor de prova via `make check`, `make verify`, `make check-deep`, `scenario-smoke`, `results-inspect` e `trace-pack`.

## Canonical Gap To Close

### Block 3 Gap

- o repositorio precisa do primeiro data plane real ponta a ponta:
  - `emulator` publica no Kafka
  - `consumer` consome Kafka e publica contrato canonical no JetStream
  - `validator` consome JetStream, aplica regra simples e produz `ValidationResult`
- esse fluxo precisa ser observavel sem inflar DSL, `validator` ou dominio do `configctl`
- contracts do dataplane, bootstrap por bindings ativos e leitura operacional de resultados precisam ficar pequenos e claros

### Block 4 Gap

- `consumer` e `validator` ainda precisam de ownership mais explicito no runtime
- `run.go` nao deve concentrar wiring procedural crescente
- topics, subjects, streams e durables precisam de registry mais forte
- a topologia de actors precisa refletir claramente `consume`, `route`, `work`, `store` e `query`

## Freeze Decision

Bloco 3 e Bloco 4 serao executados na seguinte ordem operacional:

1. fechar o contrato minimo do dataplane e o caminho e2e real
2. provar esse caminho com smoke, results e trace
3. so entao endurecer `consumer` e `validator` com decomposicao por actors, registry e startup supervisionado

Regra do plano:

- nao usar refatoracao arquitetural do Bloco 4 para esconder lacunas funcionais do Bloco 3
- nao usar inflacao de contracts ou dominio para compensar wiring ruim
- toda mudanca de runtime precisa continuar provavel pelo workflow canônico do repositório

## Scope In

- contrato minimo do dataplane entre Kafka, JetStream e `ValidationResult`
- bootstrap por bindings ativos
- geracao sintetica controlada
- validacao minima e query operacional de resultados
- topologia de actors e registry de `consumer` e `validator`
- refino de `run.go`, startup e supervisao

## Scope Out

- reabrir lifecycle do `configctl`
- expandir DSL ou regras complexas cedo demais
- criar endpoint novo no `server` so para tapar ownership difuso
- introduzir persistencia nova fora do caminho ja existente
- paralelismo irrestrito de smoke

## Immediate Execution Order

1. congelar payloads e invariantes do dataplane
2. fechar boundary Kafka adapter -> contrato interno -> JetStream
3. entregar `ValidationResult` minimo e leitura operacional pelo `server`
4. provar e2e local com smoke e inspect
5. refatorar `consumer` e `validator` para ownership claro por actors
6. consolidar registry e emagrecer `run.go`
7. repetir prova integrada com `check-deep`, `results-inspect` e `trace-pack`

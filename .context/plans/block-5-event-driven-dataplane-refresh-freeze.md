---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-5-event-driven-dataplane-refresh"
phase: "phase-1"
---

# Block 5 Event Driven Dataplane Refresh Freeze

## Objective

Congelar a próxima evolução do dataplane depois do Bloco 4: substituir o polling como gatilho primário de refresh por um sinal de runtime-change já publicado pelo `configctl`.

## Stable Truth From Block 4

- `runtimebootstrap` agregado já é o seam correto para carregar o estado efetivo do dataplane.
- `consumer` e `emulator` já convergem para o binding set ativo.
- o problema residual não é mais bootstrap inicial; é o mecanismo de refresh contínuo, que ainda depende de polling.

## Candidate Refresh Signals

### `config.activated`

- já existe e já alimenta o `validator`
- problema: é mais genérico que o necessário para o dataplane e força semântica indireta

### `config.deactivated`

- já existe e já é usado pelo `validator`
- problema: sozinho não modela o conjunto de bindings de ingestão; continua exigindo composição com outros sinais

### `config.ingestion_runtime_changed`

- já existe no domínio, publisher e registry
- carrega exatamente a mudança operacional de interesse do dataplane
- já distingue `activated` e `cleared`
- é o melhor candidato para virar gatilho canônico de refresh

## Freeze Decision

Bloco 5 vai tratar `config.ingestion_runtime_changed` como sinal primário do refresh do dataplane.

Regra do bloco:

- evento dispara reload
- `runtimebootstrap` agregado continua sendo a fonte de verdade do estado final
- assinatura do binding set continua protegendo contra reload desnecessário
- polling deixa de ser o mecanismo primário; se continuar existindo temporariamente, precisa virar fallback explícito e documentado

## Scope In

- wiring NATS do evento para dataplane
- refresh dirigido a evento em `consumer`
- refresh dirigido a evento em `emulator`
- testes e smoke que provem a troca do mecanismo

## Scope Out

- mudança de API HTTP
- redesenho do bootstrap agregado
- remover o seam por scope de troubleshooting
- paralelismo irrestrito de smoke

## Immediate Execution Order

1. adicionar consumer spec e handler para `config.ingestion_runtime_changed`
2. trocar o gatilho primário do `consumer`
3. alinhar o `emulator`
4. provar com `contract-audit`, `runtime-bindings`, `verify`, smoke e `check-deep`

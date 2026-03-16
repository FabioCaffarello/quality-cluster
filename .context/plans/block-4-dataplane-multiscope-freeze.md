---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-4-dataplane-multiscope-hardening"
phase: "phase-1"
---

# Block 4 Dataplane Multiscope Freeze

## Objective

Congelar o gap real entre o multiscope já suportado por `configctl` e `validator` e o dataplane ainda single-scope em `consumer` e `emulator`, para que o próximo bloco endureça o runtime correto em vez de mexer de novo só no tooling.

## What The Repository Already Gets Right

- `configctl` já expõe bindings ativos e runtime projections por scope e em forma agregada.
- `validator` já usa bootstrap híbrido e cache multiscope; o desenho de runtime query/results não precisa ser reinventado.
- `raccoon-cli runtime-bindings` já protege o encadeamento config -> kafka -> jetstream -> validator e deve continuar sendo o guard rail estático principal.
- o baseline local `global/default` continua sendo uma decisão operacional válida para uso diário e troubleshooting simples.

## Real Single-Scope Points Found

### Consumer Bootstrap

- `internal/application/runtimebootstrap/configured.go`
  - resolve bootstrap a partir de `config.Bootstrap.ScopeKind` e `config.Bootstrap.ScopeKey`
- `internal/actors/scopes/consumer/supervisor.go`
  - sobe uma única geração de runtime a partir de um único `ActiveIngestionBootstrap`
- `internal/actors/scopes/consumer/runtime.go`
  - constrói topologia e consumers Kafka sobre um único snapshot de bindings

### Emulator Bootstrap

- `cmd/emulator/run.go`
  - faz um único bootstrap via `WaitForConfiguredActiveIngestionBootstrap`
  - mantém `bootstrapState.Index` fixo durante todo o loop de publicação

### Operator Configs

- `deploy/configs/consumer.jsonc`
  - fixa `bootstrap.scope_kind = global`
  - fixa `bootstrap.scope_key = default`
- `deploy/configs/emulator.jsonc`
  - repete o mesmo congelamento operacional

## Freeze Decision

Block 4 vai endurecer o dataplane, não reabrir os blocs 1-3.

Regra do bloco:

- preservar `configctl` como fonte de verdade
- preservar `validator` como referência de runtime multiscope já correta
- ampliar o seam de bootstrap do dataplane para mais de um scope ativo
- manter `global/default` como default de operador, mas não como única forma suportada de funcionamento
- usar o `raccoon-cli` para provar o comportamento final

## Scope In

- bootstrap multiscope para `consumer`
- paridade de bootstrap/refresh para `emulator`
- revisão mínima de configs e wiring para suportar o novo desenho
- testes e smoke que provem o comportamento do dataplane

## Scope Out

- persistência nova
- endpoint novo só para o bloco
- paralelismo irrestrito de smoke
- redesenho de `configctl`, `server` ou `validator`

## Immediate Execution Order

1. definir o seam canônico de bootstrap multiscope
2. refatorar `consumer` para o conjunto ativo de bindings
3. alinhar `emulator` ao mesmo modelo
4. ajustar configs e checks do `raccoon-cli`
5. provar tudo com `runtime-bindings`, `verify`, `scenario-smoke` e `check-deep`

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-6-event-refresh-diagnostics-hardening"
phase: "phase-3"
---

# Block 6 Phase 3 Validation

## Validation Goal

Provar que o `raccoon-cli` deixou de ficar cego ao refresh orientado a `config.ingestion_runtime_changed` e agora produz evidência mais útil tanto em ambiente saudável quanto em ambiente ausente.

## Implemented Outcome

- `trace-pack` passou a coletar monitoramento de NATS/JetStream por `nats/healthz.json` e `nats/jsz.json`.
- `scenario-smoke` e o stage `consume` agora devolvem pistas mais úteis quando o pipeline não converge, incluindo inspeção de bindings ativos e runtime do validator.
- o caminho de troubleshooting do CLI agora separa melhor falha de runtime real de ausência total de ambiente.

## Evidence

### Rust Validation

- `make raccoon-test`

### Runtime-Facing CLI Checks

- `raccoon-cli trace-pack --output-dir /tmp/quality-service-traces`
- `raccoon-cli scenario-smoke readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make check-deep`

## Current Result

- `make raccoon-test` passou com `926` testes unitários e `177` testes de integração/matriz (`0 failed`).
- `scenario-smoke readiness-probe` falhou rápido e corretamente com bootstrap explícito:
  - `missing services: nats, kafka, configctl, server, consumer, validator, emulator`
- `trace-pack` gerou evidência em [`/tmp/quality-service-traces/trace-pack-20260316-153812`](/tmp/quality-service-traces/trace-pack-20260316-153812) e agora inclui tentativas explícitas de coleta de:
  - `nats/healthz.json`
  - `nats/jsz.json`
  - `validator-runtime.json`
- depois de subir o cluster com `make up-dataplane`, a prova profunda passou:
  - `make scenario-smoke SCENARIO=happy-path`
  - `make check-deep`
- o `quality-gate --profile deep` fechou verde com:
  - `doctor`
  - `topology-doctor`
  - `contract-audit`
  - `runtime-bindings`
  - `arch-guard`
  - `drift-detect`
  - `runtime-smoke`

## Final Status

Bloco 6 validado em duas condições operacionais:

- ambiente ausente: falha rápida com evidência explícita de ausência de serviços e monitoramento NATS/JetStream
- ambiente saudável: `happy-path` e `check-deep` passam usando o `raccoon-cli` como motor canônico de prova

## Operational Value Already Delivered

O ganho do bloco ficou material nas duas bordas do workflow:

- ausência de NATS monitor e JetStream agora aparece como evidência explícita, não como silêncio do tooling
- falhas de consumo ficam mais próximas de bindings ativos e runtime do validator
- a prova profunda do refresh orientado a evento ficou fechada no mesmo fluxo que o repositório já usa para guard rails e smoke

---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-5-event-driven-dataplane-refresh"
phase: "phase-3"
---

# Block 5 Phase 3 Validation

## Validation Goal

Provar que o dataplane deixou de usar polling como mecanismo primário de refresh e passou a convergir por `config.ingestion_runtime_changed`, preservando o bootstrap agregado como fonte de verdade e o `raccoon-cli` como motor de prova.

## Implemented Outcome

- `configctl` registry passou a expor consumers dedicados de runtime-change para `consumer` e `emulator`, com durables distintos sobre o mesmo subject canônico.
- `consumer bootstrap_actor` trocou o loop periódico por refresh sob demanda via JetStream, mantendo a assinatura do binding set como guard rail contra reload desnecessário.
- `emulator` passou a usar o mesmo gatilho de refresh e agora depende explicitamente de NATS no config e no Compose.
- o seam do bloco 4 foi preservado: evento só dispara reload; o estado efetivo continua vindo do bootstrap agregado de `/runtime/ingestion/bindings`.

## Evidence

### Focused Tests

- `go test ./internal/adapters/nats ./internal/actors/scopes/consumer ./cmd/emulator`

### Static And Fast Guard Rails

- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- contract-audit`
- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- runtime-bindings`
- `make verify`

### Deep Runtime Proof

- `make check-deep`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`

## Result

Todos os checks acima passaram.

Isso fecha a evidência mínima do bloco:

- refresh do dataplane disparado por `config.ingestion_runtime_changed`
- bootstrap agregado preservado como fonte de verdade
- `consumer` e `emulator` alinhados no mesmo gatilho de reconciliação
- guard rails, smoke e deep gate ainda verdes no cluster real

## Residual Risk

- o bloco remove o polling como mecanismo primário, mas não elimina a dependência de JetStream saudável para convergência rápida do dataplane.
- a prova continua sendo sequencial; smoke paralela segue fora de escopo enquanto o runtime inteiro não for endurecido para isso.

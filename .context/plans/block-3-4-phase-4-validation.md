---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-3-4-real-dataplane-runtime-hardening"
phase: "phase-4"
---

# Block 3-4 Phase 4 Validation

## Validation Goal

Provar que os blocos 3 e 4, em conjunto, entregaram um data plane real ponta a ponta e um runtime mais claro em ownership e supervisao, sem regredir os contracts, o bootstrap canonico ou o workflow do `raccoon-cli`.

## Expected Implemented Outcome

Ao final da implementacao, a validacao precisa conseguir afirmar com evidencia que:

- `emulator` publica payloads sinteticos controlados no Kafka por binding ativo
- `consumer` consome Kafka e publica mensagens canonical no JetStream
- `validator` consome JetStream, resolve runtime ativo, aplica regra simples e grava `ValidationResult`
- `server` permite leitura operacional de bindings, runtime loaded-state e resultados sem virar owner do estado
- `consumer` e `validator` possuem ownership mais claro por actors, com menos wiring procedural em `run.go`
- registry concentra topics, subjects, streams e durables operacionais relevantes

## Required Evidence

### Focused Tests

- `go test ./internal/application/dataplane ./internal/application/validatorresults`
- `go test ./internal/actors/scopes/consumer ./internal/actors/scopes/validator`
- `go test ./cmd/consumer ./cmd/validator ./cmd/emulator`

### Static And Fast Guard Rails

- `make check`
- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `make verify`
- `make arch-guard`

### Deep Runtime Proof

- `make up-dataplane`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

### Operational Evidence

- `make results-inspect`
- `make trace-pack`
- leitura dos endpoints de runtime e results expostos via `server`

## Minimum Acceptance Statement

Esta fase so pode ser considerada validada quando a evidencia acima sustentar simultaneamente:

- existe fluxo e2e real de ingestao ate resultado no cluster local
- os resultados sao pequenos, legiveis e suficientes para diagnostico operacional
- bootstrap, routing e contracts continuam coerentes entre `configctl`, `consumer`, `validator`, `server` e `raccoon-cli`
- `consumer` e `validator` ficaram mais claros em ownership e menos dependentes de wiring procedural disperso

## Failure Shapes To Watch

- smoke verde com `results-inspect` pobre ou ambíguo
- `consumer` ainda misturando parsing Kafka e contrato canonical sem boundary testavel
- `validator` ainda concentrando consume, evaluate, store e query em loop procedural unico
- `run.go` menor em linhas mas ainda dono do wiring real
- `contract-audit` ou `runtime-bindings` verdes enquanto a prova runtime mostra drift entre bootstrap e resultado

## Residual Risk To Reassess After Execution

- quanto do futuro reload realmente ficou preparado por ownership e supervisao, e quanto ainda depende de wiring residual
- se o registry cobre de forma suficiente os pontos de runtime ou ainda restam literais operacionais dispersos
- se a observabilidade local atual ja basta para incidentes pequenos ou ainda exige novo endurecimento de tooling

## Fill-In Rule

Quando a implementacao existir, este arquivo deve ser atualizado com:

- outcome implementado de fato
- lista exata de comandos executados
- resultado final de cada etapa
- riscos residuais confirmados

# Block 4 Work Packages

## Objective

Quebrar o Bloco 4 em pacotes de hardening de runtime com ganho estrutural claro, sem diluir o foco em ownership, registry e supervisão.

## Execution Order

### WP-B4-1 Consumer Ownership Tightening

**Goal**

Deixar mais explícita a fronteira entre bootstrap, geração de runtime, consumo por topic, roteamento e publicação.

**Primary files**

- [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/supervisor.go)
- [bootstrap_actor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/bootstrap_actor.go)
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)
- [topic_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_consumer.go)
- [topic_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_router.go)
- [publisher_actor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/publisher_actor.go)

**Expected outcome**

- supervisor só coordena geração e estado
- runtime actor só monta a geração atual
- consumers/routers/publisher ficam com ownership inequívoco

**Validation**

- `go test ./internal/actors/scopes/consumer`
- `make arch-guard`

### WP-B4-2 Validator Ownership Tightening

**Goal**

Separar com mais nitidez runtime cache, consume, route, work, store e query.

**Primary files**

- [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/supervisor.go)
- [runtime_cache.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_cache.go)
- [runtime_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_consumer.go)
- [dataplane_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/dataplane_consumer.go)
- [validation_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_router.go)
- [validation_worker.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_worker.go)
- [results_store.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/results_store.go)
- [runtime_query_responder.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_query_responder.go)
- [results_query_responder.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/results_query_responder.go)

**Expected outcome**

- supervisor perde wiring incidental e bootstrap excessivo
- actors deixam de acumular responsabilidades laterais
- query e store permanecem separados da trilha de ingestão

**Validation**

- `go test ./internal/actors/scopes/validator ./internal/application/validatorresults`
- `make arch-guard`

### WP-B4-3 Registry Consolidation

**Goal**

Puxar literals operacionais e wiring espalhado para registries e seams explícitos.

**Primary files**

- [registry.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/registry.go)
- registries em `internal/adapters/nats/*registry*.go`
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)
- [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/supervisor.go)

**Expected outcome**

- topics, subjects, streams e durables relevantes ficam centralizados
- menos `Default...Registry()` implícito em pontos profundos do runtime

**Validation**

- `raccoon-cli contract-audit`
- `raccoon-cli runtime-bindings`
- `make verify`

### WP-B4-4 Startup And Entrypoint Hygiene

**Goal**

Garantir que `run.go` continue fino e que o startup esteja claramente entregue a supervisors e seams de runtime.

**Primary files**

- [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/consumer/run.go)
- [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/validator/run.go)
- [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/emulator/run.go)

**Expected outcome**

- validação de config, logger e handoff continuam no entrypoint
- wiring de runtime não volta para o `cmd/*`

**Validation**

- `go test ./cmd/consumer ./cmd/validator ./cmd/emulator`
- `make verify`

### WP-B4-5 Runtime Proof And Cleanup

**Goal**

Fechar código morto, redundâncias e provar que o runtime endurecido continua legível operacionalmente.

**Primary files**

- superfícies tocadas pelos WPs anteriores
- [block-3-4-phase-4-validation.md](/Volumes/OWC%20Express%201M2/Develop/quality-service/.context/plans/block-3-4-phase-4-validation.md)

**Expected outcome**

- menos wiring residual
- startup, ready state e failure handling mais previsíveis
- residual risks documentados

**Validation**

- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`
- `make results-inspect`
- `make trace-pack`

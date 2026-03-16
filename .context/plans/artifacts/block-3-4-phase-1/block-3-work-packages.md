# Block 3 Work Packages

## Objective

Quebrar o Bloco 3 em pacotes pequenos, implementáveis e validáveis, respeitando a ordem funcional do primeiro data plane real.

## Execution Order

### WP-B3-1 Dataplane Contract Boundary

**Goal**

Congelar e testar a fronteira entre input Kafka e contrato canonical do dataplane.

**Primary files**

- [contracts.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/contracts.go)
- [kafka_mapping.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/kafka_mapping.go)
- [registry.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/registry.go)
- testes em `internal/application/dataplane/*_test.go`

**Expected outcome**

- `dataplane.Message` fica suficiente e pequeno
- mapping Kafka -> `dataplane.Message` fica testável sem runtime completo
- route/subject continuam derivando do registry

**Validation**

- `go test ./internal/application/dataplane`
- `raccoon-cli contract-audit`

### WP-B3-2 Emulator Synthetic Input

**Goal**

Garantir geração sintética controlada, por binding ativo, com pelo menos um caso válido e um inválido.

**Primary files**

- [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/emulator/run.go)
- [emulation.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/emulation.go)
- [client.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/runtimebootstrap/client.go)

**Expected outcome**

- `emulator` usa bootstrap ativo real
- publicações Kafka são determinísticas o bastante para smoke
- refresh de bootstrap não quebra publicações

**Validation**

- `go test ./cmd/emulator ./internal/application/runtimebootstrap ./internal/application/dataplane`
- `make verify`

### WP-B3-3 Consumer Kafka To JetStream Bridge

**Goal**

Fechar a ponte funcional Kafka -> canonical message -> JetStream.

**Primary files**

- [topic_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_consumer.go)
- [topic_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_router.go)
- [publisher_actor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/publisher_actor.go)
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)

**Expected outcome**

- topic consumer só consome e encaminha
- topic router só resolve e roteia
- publisher só publica no JetStream
- falhas do bridge são visíveis no supervisor

**Validation**

- `go test ./internal/actors/scopes/consumer ./internal/application/dataplane`
- `make verify`

### WP-B3-4 Validator Minimal Evaluation

**Goal**

Entregar avaliação mínima consistente e gravação de `ValidationResult`.

**Primary files**

- [dataplane_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/dataplane_consumer.go)
- [validation_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_router.go)
- [validation_worker.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_worker.go)
- [results_store.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/results_store.go)
- [evaluate.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/validatorresults/evaluate.go)

**Expected outcome**

- dataplane consumer só encaminha
- worker executa `Evaluate`
- store grava e query responde
- `passed` e `failed` seguem contrato estrito

**Validation**

- `go test ./internal/application/validatorresults ./internal/actors/scopes/validator`
- `make verify`

### WP-B3-5 Operational Read Surface

**Goal**

Fechar a leitura operacional mínima via `server` e tooling.

**Primary files**

- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/interfaces/http/handlers/runtime.go)
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/interfaces/http/routes/runtime.go)
- [gateway.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/gateway.go)
- [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/run.go)
- [validator_results_gateway.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/adapters/nats/validator_results_gateway.go)

**Expected outcome**

- results e runtime ficam legíveis sem lógica de domínio no HTTP
- `results-inspect` e endpoints contam a mesma história

**Validation**

- `go test ./internal/interfaces/http/... ./internal/adapters/nats/...`
- `make results-inspect`
- `make scenario-smoke SCENARIO=happy-path`

## Block Exit Gate

Bloco 3 não avança para o Bloco 4 até todos os pacotes acima sustentarem:

- `make verify`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make check-deep`

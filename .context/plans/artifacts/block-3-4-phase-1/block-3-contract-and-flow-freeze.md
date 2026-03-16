# Block 3 Contract And Flow Freeze

## Objective

Congelar o fluxo funcional do Bloco 3 no nível de componente e contrato, para que a implementação entregue um data plane real antes de qualquer refino arquitetural mais profundo.

## Canonical E2E Flow

1. `runtimebootstrap` lê `/runtime/ingestion/bindings` como fonte de bootstrap ativo.
2. `emulator` usa esse bootstrap para descobrir bindings e topics ativos.
3. `emulator` publica carga sintética controlada no Kafka.
4. `consumer` consome Kafka por topic, normaliza para o contrato interno do dataplane e publica no JetStream.
5. `validator` consome JetStream, resolve o runtime ativo em cache, aplica regras simples e grava `ValidationResult`.
6. `server` expõe leitura operacional do estado carregado e dos resultados.

## Source Files Anchoring The Flow

- bootstrap:
  - [client.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/runtimebootstrap/client.go)
- emulator:
  - [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/emulator/run.go)
- consumer:
  - [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/supervisor.go)
  - [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)
- dataplane contract:
  - [contracts.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/contracts.go)
  - [registry.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/registry.go)
  - [kafka_mapping.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/dataplane/kafka_mapping.go)
- validator:
  - [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/supervisor.go)
  - [evaluate.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/validatorresults/evaluate.go)
- operational read path:
  - [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/server/run.go)

## Frozen Contract Boundaries

### Kafka Input

- Kafka continua sendo input bruto de ingestão do dataplane.
- O adapter Kafka não carrega regra de validação.
- A borda Kafka precisa só produzir dados suficientes para:
  - identificar topic, key, published timestamp e headers
  - derivar `message_id` determinístico
  - carregar o payload JSON bruto

### Canonical Dataplane Message

O contrato interno congelado para o Bloco 3 é o `dataplane.Message` atual:

- `binding`
  - nome do binding
  - topic
  - scope
  - config/version metadata
- `origin`
  - source
  - topic
  - key
  - published_at
- `metadata`
  - `message_id`
  - `correlation_id`
  - `ingested_at`
  - `content_type`
- `payload`
  - JSON bruto válido

Regra do bloco:

- qualquer parsing ou enriquecimento além disso fica fora do adapter Kafka
- o contrato interno é a fronteira entre ingestão e validação

### JetStream Publish Surface

- stream canônico:
  - `DATA_PLANE_INGESTION`
- subject canônico:
  - `dataplane.ingestion.received.<scope-kind>.<scope-key>.<binding-name>`
- durable do validator:
  - `validator-dataplane-v1`

Regra do bloco:

- o subject do JetStream continua derivado do registry
- `consumer` não inventa subjects ad hoc

### Validation Result

O `ValidationResult` mínimo congelado para o Bloco 3 precisa manter:

- identidade da mensagem
- binding e scope
- versão/config ativa
- `status = passed|failed`
- `violations` apenas quando falhou
- `processed_at`

Regra do bloco:

- resultado pequeno e legível
- nada de expandir regra, explainability ou payload replay neste bloco

## Component Execution Slices

### Emulator

- manter bootstrap por bindings ativos
- publicar pelo menos:
  - payload válido
  - payload inválido por campo obrigatório ausente
- preservar evidência por `correlation_id` e logs

### Consumer

- isolar os slices:
  - consumo Kafka
  - mapeamento Kafka -> `dataplane.Message`
  - roteamento por registry
  - publicação no JetStream
- testes devem conseguir falhar no mapper sem subir Kafka/NATS

### Validator

- isolar os slices:
  - consumo JetStream
  - lookup de runtime ativo
  - avaliação mínima
  - persistência/query atual
- `Evaluate` continua sendo a borda funcional mínima do bloco

### Server

- manter leitura operacional fina
- não puxar domínio nem lógica de resolução de runtime para HTTP

## Acceptance Filter For Block 3

Bloco 3 só está pronto quando:

- existe mensagem real no Kafka
- existe mensagem canonical correspondente no JetStream
- existe `ValidationResult` correspondente legível na superfície operacional
- todo esse caminho continua dependente do bootstrap ativo, não de wiring manual

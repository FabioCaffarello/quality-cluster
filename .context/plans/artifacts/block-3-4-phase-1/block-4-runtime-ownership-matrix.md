# Block 4 Runtime Ownership Matrix

## Objective

Congelar a matriz de ownership do Bloco 4 para que o hardening de runtime tenha alvo concreto: menos wiring procedural, mais supervisão explícita e registry forte.

## Current Runtime Anchors

### Consumer

- entrypoint fino:
  - [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/consumer/run.go)
- supervisor atual:
  - [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/supervisor.go)
- runtime atual:
  - [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/runtime.go)
- actors auxiliares já existentes:
  - [bootstrap_actor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/bootstrap_actor.go)
  - [publisher_actor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/publisher_actor.go)
  - [topic_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_router.go)
  - [topic_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/consumer/topic_consumer.go)

### Validator

- entrypoint fino:
  - [run.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/cmd/validator/run.go)
- supervisor atual:
  - [supervisor.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/supervisor.go)
- actors e stores já existentes:
  - [runtime_cache.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_cache.go)
  - [runtime_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_consumer.go)
  - [dataplane_consumer.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/dataplane_consumer.go)
  - [validation_router.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_router.go)
  - [validation_worker.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/validation_worker.go)
  - [results_store.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/results_store.go)
  - [runtime_query_responder.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/runtime_query_responder.go)
  - [results_query_responder.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/actors/scopes/validator/results_query_responder.go)

## Ownership Matrix

| Responsibility | Consumer Target Owner | Validator Target Owner | Must Not Own |
| --- | --- | --- | --- |
| bootstrap | bootstrap actor + runtimebootstrap seam | supervisor bootstrap of runtime cache only | `server`, HTTP handlers |
| consume | topic consumer actor | dataplane consumer actor | `run.go`, stores |
| route | topic router actor | validation router actor | adapters, entrypoints |
| publish/work | publisher actor | validation worker actor | supervisors |
| runtime state | runtime generation state under supervisor/runtime actor | runtime cache actor | HTTP facade |
| results store | none | results store actor | dataplane consumer |
| query | none | runtime/results query responders | domain/application core |
| registry/wiring | registry-backed runtime setup | registry-backed runtime setup | duplicated literals in actors |

## Hardening Decisions

### Decision 1

- `run.go` continua magro.
- Qualquer nova responsabilidade operacional deve nascer em actor, registry ou application seam, não em entrypoint.

### Decision 2

- supervisors podem orquestrar ciclo de vida, mas não devem absorver:
  - parsing de payload
  - avaliação de regra
  - lógica de query
  - literals de routing

### Decision 3

- registry passa a ser a fonte de:
  - topics Kafka
  - subjects JetStream/NATS
  - streams
  - durables
  - nomes operacionais centrais

### Decision 4

- `consumer` e `validator` devem convergir para a mesma disciplina:
  - consume separado
  - route separado
  - work separado
  - store/query separados quando aplicável

## Current Good News

- os entrypoints de `consumer` e `validator` já estão finos
- `consumer` já tem supervisor, bootstrap actor, publisher, routers e topic consumers
- `validator` já tem atores separados para cache, consumo, roteamento, worker, store e responders

## Residual Runtime Smells To Watch

- wiring operacional ainda espalhado por defaults e registries concretos dentro dos supervisors/runtime actors
- uso de `Default...Registry()` dentro do runtime dificultando troca explícita de dependência
- bootstrap inicial do validator ainda concentrado no supervisor
- possibilidade de duplicação entre responsabilidade de runtime actor e supervisor no `consumer`

## Implementation Filter For Block 4

Toda refatoração proposta para o Bloco 4 deve responder claramente:

1. qual ownership ficou mais explícito?
2. qual wiring procedural saiu de `run.go` ou de supervisor?
3. qual literal operacional foi puxado para registry?
4. como a mudança melhora reload futuro, incidentes ou evolução do motor?

Se a resposta não for objetiva para esses quatro pontos, a refatoração não pertence ao bloco.

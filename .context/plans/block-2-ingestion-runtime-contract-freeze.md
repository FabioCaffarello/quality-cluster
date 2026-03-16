# Block 2 Runtime Contract Freeze

> Artefato de `B2-P1-S1`, `B2-P1-S2` e `B2-P1-S3`: congela a hierarquia entre state query, loaded-state do validator, bootstrap da ingestao e evento de runtime para o trilho do novo Bloco 2.

## Scope

- congela apenas o contract fundacional entre `configctl`, `server`, `validator` e dataplane clients
- nao introduz persistencia nova, multiscope, refresh event-driven avancado ou redesign de dataplane message
- separa explicitamente source of truth, trigger de runtime e leitura operacional local

## Baseline Evidence

- registry canonico de `configctl` em `internal/adapters/nats/configctl_registry.go`
- runtime routes HTTP em `internal/interfaces/http/routes/runtime.go`
- runtime handler em `internal/interfaces/http/handlers/runtime.go`
- bootstrap client em `internal/application/runtimebootstrap/client.go`
- dataplane contracts e registry em `internal/application/dataplane/contracts.go` e `internal/application/dataplane/registry.go`
- runtime cache e runtime responder do validator em `internal/actors/scopes/validator/runtime_cache.go` e `internal/actors/scopes/validator/runtime_query_responder.go`

## Freeze Decisions

### Preserve As Source Of Truth

- `configctl.control.list_active_runtime_projections`
  - query canonica do estado ativo de runtime projetado por `configctl`
  - truth de runtime configurado, nao de runtime carregado
- `configctl.control.list_active_ingestion_bindings`
  - query canonica do bootstrap operacional da ingestao
  - derivacao de truth voltada a binding/topic/scope/config/artifact para dataplane clients

### Preserve As Operational Local Read Models

- `GET /runtime/ingestion/bindings`
  - superficie HTTP estavel para inspecao local e bootstrap do futuro `consumer`
- `GET /runtime/validator/active`
  - leitura do runtime carregado no `validator`
  - nao substitui a truth do `configctl`
- `GET /runtime/validator/results`
  - leitura operacional de resultados do `validator`
  - nao participa da truth do runtime da ingestao

### Preserve As Trigger, Not State

- `configctl.events.config.ingestion_runtime_changed`
  - evento de refresh/convergencia
  - nao deve ser tratado como snapshot de runtime
  - nao deve carregar responsabilidade de substituir `list_active_runtime_projections` ou `list_active_ingestion_bindings`

## Contract Hierarchy

| Surface | Owner | Purpose | What It Must Not Become |
| --- | --- | --- | --- |
| `list_active_runtime_projections` | `configctl` | state query canonica do runtime ativo | endpoint de loaded-state do validator |
| `list_active_ingestion_bindings` | `configctl` | bootstrap operacional do dataplane | dump de dominio completo |
| `/runtime/ingestion/bindings` | `server` edge sobre `configctl` | inspecao local e bootstrap HTTP estavel | API de lifecycle ou projecao duplicada de dominio |
| `validator.runtime.get_active` | `validator` | leitura do runtime efetivamente carregado | source of truth da ingestao |
| `config.ingestion_runtime_changed` | `configctl` event plane | trigger de refresh | payload de estado canonico |

## Payload Freeze

### `RuntimeProjectionRecord`

Campos que justificam existir neste contract:

- `scope`
- `config_set_id`
- `config_key`
- `version_id`
- `version`
- `artifact`
- `activated_at`
- `bindings`
- `fields`
- `rules`
- `definition_checksum`

Regra:

- o record continua orientado a projection/runtime truth
- qualquer novo campo precisa justificar bootstrap, inspecao operacional ou coerencia de versionamento

### `ActiveIngestionBindingRecord`

Campos que justificam existir neste contract:

- `binding`
  - `name`
  - `topic`
- `fields`
- `runtime`
  - `scope`
  - `config`
  - `artifact`
  - `activated_at`

Regra:

- o record continua pequeno e orientado a bootstrap
- nao deve passar a carregar regras de lifecycle, decisoes de compilacao ou payloads de evento por conveniencia

## HTTP Freeze

- preservar `/runtime/ingestion/bindings` como principal seam HTTP do bootstrap da ingestao
- preservar `/runtime/validator/active` e `/runtime/validator/results` como seams do validator
- nao adicionar endpoint novo apenas para espelhar a control surface NATS
- se um endpoint HTTP adicional for realmente necessario, ele deve:
  - viver em `/runtime/*`
  - ser pequeno e operacional
  - continuar sendo facade do `configctl`
  - nao reabrir aliases como ocorreu no lifecycle do Bloco 1

## Preserve Targets For Block 2

- `configctl` como owner do runtime ativo
- `server` como borda HTTP fina
- `validator` como loaded-state minimo e query de resultados
- `runtimebootstrap.Client` usando `/runtime/ingestion/bindings`
- distincao entre state query e refresh event

## Removal Targets For Block 2

- qualquer interpretacao de `validator.runtime.get_active` como truth de ingestao
- qualquer crescimento do `server` para orquestrar ou compor dominio de runtime
- qualquer tendencia de usar `config.ingestion_runtime_changed` como fonte de snapshot
- qualquer expansao do payload de bindings sem justificativa operacional explicita

## Immediate Next Steps

- `B2-P1-S4`: fechar a escada de validacao do bloco
- `B2-P2-S1`: consolidar contracts/gateways com essa hierarquia como regra
- `B2-P2-S2`: endurecer apenas a superficie HTTP realmente necessaria

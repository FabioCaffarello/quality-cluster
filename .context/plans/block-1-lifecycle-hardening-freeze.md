# Block 1 Lifecycle Freeze

> Artefato de `B1-P1-S1` e `B1-P1-S2`: congela a superficie canonica do lifecycle entre `server`, `configctl` e `validator` e inventaria aliases, envelopes antigos e pontos de drift em tooling e `.http`.

## Scope

- congela apenas a superficie ja existente no repositorio
- nao inventa endpoints novos
- separa o que deve ser preservado, o que deve ser deprecado e o que esta objetivamente stale

## Baseline Evidence

- routes HTTP de `configctl` expostas em `internal/interfaces/http/routes/configctl.go`
- envelopes HTTP reais definidos em `internal/interfaces/http/handlers/configctl.go`
- envelopes runtime definidos em `internal/interfaces/http/handlers/runtime.go`
- `.http` operacionais em `tests/http/configctl.http` e `tests/http/lifecycle.http`
- smoke e trace tooling em `tools/raccoon-cli/src/smoke` e `tools/raccoon-cli/src/trace_pack`
- `contract-audit` do `raccoon-cli` executado com sucesso em `2026-03-16`

## Freeze Decisions

### Preserve

- `POST /configctl/configs` como endpoint de criacao de draft
- familia `GET/POST /configctl/config-versions/...` como superficie canonica de leitura e transicao de versoes persistidas
- `GET /configctl/configs/active` como read model canonico da configuracao ativa por scope
- runtime query como ja esta:
  - `GET /runtime/validator/active`
  - `GET /runtime/ingestion/bindings`
  - `GET /runtime/validator/results`
- envelopes HTTP com wrappers explicitos:
  - `config`
  - `configs`
  - `validation`
  - `runtime`
  - `bindings`
  - `results`

### Deprecate And Remove In Block 1

- `GET /configctl/configs` como alias de listagem de versoes
- `GET /configctl/configs/by-id?id=...` como alias de lookup
- `GET /configctl/active-config` como alias de config ativa

### Preserve For Later Blocks, Not For This Cleanup

- `results` em memoria no `validator`
- `runtime/ingestion/bindings` como bootstrap surface de dataplane
- `server` como facade HTTP fina

## Canonical Lifecycle Matrix

| Intent | Canonical Endpoint | Method | Canonical Response Shape | Decision |
| --- | --- | --- | --- | --- |
| Create draft | `/configctl/configs` | `POST` | `{ "status": "created", "config": ConfigVersionDetail }` | preserve |
| Dry-run validate source | `/configctl/configs/validate` | `POST` | `{ "status": "valid|invalid", "validation": ValidateDraftReply }` | preserve |
| List persisted versions | `/configctl/config-versions` | `GET` | `{ "configs": []ConfigVersionSummary }` | preserve |
| Get persisted version | `/configctl/config-versions/:id` | `GET` | `{ "config": ConfigVersionDetail }` | preserve |
| Validate persisted version | `/configctl/config-versions/:id/validate` | `POST` | `{ "status": "validated|invalid", "validation": ValidateConfigReply }` | preserve |
| Compile persisted version | `/configctl/config-versions/:id/compile` | `POST` | `{ "status": "compiled", "config": ConfigVersionDetail }` | preserve |
| Activate persisted version | `/configctl/config-versions/:id/activate` | `POST` | `{ "status": "activated", "config": ConfigVersionDetail, "activation": ActivationRecord, "projection": RuntimeProjectionRecord }` | preserve |
| Get active config by scope | `/configctl/configs/active` | `GET` | `{ "config": ConfigVersionDetail }` | preserve |
| Get validator runtime | `/runtime/validator/active` | `GET` | `{ "runtime": ActiveRuntimeRecord }` | preserve |
| Get active ingestion bindings | `/runtime/ingestion/bindings` | `GET` | `{ "bindings": []ActiveIngestionBindingRecord }` | preserve |
| Get validation results | `/runtime/validator/results` | `GET` | `{ "results": []ValidationResultRecord }` | preserve |

## Alias And Drift Inventory

### HTTP Route Duplication

| Surface | Current State | Recommendation | Why |
| --- | --- | --- | --- |
| `/configctl/configs` `GET` | alias para `ListConfigs` | deprecate | mistura drafts/configs com listagem de versoes persistidas; `config-versions` ja e a rota semantica correta |
| `/configctl/configs/by-id?id=...` | alias para `GetConfig` | remove | semantica inferior a `/configctl/config-versions/:id` e mantem lookup por query param desnecessario |
| `/configctl/active-config` | alias para `GetActiveConfig` | remove | `trace-pack` e `smoke/api.rs` ja usam `/configctl/configs/active`; manter as duas rotas so espalha legado |

### Tooling Drift In `raccoon-cli`

| Location | Current Assumption | Actual Shape | Action |
| --- | --- | --- | --- |
| `tools/raccoon-cli/src/smoke/stages.rs` | `create_draft` retorna `id` no topo | retorna `config.id` | corrigir |
| `tools/raccoon-cli/src/smoke/scenarios.rs` | `create_draft` retorna `id` no topo | retorna `config.id` | corrigir |
| `tools/raccoon-cli/src/smoke/scenarios.rs` | `validate_config` retorna `valid` no topo | retorna `validation.valid` | corrigir |
| `tools/raccoon-cli/src/smoke/scenarios.rs` | `compile_config` retorna `artifact` no topo | retorna `config.artifact` | corrigir |
| `tools/raccoon-cli/src/smoke/scenarios.rs` | `get_active_config` retorna `id` no topo | retorna `config.id` | corrigir |

### `.http` Drift

| File | Current Drift | Action |
| --- | --- | --- |
| `tests/http/configctl.http` | usa `/configctl/active-config` | migrar para `/configctl/configs/active` |
| `tests/http/lifecycle.http` | usa `/configctl/active-config` | migrar para `/configctl/configs/active` |

### Test Drift

| File | Current Drift | Action |
| --- | --- | --- |
| `internal/interfaces/http/routes/configctl_test.go` | cobre aliases como se fossem rota primaria | manter apenas enquanto a remocao nao ocorrer; depois reduzir para superficie canonica |
| `internal/interfaces/http/handlers/configctl_test.go` | ainda valida `configs/by-id` em mapeamento de problema | mover o teste para `/configctl/config-versions/:id` quando o alias sair |

### Tooling Already Aligned And Worth Preserving

| Surface | Reason To Preserve |
| --- | --- |
| `tools/raccoon-cli/src/smoke/api.rs` | ja usa `/configctl/configs/active`, que e a melhor rota canonica existente para config ativa |
| `tools/raccoon-cli/src/trace_pack/collect.rs` | coleta `active-config.json` via `/configctl/configs/active`, coerente com a rota que deve permanecer |
| runtime handlers e tests | envelopes `runtime`, `bindings` e `results` estao consistentes com contracts e nao precisam de redesign neste bloco |

## Why This Canonical Surface

- `config-versions` ja e a familia que organiza leitura, validate, compile e activate de versoes persistidas; listar versoes sob `/configctl/configs` so aumenta ambiguidade
- `configs/active` ja e consumido por tooling de diagnostico e pelo cliente de smoke; ele vence `active-config` por aderencia pratica e menor drift
- os wrappers `config`, `validation`, `runtime`, `bindings` e `results` ja estao refletidos nos handlers e tests; remover esses envelopes agora aumentaria acoplamento e quebraria estabilidade sem ganho real
- runtime query e read models HTTP estao mais consistentes que a superficie de `configctl`; o cleanup do bloco deve focar primeiro onde a duplicacao realmente existe

## Removal Targets For Block 1

- remover `GET /configctl/configs/by-id`
- remover `GET /configctl/active-config`
- deprecar e depois remover `GET /configctl/configs`
- remover expectativas de payload top-level em `raccoon-cli` smoke
- remover referencias legadas em `.http` e testes de rota

## Preserve Targets For Block 1

- manter `POST /configctl/configs`
- manter `GET /configctl/config-versions`
- manter `GET /configctl/config-versions/:id`
- manter `GET /configctl/configs/active`
- manter os tres endpoints de runtime query sem alteracao de shape
- manter os defaults `scope_kind=global` e `scope_key=default` como semantica canonica para consultas sem scope explicito

## Ready For Next Planning Steps

- `B1-P1-S1` pode ser tratado como fechado: a matriz canonica esta definida
- `B1-P1-S2` pode ser tratado como fechado: o inventario de alias, drift e legado esta definido
- os proximos passos corretos sao `B1-P1-S3` e `B1-P1-S4`:
  - definir o contrato de `readyz` do `server`
  - definir o seam de startup/replay do `validator`

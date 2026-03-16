# Block 8 Platform Work Packages

## Objective

Quebrar a consolidacao da plataforma em pacotes pequenos, implementaveis e validaveis, mantendo o `raccoon-cli` e o workflow do repositório como plano de engenharia principal.

## Execution Order

### WP-B78-1 DSL Contract And Capability Versioning

**Goal**

Fechar o contrato de evolucao da DSL em `configctl` e tornar capabilities explicitamente versionadas.

**Primary files**

- [document.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/domain/configctl/document.go)
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/domain/configctl/runtime.go)
- [compile_config.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/configctl/compile_config.go)
- contracts e mappers em `internal/application/configctl/`

**Expected outcome**

- crescimento aditivo e version-aware da DSL
- artifact e compile metadata dizem o que o runtime suporta
- projection deixa de depender de interpretacao implicita

**Validation**

- `go test ./internal/domain/configctl ./internal/application/configctl`
- `raccoon-cli contract-audit`
- `raccoon-cli drift-detect`

### WP-B78-2 Projection And Query Explainability

**Goal**

Amadurecer projections e queries de runtime para inspeção e auditoria sem inflar o HTTP facade.

**Primary files**

- [config.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/configctl/contracts/config.go)
- [mappers.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/configctl/mappers.go)
- [configctl.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/interfaces/http/handlers/configctl.go)
- [runtime.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/interfaces/http/handlers/runtime.go)

**Expected outcome**

- projections e active runtime views ficam mais explicaveis
- query surfaces continuam finas e aditivas
- operator e artifact provenance ficam legiveis sem abrir ownership paralelo

**Validation**

- `go test ./internal/interfaces/http/... ./internal/application/configctl/...`
- `make verify`

### WP-B78-3 Validator Execution And Result Explainability

**Goal**

Executar runtime mais rico no `validator` sem permitir autoria local e com explainability suficiente em results/incidents.

**Primary files**

- [evaluate.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/validatorresults/evaluate.go)
- [results.go](/Volumes/OWC%20Express%201M2/Develop/quality-service/internal/application/validatorresults/contracts/results.go)
- actors em `internal/actors/scopes/validator/`

**Expected outcome**

- operadores suportados ficam explicitos
- mismatch de capacidade falha de forma clara
- resultados e incidentes carregam provenance util sem virar analytics payload

**Validation**

- `go test ./internal/application/validatorresults ./internal/actors/scopes/validator`
- `make scenario-smoke SCENARIO=invalid-payload`
- `make results-inspect`

### WP-B78-4 CLI Guard Rails And Baseline Governance

**Goal**

Fazer o `raccoon-cli` refletir a nova etapa da linguagem e reduzir drift antes da execucao ao vivo.

**Primary files**

- analyzers em `tools/raccoon-cli/src/analyzers/`
- [main.rs](/Volumes/OWC%20Express%201M2/Develop/quality-service/tools/raccoon-cli/src/main.rs)
- [README.md](/Volumes/OWC%20Express%201M2/Develop/quality-service/tools/raccoon-cli/README.md)

**Expected outcome**

- recommend, drift, contracts e baselines apontam para as invariantes novas
- o CLI explica melhor o que validar e por que
- governanca de plataforma fica no workflow canonico, nao em memoria informal

**Validation**

- `make raccoon-test`
- `make check`
- `make quality-gate-ci`

### WP-B78-5 Canonical Smoke, Diagnostics And Evidence Pack

**Goal**

Consolidar smoke, inspect e trace-pack como prova e diagnostico da nova etapa da linguagem.

**Primary files**

- smoke em `tools/raccoon-cli/src/smoke/`
- inspect em `tools/raccoon-cli/src/results_inspect/`
- trace em `tools/raccoon-cli/src/trace_pack/`
- cenarios e assets em `tests/http/`

**Expected outcome**

- smoke prova lifecycle, runtime richer rules e loaded-state
- inspect e trace-pack explicam o estado do motor sem depender de leitura manual dispersa
- evidence pack vira parte normal do fechamento de mudancas runtime-significant

**Validation**

- `make scenario-smoke SCENARIO=config-lifecycle`
- `make scenario-smoke SCENARIO=happy-path`
- `make trace-pack`
- `make check-deep`

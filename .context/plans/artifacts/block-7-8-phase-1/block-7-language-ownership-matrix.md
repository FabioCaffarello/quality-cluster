# Block 7 Language Ownership Matrix

## Objective

Congelar quem pode autorar, compilar, executar, consultar e governar a linguagem de qualidade antes da proxima expansao da DSL.

## Ownership Matrix

| Responsibility | Target Owner | Must Not Own | Notes |
| --- | --- | --- | --- |
| DSL source authoring | `configctl` draft/validate lifecycle | `validator`, `consumer`, `emulator`, `server`, `raccoon-cli` | a linguagem nasce na config, nunca em runtime local |
| capability/version gating | `configctl` compile path | `validator` inline fallbacks | `schema_version`, `runtime_loader` e `compiler_version` definem o envelope suportado |
| runtime projection shape | `configctl` contracts and mappers | HTTP handlers, dataplane clients | projection madura e compilada, nao inferida ad hoc |
| rule execution | `validator` over compiled runtime | `configctl` request handlers, `consumer` bridge | executor recebe projection pronta e falha explicitamente no que nao suporta |
| dataplane bootstrap | `configctl` active runtime queries | `consumer`/`emulator` local truth | bootstrap continua view derivada da runtime truth |
| operational query facade | `server` | domain/application ownership | query surfaces continuam finas e transport-backed |
| quality governance | `raccoon-cli` + canonical make workflow | runtime services as ad hoc governance | tooling observa, prova e sinaliza; nao recompila DSL |

## Ownership Rules

### Configctl Must Own

- validacao sintatica e semantica da DSL
- compilacao para artifact e projection
- versionamento de capacidade da linguagem
- publicacao de contracts canonicamente mapeados

### Validator Must Own

- loaded-state do runtime compilado
- execucao deterministica por payload
- explainability de resultado/incidente derivada da projection
- falha explicita para capacidade nao suportada

### Consumer And Emulator Must Not Own

- parsing semantico da DSL
- inferencia de capacidade de regra
- completude de projection via heuristica local

### Raccoon CLI Must Own

- drift detection entre source, projection, contracts, docs e runtime assumptions
- recomendacao de validacao, baseline e evidence pack
- diagnostico acionavel sobre o estado do sistema

### Raccoon CLI Must Not Own

- semantic truth da DSL
- compilacao de artifacts
- correcoes silenciosas de contract ou projection

## Entry Filters For Any Language Change

Toda proposta de evolucao da linguagem precisa responder:

1. onde a capacidade nasce em `configctl`?
2. como ela vira projection ou artifact explicito?
3. como `validator` executa ou rejeita essa capacidade?
4. como `raccoon-cli` prova e diagnostica a mudanca?

Se qualquer resposta ficar implicita, a mudanca ainda nao pertence ao bloco.

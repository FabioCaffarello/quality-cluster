# Block 7 DSL Expansion Envelope

## Objective

Definir o envelope seguro para a proxima expansao da linguagem de qualidade sem abrir statefulness nem empurrar semantica para fora de `configctl`.

## Current Baseline

- operators atuais:
  - `required`
  - `not_empty`
  - `equals`
- execution model atual:
  - deterministico por payload JSON
  - sem estado compartilhado
  - sem correlacao entre mensagens
  - sem lookup externo

## Allowed Expansion Shape

### Allowed Families

- comparadores escalares por campo, desde que tipados e deterministicos
- checks tipados por campo, quando o tipo ja existir na DSL e na projection
- metadata adicional de regra para explainability, gravidade e governanca
- enrichments pequenos de projection que melhorem explainability e auditabilidade

### Admission Criteria

Uma capacidade nova so entra se cumprir todas as condicoes:

1. pode ser validada em `ConfigDocument` sem heuristica fraca
2. pode ser compilada em artifact/projection explicitos
3. pode ser executada pelo `validator` sem depender de estado historico
4. pode ser explicada em `ValidationResult` ou `ValidationIncident` sem payload inflado
5. pode ser provada por teste, smoke ou diagnostico no workflow canonico
6. pode ser protegida por contract/drift checks no `raccoon-cli`

## Explicitly Deferred

- correlacao entre mensagens
- janelas temporais
- agregacoes
- regras dependentes de banco externo ou servico externo
- funcoes arbitrarias ou linguagem geral de expressoes
- side effects em tempo de avaliacao
- policy/alerting workflow embutido na DSL

## Projection Contract Requirements

Cada nova capacidade de linguagem precisa refletir:

- capability/version source em artifact ou compile metadata
- projection suficientemente rica para explainability
- contrato additivo e retrocompativel em query surfaces
- falha explicita no runtime quando houver mismatch de capacidade

## Release Rule

A ordem de liberacao continua sendo:

1. validar a linguagem
2. compilar para projection/artifact
3. executar no `validator`
4. inspecionar e diagnosticar no workflow
5. congelar docs e guard rails

Se um desses passos faltar, a capacidade nao sai do envelope.

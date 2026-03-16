---
status: filled
generated: 2026-03-16
updated: 2026-03-16
agents:
  - type: "cli-evolution"
    role: "Endurecer o raccoon-cli como prova estática e operacional do refresh orientado a evento"
  - type: "contract-guardian"
    role: "Proteger os durables, subjects e streams envolvidos em config.ingestion_runtime_changed"
  - type: "runtime-validator"
    role: "Provar que o diagnóstico do refresh orientado a evento continua aderente ao cluster real"
  - type: "tdd-coordinator"
    role: "Fechar a escada mínima de testes para analyzers, smoke e deep gate"
  - type: "documentation-writer"
    role: "Atualizar docs e workflow do contexto com o novo foco de diagnóstico"
docs:
  - "cluster-quality.md"
  - "messaging-contracts.md"
  - "tooling-raccoon-cli.md"
phases:
  - id: "phase-1"
    name: "Freeze Diagnostics Gap"
    prevc: "P"
    agent: "cli-evolution"
  - id: "phase-2"
    name: "Static Proof Parity"
    prevc: "E"
    agent: "cli-evolution"
  - id: "phase-3"
    name: "Live Evidence And Workflow Update"
    prevc: "V"
    agent: "runtime-validator"
---

# Block 6 - Event Refresh Diagnostics Hardening Plan

> Endurecer o motor de qualidade para que o refresh orientado a `config.ingestion_runtime_changed` não fique correto só no runtime, mas também auditável e diagnosticável pelo `raccoon-cli`.

## Task Snapshot

- **Primary goal:** fechar o gap entre o runtime pós-Bloco 5 e os analyzers do `raccoon-cli`, e depois ampliar a prova operacional para falhas de refresh, durables e JetStream.
- **Success signal:** `topology-doctor`, `runtime-bindings`, `drift-detect`, `contract-audit`, `scenario-smoke` e `check-deep` passam conhecendo o modelo canônico de refresh por evento e apontam drift real quando ele quebrar.
- **Out of scope:** redesign do runtimebootstrap, troca do transporte NATS/JetStream, alteração da API HTTP pública, ou paralelismo de smoke.
- **Key references:**
  - [Cluster Quality](../docs/cluster-quality.md)
  - [Messaging Contracts](../docs/messaging-contracts.md)
  - [Tooling Raccoon CLI](../docs/tooling-raccoon-cli.md)
  - [Block 5 Event Driven Dataplane Refresh](./block-5-event-driven-dataplane-refresh.md)
  - [Block 5 Phase 3 Validation](./block-5-phase-3-validation.md)

## Codebase Context

### Current Diagnostic

- o Bloco 5 deixou `consumer` e `emulator` dependentes de durables dedicados de `config.ingestion_runtime_changed`:
  - `consumer-runtime-refresh-v1`
  - `emulator-runtime-refresh-v1`
- `emulator` agora também depende de NATS para refresh, além de Kafka e bootstrap HTTP.
- o primeiro corte deste bloco já foi aplicado:
  - `topology-doctor` agora exige NATS no `emulator`, `depends_on: nats` no Compose e presença dos durables de refresh;
  - `runtime-bindings` agora trata os durables de refresh como parte do contrato estático do dataplane;
  - `drift-detect` agora falha se os durables de refresh apontarem para stream errada.
- `trace-pack` já coleta `nats/healthz.json` e `nats/jsz.json`, e o stage `consume` já tenta diagnosticar bindings ativos e runtime do validator antes de falhar seco.
- o gap remanescente não é mais de wiring básico; é a prova profunda em cluster vivo com `happy-path` e `check-deep`.

### Preserve These Decisions

- `config.ingestion_runtime_changed` continua sendo o gatilho canônico do refresh do dataplane.
- o bootstrap agregado continua sendo a fonte de verdade do estado efetivo.
- o `raccoon-cli` continua fora do runtime e prova o sistema pela borda do repositório.
- `contract-audit`, `runtime-bindings`, `topology-doctor` e `drift-detect` continuam sendo a primeira linha de defesa antes de smoke.

## Agent Lineup

| Agent | Role in this plan | Playbook | First responsibility focus |
| --- | --- | --- | --- |
| CLI Evolution | endurecer analyzers, smoke e trace-pack para o refresh dirigido a evento | [cli-evolution](../agents/cli-evolution.md) | cobertura estática e diagnóstica do raccoon-cli |
| Contract Guardian | proteger stream, subjects e durables do runtime-change | [contract-guardian](../agents/contract-guardian.md) | evitar drift entre registry e analyzers |
| Runtime Validator | provar o comportamento no cluster real | [runtime-validator](../agents/runtime-validator.md) | smoke e deep gate com foco em refresh |
| TDD Coordinator | escolher a ladder mínima e suficiente | [tdd-coordinator](../agents/tdd-coordinator.md) | `make raccoon-test`, analyzers e smoke certo |
| Documentation Writer | consolidar a verdade operacional | [documentation-writer](../agents/documentation-writer.md) | docs canônicas e artefatos de validação |

## Documentation Touchpoints

| Guide | File | Why It Changes |
| --- | --- | --- |
| Cluster Quality | [cluster-quality.md](../docs/cluster-quality.md) | deixar explícito como provar e diagnosticar refresh orientado a evento |
| Messaging Contracts | [messaging-contracts.md](../docs/messaging-contracts.md) | registrar o papel dos durables de refresh e sua relação com `CONFIGCTL_EVENTS` |
| Tooling Raccoon CLI | [tooling-raccoon-cli.md](../docs/tooling-raccoon-cli.md) | alinhar o que o CLI deve detectar e provar sobre refresh por evento |

## Risk Assessment

### Identified Risks

| Risk | Probability | Impact | Mitigation Strategy | Owner |
| --- | --- | --- | --- | --- |
| analyzers ficarem atrasados em relação ao runtime e produzirem falso verde | Medium | High | congelar invariantes canônicos e cobri-las com testes do CLI | `cli-evolution` |
| drift de durable/stream quebrar refresh silenciosamente | Medium | High | falhar cedo em `runtime-bindings`, `drift-detect` e `contract-audit` | `contract-guardian` |
| prova viva continuar verde sem evidência suficiente para troubleshooting | Medium | Medium | ampliar `trace-pack` e roteiro de smoke para refresh failures | `runtime-validator` |

### Dependencies

- **Internal:** `tools/raccoon-cli/src/analyzers/*`, `tools/raccoon-cli/src/smoke/*`, `tools/raccoon-cli/src/trace_pack/*`, `internal/adapters/nats/configctl_registry.go`
- **External:** cluster local com NATS/JetStream saudável para prova final
- **Technical:** manter o contrato do Bloco 5 estável enquanto o motor de qualidade ganha cobertura

### Assumptions

- os durables de refresh recém-introduzidos são permanentes, não transitórios;
- o próximo ganho de valor vem do motor de qualidade e da evidência operacional, não de mais um refactor de runtime.

## Working Phases

### Phase 1 - Freeze Diagnostics Gap
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** congelar o gap entre o runtime pós-Bloco 5 e o que o `raccoon-cli` ainda provava.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | inventariar invariantes novas de refresh orientado a evento | `cli-evolution` + `contract-guardian` | completed | freeze do gap |
| 1.2 | decidir o menor corte de endurecimento do CLI | `cli-evolution` + `tdd-coordinator` | completed | escopo estático do bloco |

### Phase 2 - Static Proof Parity
> **Primary Agent:** `cli-evolution` - [Playbook](../agents/cli-evolution.md)

**Objective:** fazer os analyzers e a suíte do `raccoon-cli` conhecerem o modelo canônico pós-Bloco 5.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | exigir NATS no `emulator` e durables de refresh em `topology-doctor` | `cli-evolution` | completed | analyzer e testes atualizados |
| 2.2 | endurecer `runtime-bindings` contra ausência ou drift dos durables de refresh | `cli-evolution` + `contract-guardian` | completed | checks estáticos atualizados |
| 2.3 | endurecer `drift-detect` para stream alvo de refresh | `cli-evolution` | completed | drift explícito em `CONFIGCTL_EVENTS` |
| 2.4 | validar o motor com suíte e comandos canônicos | `tdd-coordinator` + `cli-evolution` | completed | `make raccoon-test`, analyzers verdes |

### Phase 3 - Live Evidence And Workflow Update
> **Primary Agent:** `runtime-validator` - [Playbook](../agents/runtime-validator.md)

**Objective:** ampliar a prova viva e o troubleshooting do refresh orientado a evento, agora que a paridade estática foi fechada.

**Tasks**

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | decidir se `trace-pack` deve coletar evidência específica de durables/refresh | `runtime-validator` + `cli-evolution` | completed | recorte de diagnóstico |
| 3.2 | endurecer smoke/troubleshooting para falhas de refresh silencioso | `runtime-validator` | completed | prova viva mais diagnóstica |
| 3.3 | atualizar docs e artefato de validação do bloco | `documentation-writer` | completed | docs canônicas + validation artifact |

## Validation Ladder

**Before**

- `raccoon-cli recommend tools/raccoon-cli/src/analyzers/topology.rs tools/raccoon-cli/src/analyzers/runtime_bindings.rs tools/raccoon-cli/src/analyzers/drift_detect.rs`
- `raccoon-cli contract-audit`

**During**

- `make raccoon-test`
- `raccoon-cli topology-doctor`
- `raccoon-cli runtime-bindings`
- `raccoon-cli drift-detect`

**After**

- `raccoon-cli contract-audit`
- `make verify`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make check-deep`

## Done Definition

Este bloco só termina quando:

- o `raccoon-cli` falhar cedo para drift real do refresh orientado a evento;
- a prova estática e a prova viva contarem a mesma história sobre `config.ingestion_runtime_changed`;
- troubleshooting de refresh não depender mais só de leitura manual de logs dispersos;
- o `.context` refletir esse novo padrão de manutenção do motor de qualidade.

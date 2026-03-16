---
status: filled
generated: 2026-03-16
agents:
  - type: "documentation-writer"
    role: "Consolidar a documentacao do contexto com base no repositorio real"
  - type: "feature-developer"
    role: "Executar ajustes estruturais necessarios no .context e workflow"
  - type: "code-reviewer"
    role: "Verificar consistencia, links e aderencia ao fluxo do repositorio"
  - type: "test-writer"
    role: "Definir a verificacao minima para manter o contexto confiavel"
docs:
  - "project-overview.md"
  - "development-workflow.md"
  - "testing-strategy.md"
  - "tooling.md"
phases:
  - id: "phase-1"
    name: "Contexto Base"
    prevc: "P"
    agent: "documentation-writer"
  - id: "phase-2"
    name: "Vinculacao e Operacionalizacao"
    prevc: "E"
    agent: "feature-developer"
  - id: "phase-3"
    name: "Validacao do Bootstrap"
    prevc: "V"
    agent: "code-reviewer"
---

# Bootstrap do ai-context no quality-service Plan

> Consolidar a inicializacao do `.context`, revisar a documentacao base gerada, vincular um plano ao workflow PREVC e deixar o repositorio pronto para execucoes estruturadas futuras.

## Task Snapshot

- **Primary goal:** deixar o workspace com `.context/` utilizavel, documentacao base preenchida e workflow PREVC ativo com um plano vinculado.
- **In scope:** documentacao de contexto, playbooks de agentes, plano de bootstrap e estado do workflow.
- **Out of scope:** implementacao de features do produto, preenchimento das skills geradas automaticamente e execucao de validacoes pesadas do runtime.
- **Success signal:** `context.check` retorna `initialized: true`, os docs/agentes principais estao preenchidos, o plano esta linkado ao workflow e a fase sai de `Planning`.

## Repository Context

- **Workspace root:** `/Volumes/OWC Express 1M2/Develop/quality-service`
- **Project shape:** workspace Go com multiplos modulos e uma CLI Rust em `tools/raccoon-cli`.
- **Operational entrypoints:** `cmd/configctl`, `cmd/server`, `cmd/consumer`, `cmd/validator`, `cmd/emulator`.
- **Core references:** `DEVELOPMENT.md`, `Makefile`, `go.work`, `deploy/compose/docker-compose.yaml`, `tools/raccoon-cli/README.md`.

## Agent Lineup

| Agent | Role in this plan | Playbook | Focus |
| --- | --- | --- | --- |
| Documentation Writer | Consolidar e corrigir a documentacao do contexto | [Documentation Writer](../agents/documentation-writer.md) | Remover scaffolding generica e alinhar comandos/arquitetura |
| Feature Developer | Executar bootstrap do workflow e vinculacao do plano | [Feature Developer](../agents/feature-developer.md) | Deixar `.context` operacional |
| Code Reviewer | Verificar coerencia do estado final | [Code Reviewer](../agents/code-reviewer.md) | Confirmar links, status e gate do workflow |
| Test Writer | Definir a verificacao minima do bootstrap | [Test Writer](../agents/test-writer.md) | Validar checks do MCP e ausencia de placeholders criticos |

## Documentation Touchpoints

| Guide | File | Why it matters |
| --- | --- | --- |
| Project Overview | [project-overview.md](../docs/project-overview.md) | Explica a arquitetura real do workspace e seus componentes |
| Development Workflow | [development-workflow.md](../docs/development-workflow.md) | Registra o fluxo canonico baseado em `make check`, `make verify` e `raccoon-cli` |
| Testing Strategy | [testing-strategy.md](../docs/testing-strategy.md) | Resume testes Go, CLI Rust e smoke/runtime validation |
| Tooling | [tooling.md](../docs/tooling.md) | Documenta Makefile, scripts, Compose e `raccoon-cli` |
| Agent Handbook | [../agents/README.md](../agents/README.md) | Centraliza como os playbooks devem ser usados no repositorio |

## Risks And Mitigations

| Risk | Probability | Impact | Mitigation | Owner |
| --- | --- | --- | --- | --- |
| Scaffolding generica continuar no contexto | Medium | Medium | Revisar e substituir placeholders manualmente | `documentation-writer` |
| Workflow ficar travado sem plano vinculado | High | Medium | Criar plano especifico e usar `plan.link` antes de avancar | `feature-developer` |
| Contexto documentar comportamento inexistente | Medium | High | Basear todo o conteudo em arquivos reais do repositorio | `code-reviewer` |
| Persistirem artefatos pendentes fora do escopo imediato | Medium | Low | Registrar que skills auto-geradas ficam fora do bootstrap atual | `test-writer` |

## Working Phases

### Phase 1 — Contexto Base
> **Primary Agent:** `documentation-writer`

**Objective:** preencher e corrigir os documentos de contexto e os playbooks principais com informacoes extraidas do repositorio real.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 1.1 | Inspecionar `DEVELOPMENT.md`, `Makefile`, `go.work`, `deploy/` e `tools/raccoon-cli` | `documentation-writer` | completed | Fonte de verdade para o contexto |
| 1.2 | Preencher `.context/docs/*.md` com conteudo especifico do repositorio | `documentation-writer` | completed | Documentacao base preenchida |
| 1.3 | Preencher `.context/agents/*.md` com playbooks uteis para este workspace | `documentation-writer` | completed | Playbooks utilizaveis |
| 1.4 | Corrigir links quebrados e indices genericos | `documentation-writer` | completed | READMEs consistentes |

**Commit Checkpoint**
- `chore(context): fill base docs and agent playbooks`

### Phase 2 — Vinculacao e Operacionalizacao
> **Primary Agent:** `feature-developer`

**Objective:** criar um plano de bootstrap, linkar ao workflow e deixar o PREVC pronto para execucao futura.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 2.1 | Inicializar workflow PREVC para o repositorio | `feature-developer` | completed | `.context/workflow/status.yaml` criado |
| 2.2 | Gerar plano especifico de bootstrap | `feature-developer` | completed | `plans/context-bootstrap.md` preenchido |
| 2.3 | Vincular o plano ao workflow | `feature-developer` | pending | Workflow com plano associado |
| 2.4 | Avancar a fase apos satisfazer o gate | `feature-developer` | pending | Workflow fora de `Planning` |

**Commit Checkpoint**
- `chore(workflow): link context bootstrap plan`

### Phase 3 — Validacao do Bootstrap
> **Primary Agent:** `code-reviewer`

**Objective:** confirmar que o contexto ficou coerente, navegavel e suficiente para o proximo trabalho estruturado.

| # | Task | Agent | Status | Deliverable |
|---|------|-------|--------|-------------|
| 3.1 | Confirmar `context.check` com docs/agentes inicializados | `code-reviewer` | pending | Evidencia do estado do contexto |
| 3.2 | Confirmar workflow, fase atual e plano linkado | `code-reviewer` | pending | Evidencia do status PREVC |
| 3.3 | Registrar limites conhecidos do bootstrap | `code-reviewer` | pending | Risco residual documentado |

**Commit Checkpoint**
- `chore(context): validate bootstrap workflow state`

## Validation Requirements

- `context.check` deve indicar `.context` inicializado.
- Os arquivos principais em `docs/` e `agents/` nao podem permanecer com `status: unfilled`.
- O plano `context-bootstrap` deve estar vinculado ao workflow.
- O workflow deve conseguir avancar de `P` para a fase seguinte permitida.

## Rollback Plan

- Se o contexto ficar incoerente, restaurar apenas os arquivos em `.context/` alterados no bootstrap.
- Se o workflow PREVC for inicializado com configuracao inadequada, remover ou recriar o estado em `.context/workflow/`.
- Nenhum rollback de dados de aplicacao e necessario, porque o escopo e apenas documental/operacional.

## Artifacts

- `.context/docs/*.md`
- `.context/agents/*.md`
- `.context/plans/context-bootstrap.md`
- `.context/workflow/status.yaml`

## Success Criteria

1. O contexto pode ser usado como base para futuras tarefas no repositorio.
2. O workflow PREVC deixa de depender de scaffolding vazia.
3. Os documentos refletem a arquitetura e o fluxo reais do `quality-service`.
4. O bootstrap termina sem placeholders criticos nas pecas centrais do `.context`.

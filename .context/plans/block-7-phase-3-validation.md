---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-7-refresh-reconciliation-hardening"
phase: "phase-3"
---

# Block 7 Phase 3 Validation

## Validation Goal

Provar que `consumer` e `emulator` agora tem um caminho bounded de auto-cura por bootstrap agregado, sem perder o refresh imediato por `config.ingestion_runtime_changed` e sem regredir o workflow do `raccoon-cli`.

## Implemented Outcome

- `bootstrap.reconcile_interval` entrou na config compartilhada com default e uso explícito nos configs de `consumer` e `emulator`.
- `consumer bootstrap_actor` agora reconcilia o bootstrap agregado periodicamente, mas só publica novo estado quando a assinatura muda.
- `emulator` usa a mesma semântica de fallback antes de continuar a publicação sintética.
- a narrativa operacional do `.context` foi atualizada para deixar claro que evento é primário e reconciliação é fallback.

## Evidence

### Focused Tests

- `go test ./internal/shared/settings ./internal/actors/scopes/consumer ./cmd/emulator`

### Static And Fast Guard Rails

- `raccoon-cli runtime-bindings`
- `make verify`

### Deep Runtime Proof

- `make scenario-smoke SCENARIO=happy-path`
- `make check-deep`

## Result

Todos os checks acima passaram.

Isso fecha a evidência mínima do bloco:

- o dataplane continua convergindo por evento quando o caminho rápido está saudável
- o runtime agora também tem fallback bounded de auto-cura via bootstrap agregado
- a assinatura do binding set continua impedindo churn e reload desnecessário
- os guard rails e o smoke seguem verdes no fluxo canônico do repositório

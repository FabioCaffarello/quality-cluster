---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-7-refresh-reconciliation-hardening"
phase: "phase-1"
---

# Block 7 Refresh Reconciliation Freeze

## Canonical Gap

Depois do Bloco 6, o refresh do dataplane esta correto e diagnosticavel, mas ainda depende de o processo receber `config.ingestion_runtime_changed` em tempo util. Se esse gatilho falhar localmente depois do startup, `consumer` e `emulator` podem ficar stale ate novo evento ou restart.

## Frozen Decisions

- `config.ingestion_runtime_changed` continua sendo o gatilho primario de refresh.
- o fallback do bloco vive no seam compartilhado de bootstrap agregado, nao em polling por scope.
- a configuracao canônica do fallback fica em `bootstrap.reconcile_interval`.
- reconciliacao periodica so pode adotar novo estado quando a assinatura do bootstrap agregado mudar.
- o bloco nao muda contratos NATS/HTTP; ele endurece recuperacao operacional do dataplane.

## Immediate Scope

- `internal/shared/settings/schema.go`
- `internal/actors/scopes/consumer/bootstrap_actor.go`
- `cmd/emulator/run.go`
- docs canônicas e artefato de validacao do bloco

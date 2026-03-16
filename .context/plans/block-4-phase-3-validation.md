---
status: filled
generated: 2026-03-16
updated: 2026-03-16
plan: "block-4-dataplane-multiscope-hardening"
phase: "phase-3"
---

# Block 4 Phase 3 Validation

## Validation Goal

Provar que o dataplane deixou de depender apenas do snapshot de startup e do scope único configurado, sem regredir o workflow do `raccoon-cli` nem o baseline operacional local.

## Implemented Outcome

- `runtimebootstrap` ganhou bootstrap agregado explícito e assinatura estável do conjunto de bindings.
- `consumer` passou a subir pelo bootstrap agregado e a fazer refresh contínuo, trocando a runtime apenas quando a assinatura muda.
- `consumer supervisor` agora ignora mensagens `ready/failed` de gerações antigas, evitando overwrite de estado após reload.
- `emulator` passou a usar o bootstrap agregado no startup e a tentar refresh do snapshot de bindings a cada ciclo de publicação, adotando o novo estado só quando o refresh e o `EnsureTopics` fecham com sucesso.

## Evidence

### Focused Tests

- `go test ./internal/application/runtimebootstrap ./internal/actors/scopes/consumer ./cmd/emulator`

### Static And Fast Guard Rails

- `cargo run --manifest-path tools/raccoon-cli/Cargo.toml -- runtime-bindings`
- `make verify`

### Deep Runtime Proof

- `make check-deep`
- `make scenario-smoke SCENARIO=readiness-probe`
- `make scenario-smoke SCENARIO=happy-path`
- `make scenario-smoke SCENARIO=invalid-payload`

## Result

Todos os checks acima passaram.

Isso fecha a evidência mínima do bloco:

- bootstrap agregado funcionando no dataplane
- refresh contínuo sem regressão de startup
- contrato e roteamento ainda coerentes para o `raccoon-cli`
- smoke e deep gate verdes no cluster real

## Residual Risk

- o refresh contínuo do dataplane nesta fase é polling sobre o seam agregado, não reconciliação dirigida por eventos NATS.
- o baseline `global/default` continua sendo o modo simples de operador local; o bloco removeu o hard requirement desse scope no dataplane, mas não transforma o runtime inteiro em paralelismo irrestrito.

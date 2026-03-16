# Block 1 Readiness And Runtime Seam

> Artefato de `B1-P1-S3` e `B1-P1-S4`: define o contrato de prontidao do `server` e o seam canonico de startup/replay do `validator` sem alterar ainda o codigo do produto.

## B1-P1-S3 - `server` Readiness Contract

### Current Reality

- `healthz` e process-only
- `readyz` hoje testa apenas `configctl.ListConfigs`
- o `server` tambem serve:
  - `GET /runtime/validator/active`
  - `GET /runtime/validator/results`
  - `GET /runtime/ingestion/bindings`
- esses endpoints dependem de tres gateways distintos:
  - `ConfigctlGateway`
  - `ValidatorRuntimeGateway`
  - `ValidatorResultsGateway`

### Canonical Readiness Goal

`readyz` deve responder `200` apenas quando o `server` consegue atender de forma honesta todas as superficies que ele mesmo publica no cluster local padrao.

Isso nao significa exigir dados ativos; significa exigir que os responders e queries criticas estejam acessiveis e coerentes.

### Canonical Checks

| Dependency Surface | Probe | Healthy Condition | Unhealthy Condition |
| --- | --- | --- | --- |
| config lifecycle control | `configctl.ListConfigs({})` | responde sem erro | timeout, unavailable, internal, nats disabled |
| configctl runtime bootstrap surface | `configctl.ListActiveIngestionBindings(global/default)` | responde sem erro, mesmo com `bindings=[]` | timeout, unavailable, invalid response |
| validator results surface | `validatorresults.ListValidationResults(global/default, limit=1)` | responde sem erro, mesmo com `results=[]` | timeout, unavailable, invalid response |
| validator runtime surface | `validatorruntime.GetActiveRuntime(global/default)` | responde com runtime **ou** `NotFound` quando nao existe runtime ativo | timeout, unavailable, invalid response |

### Coherence Rule

- se `ListActiveIngestionBindings(global/default)` retornar bindings ativos, `GetActiveRuntime(global/default)` nao pode continuar em `NotFound`
- esse caso deve derrubar `readyz`, porque indica que `configctl` ja projeta runtime bootstrapavel mas o `validator` ainda nao carregou seu runtime

### Scope Rule

- `readyz` usa `global/default` como scope canonico do cluster local e dos smokes
- verificacoes multi-scope ficam para `scenario-smoke`, troubleshooting e blocos futuros
- `healthz` continua sem tocar dependencias externas

### NATS Disabled Rule

- se `NATS.Enabled=false`, `readyz` deve falhar
- nesse modo o `server` pode ate subir processo HTTP, mas suas superficies reais de controle e runtime ficam indisponiveis
- isso preserva a semantica de `healthz` versus `readyz`: processo vivo nao implica servico operacional

### Why This Contract

- evita falso positivo em compose local
- representa a realidade das superficies publicadas pelo `server`
- nao exige runtime ativo antes de existir uma ativacao
- captura o drift relevante entre `configctl` e `validator` que hoje passaria despercebido

## B1-P1-S4 - Validator Startup/Replay Seam

### Current Reality

- o `validator` sobe `runtime-cache`, `results-store`, consumers e responders
- o cache de runtime so e populado por `configctl.events.config.activated`
- o durable atual (`validator-runtime-cache-v1`) garante consumo continuo de eventos novos, mas nao reidrata o cache a partir do estado atual depois que mensagens antigas ja foram acked
- o cache nao consome `config.deactivated`, entao um scope pode ficar stale se houver desativacao sem nova ativacao imediata
- `consumer` e `emulator` ja fazem bootstrap explicito por query em `/runtime/ingestion/bindings`; o `validator` ainda nao

### Canonical Source Of Truth

- a fonte de verdade do runtime ativo e `configctl`
- o `validator` cache e derivado
- o stream de eventos e o canal de atualizacao continua, nao a unica fonte para reidratacao

### Canonical Seam

O lifecycle do runtime do `validator` deve ser hibrido:

1. **bootstrap snapshot**
   - ao iniciar, o `validator` consulta `configctl` para obter as runtime projections ativas
   - esse bootstrap deve popular o cache antes de o runtime ser considerado coerente

2. **steady-state stream**
   - depois do bootstrap, o `validator` continua consumindo `configctl.events.config.activated`
   - eventos novos continuam sendo o mecanismo normal de update

3. **explicit clear path**
   - desativacoes precisam limpar ou substituir scopes no cache
   - isso pode vir por consumo de `configctl.events.config.deactivated` ou por uma regra equivalente de evict no bootstrap/update path

4. **staleness guard**
   - updates no cache devem ser monotonicamente comparados por `scope` e `activated_at`
   - um evento mais antigo nao pode sobrescrever um runtime bootstrapado mais novo

### Required Bootstrap Input

O bootstrap do `validator` precisa de runtime projection completa por scope:

- scope
- config set/key/version
- artifact
- activated_at
- bindings
- fields
- rules
- definition checksum

### Gap In The Current Surface

As superficies publicas atuais nao expõem isso de forma completa para o bootstrap do `validator`:

- `GET /configctl/configs/active` devolve `ConfigVersionDetail`, mas nao traz `activated_at` por scope
- `GET /runtime/ingestion/bindings` traz runtime resumido e fields por binding, mas nao traz `rules`
- o evento `config.activated` traz `projection` completa, mas o durable nao garante replay total do estado atual depois de acks passados

### Canonical Recommendation

Adicionar uma superficie **interna** de bootstrap para o `validator`, ancorada em `configctl`, em vez de tentar reconstruir o estado apenas por replay do stream.

Forma preferida:

- novo control query interno em `configctl` para listar runtime projections ativas completas por scope
- consumo pelo `validator` via gateway/control plane, nao por endpoint HTTP publico

Forma aceitavel, mas secundaria:

- endpoint ou query dedicada de bootstrap com mesma shape de `RuntimeProjectionRecord`

Forma rejeitada como primaria:

- depender apenas de replay do JetStream
- reconstruir runtime a partir de `GetActiveConfig` + heuristicas locais

### Runtime Cache Rules After The Seam

- `LoadedAt` continua significando quando o cache aplicou a projection
- `ActivatedAt` continua vindo da projection/evento de `configctl`
- cache faz upsert por `scope`
- cache faz evict por desativacao
- `results-store` continua em memoria e fora do bootstrap deste bloco

### Readiness Relationship

- `readyz` nao deve exigir que exista runtime ativo
- `readyz` deve exigir que o path de query do runtime esteja acessivel
- se `configctl` indicar bindings ativos e o `validator` nao tiver runtime correspondente, isso e incoerencia operacional e deve aparecer em `readyz`

## Recommended Execution Impact For Phase 2

- implementar checker composto no `server`
- ajustar testes de `readyz` para diferenciar `NotFound` aceitavel de `Unavailable` nao aceitavel
- adicionar bootstrap explicito do runtime do `validator`
- introduzir clear path para desativacao de scope
- manter results replay fora do escopo do Bloco 1

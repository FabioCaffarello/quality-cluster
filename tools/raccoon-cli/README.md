# raccoon-cli

Engineering quality toolkit for `quality-service`. Fully isolated from the Go runtime — reads files, configs, and source; executes subprocesses only for compose status checks.

## Build & Test

```sh
cd tools/raccoon-cli
cargo build --release
cargo test
```

## Quick Start

```sh
# From the project root:
raccoon-cli doctor                           # project structure check
raccoon-cli quality-gate                     # fast static checks (default)
raccoon-cli quality-gate --profile ci --json # CI pipeline
raccoon-cli quality-gate --profile deep      # full validation (requires running environment)
```

## Commands

### `doctor`

Validates project structure: `go.work`, `internal/`, `deploy/`, `tests/`, `tools/`, compose file, and config files.

```sh
raccoon-cli doctor
raccoon-cli --project-root /path/to/quality-service doctor
raccoon-cli -v doctor   # show all findings, not just failures
```

### `topology-doctor`

Audits the full data pipeline topology by inspecting configs, compose, and Go source.

```sh
raccoon-cli topology-doctor
raccoon-cli --json topology-doctor
raccoon-cli -v topology-doctor   # verbose: show all check details
```

**Checks performed (13):**
| Check | What it validates |
|---|---|
| `configs-dir-exists` | `deploy/configs/` directory exists |
| `config-completeness` | consumer, emulator, validator configs present with required transports |
| `compose-services` | All 7 services defined in docker-compose |
| `compose-dependencies` | Service dependency graph is correct |
| `source-streams` | `DATA_PLANE_INGESTION` and `CONFIGCTL_EVENTS` streams found in source |
| `source-durables` | `validator-dataplane-v1` and `validator-runtime-cache-v1` durables found |
| `source-subjects` | Subject prefixes for dataplane, configctl events, and configctl control present |
| `kafka-broker-consistency` | All services point to the same Kafka brokers; ports match compose |
| `nats-url-consistency` | All services point to the same NATS URL |
| `bootstrap-url-consistency` | All services point to the same bootstrap server URL |
| `stream-subject-alignment` | Stream subject patterns have matching subjects in source |
| `durable-stream-alignment` | Every durable consumer references an existing stream |
| `pipeline-continuity` | Full pipeline emulator->kafka->consumer->jetstream->validator is wired |

### `contract-audit`

Audits messaging contracts and invariants across Kafka, NATS/JetStream, and internal transports.

```sh
raccoon-cli contract-audit
raccoon-cli --json contract-audit
```

**Checks performed (13):**
| Check | What it validates |
|---|---|
| `registry-control-completeness` | Control specs have Subject, RequestType, ReplyType, QueueGroup |
| `registry-event-completeness` | Event specs have Subject and Type |
| `subject-type-convention` | Naming patterns: `.control.` -> `.command.`/`.query.` + `.reply.` |
| `reply-type-symmetry` | Request/reply pairs share operation suffix |
| `queue-group-convention` | Queue group follows `{domain}.{scope}` format |
| `event-stream-coverage` | All event subjects covered by a JetStream stream |
| `consumer-filter-validity` | Consumer filters match their stream's subject patterns |
| `envelope-required-fields` | Envelope validates id, kind, type, timestamp, content_type |
| `codec-consistency` | NATS codec uses CBOR with correct kind checks |
| `dataplane-field-completeness` | DataPlane Message struct validates all critical fields |
| `dataplane-content-type` | DataPlane defaults content_type to application/json |
| `event-metadata-presence` | All domain events have events.Metadata field |
| `event-registry-alignment` | Domain event names match registry event specs |

### `quality-gate`

Canonical validation entrypoint. Orchestrates all quality checks in a disciplined sequence — verify structure, validate topology, audit contracts, then (optionally) prove runtime behavior — with per-step timing, finding-level counts, actionable output, and a single exit code.

This is the command you should use everywhere: local development, CI pipelines, and pre-merge validation. It replaces ad-hoc combinations of individual commands with a single, predictable, profile-driven flow.

```sh
raccoon-cli quality-gate                                    # fast (default)
raccoon-cli quality-gate --profile ci --json                # CI pipeline
raccoon-cli quality-gate --profile deep                     # full (requires infra)
raccoon-cli quality-gate --profile deep --base-url http://localhost:9090
```

**Profiles:**
| Profile | Checks included | Strictness | Use case |
|---------|------------------------------------------------------|-------------------------------|------------------------|
| `fast`  | doctor + topology-doctor + contract-audit + runtime-bindings (default) | warnings pass | Pre-commit, local dev  |
| `ci`    | doctor + topology-doctor + contract-audit + runtime-bindings | warnings promoted to errors | CI pipeline            |
| `deep`  | doctor + topology-doctor + contract-audit + runtime-bindings + runtime-smoke | warnings pass | Pre-merge / nightly    |

**Options:**
| Flag | Default | Description |
|------|---------|-------------|
| `--profile` | `fast` | Execution profile: `fast`, `ci`, or `deep` |
| `--base-url` | `http://127.0.0.1:8080` | Base URL for runtime-smoke (deep profile only) |
| `--fail-fast` | off | Stop after the first failing step (skip remaining steps) |

**Sample output:**
```
=== quality-gate [profile: fast] ===

  [+] doctor PASS (2ms) — 8 checks
  [+] topology-doctor PASS (45ms) — 13 checks
  [+] contract-audit PASS (12ms) — 13 checks
  [+] runtime-bindings PASS (8ms) — 11 checks
  [-] arch-guard SKIP (0ms) — not yet implemented
  [-] runtime-smoke SKIP (0ms) — skipped in 'fast' profile — use --profile deep

Result: PASSED | 4 passed, 0 failed, 2 skipped | 45 checks | 67ms
```

On failure, each failed step shows error/warning counts and per-step remediation hints:
```
  [x] topology-doctor FAIL (25ms) — 13 checks, 2 errors, 1 warning
      [config-completeness] [error] consumer-nats: consumer config has no nats url

Actionable next steps:
  - Fix 'doctor': run `raccoon-cli doctor` — check go.work, dirs, compose, and config files
  - Fix 'topology-doctor': run `raccoon-cli topology-doctor` — check configs, compose, and source wiring
```

Execution errors (IO failures, parse errors) are distinguished from check failures:
```
  [x] topology-doctor FAIL (1ms) — execution error
```

Use `-v` (verbose) to see all check details even for passing steps:
```sh
raccoon-cli -v quality-gate
```

With `--fail-fast`, the gate stops at the first failure:
```
=== quality-gate [profile: fast] ===

  [x] doctor FAIL (1ms) — 8 checks, 3 errors
  [-] topology-doctor SKIP (0ms) — skipped — prior step 'doctor' failed (fail-fast mode)
  [-] contract-audit SKIP (0ms) — skipped — prior step 'doctor' failed (fail-fast mode)
  ...
```

**Pipeline architecture:**
```
quality-gate
├── doctor            (static — project structure: go.work, dirs, compose, configs)
├── topology-doctor   (static — config/compose/source consistency)
├── contract-audit    (static — messaging contracts and invariants)
├── runtime-bindings  (static — config → kafka → jetstream → validator routing)
├── arch-guard        (reserved — dependency and layer boundary checks)
└── runtime-smoke     (--profile deep only — e2e against live environment)
```

### `arch-guard`

Guards architectural boundaries using **AST-based semantic analysis**. Goes beyond import-path checking to inspect type definitions, struct fields, interface method signatures, and exported function parameters via the `codeintel` structural index.

```sh
raccoon-cli arch-guard
raccoon-cli --json arch-guard
raccoon-cli -v arch-guard
```

**Rules enforced (11):**

*Import-based rules (1–4):*
| Check | Rule | Severity |
|---|---|---|
| `layer-dependency-direction` | Layers can only depend inward: domain (0) <- application (1) <- adapters (2) <- actors (3) <- interfaces (4). Same-layer and `shared/` imports are always allowed. | error |
| `domain-purity` | `domain/` must not import infrastructure packages (nats, kafka, hollywood, net/http, database/sql) | error |
| `application-isolation` | `application/` must not import `adapters/`, `actors/`, or `interfaces/` — use ports instead | error |
| `interfaces-isolation` | `interfaces/` (HTTP handlers) must not import `adapters/` or `actors/` | error |

*Structural/boundary rules (5–8):*
| Check | Rule | Severity |
|---|---|---|
| `cmd-boundary` | `cmd/` should not import `domain/` directly (warning) or define >5 types via AST counting (warning) | warning |
| `tooling-boundary` | `tools/` must not contain `go.mod` files or reference Go internals as Rust modules | error |
| `no-cross-cmd` | One `cmd/` binary must not import another's package — binaries are independently deployable | error |
| `deploy-boundary` | Go source in `internal/` must not hardcode `deploy/` paths (comments excluded) | warning |

*Semantic rules (9–11 — AST-based, new):*
| Check | Rule | Severity |
|---|---|---|
| `port-contract-leaks` | Port interfaces in `application/ports/` must not reference infrastructure types (nats, kafka, http.Client, sql.DB, etc.) or adapter-qualified types in method signatures | error |
| `domain-type-contamination` | Struct fields in `domain/` must not use infrastructure type expressions (e.g., `Conn *nats.Conn`) | error |
| `exported-signature-leaks` | Exported functions in `domain/` and `application/` must not accept or return infrastructure types in their parameters/returns | warning |

**Allowed dependency matrix:**
```
                domain  application  adapters  actors  interfaces  shared
domain             ✓         ✗          ✗        ✗        ✗          ✓
application        ✓         ✓          ✗        ✗        ✗          ✓
adapters           ✓         ✓          ✓        ✗        ✗          ✓
actors             ✓         ✓          ✓        ✓        ✗          ✓
interfaces         ✓         ✓          ✗        ✗        ✓          ✓
```

**Semantic analysis details:**

The semantic rules (9–11) use the `codeintel` structural index to parse Go source into AST-level facts (types, fields, method signatures, function params/returns) and detect violations that import-only analysis misses:

- A port interface returning `*nats.Conn` passes import checks (the import is in `adapters/`) but fails `port-contract-leaks` because the infrastructure type leaks into the application contract
- A domain struct with field `Reader *kafka.Reader` passes import checks but fails `domain-type-contamination` because the domain is coupled to a concrete adapter
- An exported `NewService(conn *nats.Conn)` in `application/` passes import checks but fails `exported-signature-leaks` because callers are forced to depend on the concrete adapter

### `impact-map`

Maps the structural impact of changed files, packages, or symbols. Uses the `codeintel` AST index to trace import relationships, exported symbols, and contract surface. Differentiates observed facts from inferred risks.

With `--lsp`, enriches the analysis with gopls semantic references — revealing actual call sites in function bodies that import-level analysis cannot see.

```sh
raccoon-cli impact-map internal/domain/configctl/config.go   # single file
raccoon-cli impact-map internal/adapters/nats/               # whole package
raccoon-cli impact-map ConfigSet                             # symbol name
raccoon-cli impact-map                                       # auto-detect from git status
raccoon-cli impact-map --lsp internal/domain/configctl       # enrich with gopls references
raccoon-cli --json impact-map --lsp ConfigSet                # JSON output with LSP
raccoon-cli -v impact-map internal/domain/configctl          # verbose: show all files
```

**Target resolution (in order):**
1. **File** — exact match against indexed Go files (normalizes `./` prefix)
2. **Package** — directory match against indexed packages (normalizes trailing `/`)
3. **Symbol** — type or function name match across all packages
4. **Unresolved** — still reports sensitive area matches for known project paths

**What the output includes:**

| Section | Content | Source |
|---------|---------|--------|
| Exported symbols | Types, functions, constants, methods with locations | `[ast]` |
| Contract surface | Interfaces, message types (`*Command`, `*Event`, etc.), port interfaces | `[ast]` |
| Direct dependents | Packages that import the target package | `[ast]` |
| Sensitive areas | Matched area name + description | `[ast]` |
| Risks | Fan-out, contract breakage, large API surface | `[ast]` labeled |
| Semantic references | Cross-package call sites (with `--lsp`) | `[lsp]` |
| Recommended commands | `raccoon-cli` checks to run | Derived |

**LSP enrichment (`--lsp`):** Queries gopls for references to exported symbols, revealing function body call sites not visible to import-level analysis. Falls back cleanly if gopls is unavailable — AST results are always present.

**What it does NOT do without `--lsp`:**
- No call graph (function bodies are not analyzed)
- No type resolution across packages
- No runtime/reflection tracing

All limitations are stated in the output via the `scope_note` field.

**Example output:**
```
=== Impact Map ===

Target: internal/domain/configctl [package]
  Package: internal/domain/configctl
  Exported symbols (7):
    ConfigSet [struct] at internal/domain/configctl/config.go:5
    NewConfigSet [func] at internal/domain/configctl/config.go:15
    VersionLifecycle [type_alias] at internal/domain/configctl/lifecycle.go:3
    ...
  Direct dependents (3):
    internal/application/configctl (observed: direct import in AST)
    internal/adapters/nats (observed: direct import in AST)
    internal/actors/scopes/configctl (observed: direct import in AST)
  Sensitive areas:
    domain — domain layer — business rules (must be pure)
  Risks:
    [observed] 3 direct dependent package(s) — verify they still compile and behave correctly

Recommended checks:
  $ raccoon-cli arch-guard
  $ raccoon-cli contract-audit
  $ raccoon-cli quality-gate

Scope: Impact is computed from static import graphs and exported symbol analysis.
  No call graph, type resolution, or runtime tracing is available.
```

### `symbol-trace`

Traces a symbol (type, function, constant, variable) across the Go codebase. Uses the `codeintel` AST index to find definitions, structural references, package relationships, and contract connections.

With `--lsp`, enriches results with gopls semantic analysis — type-resolved definitions, cross-package references (including function body call sites), and hover/type signatures.

```sh
raccoon-cli symbol-trace ConfigSet                   # trace a struct (AST only)
raccoon-cli symbol-trace --lsp ConfigSet             # enrich with gopls
raccoon-cli symbol-trace VersionLifecycle            # trace a type alias + its constants
raccoon-cli symbol-trace ConfigctlGateway            # trace a port interface
raccoon-cli symbol-trace NewConfigSet                # trace a function
raccoon-cli --json symbol-trace --lsp CreateDraftCommand   # JSON output with LSP
raccoon-cli -v symbol-trace --lsp ConfigSet          # verbose: show all references
```

**Resolution statuses:**
- **resolved** — single definition found
- **ambiguous** — multiple definitions across packages (labeled, user disambiguates)
- **not_found** — no definition found (helpful diagnostics provided)

**What the output includes:**

| Section | Content | Source |
|---------|---------|--------|
| Definitions | Name, kind, package, file, line, visibility, details | `[ast]` |
| Structural references | Struct field types, function params/returns, receivers, interface embeds, alias underlyings | `[ast]` |
| Additional definitions | Type-resolved definitions from gopls (with `--lsp`) | `[lsp]` |
| Semantic references | Cross-package call sites from gopls (with `--lsp`) | `[lsp]` |
| Type signature | Hover/type info from gopls (with `--lsp`) | `[lsp]` |
| Packages involved | All packages where the symbol is defined or referenced | Both |
| Contract connections | Port interfaces, message types, contract-layer types | `[ast]` |
| Recommended commands | `raccoon-cli` checks relevant to the symbol's location | Derived |

**LSP enrichment (`--lsp`):** When enabled, starts gopls in the background, queries definition/references/hover at the symbol's AST location, and merges results. Falls back cleanly if gopls is unavailable — AST results are always present, and the output clearly shows `LSP: unavailable (reason)`.

**Fallback behavior:**
- gopls not installed → AST-only results + `LSP: unavailable (gopls not found on PATH)`
- workspace not indexed → AST-only + `LSP: unavailable (reason)`
- gopls returns no results → AST-only + `LSP: connected but no additional results`
- Symbol ambiguous → all definitions shown, LSP queries first AST location

**Example output (with `--lsp`):**
```
=== Symbol Trace: ConfigSet ===

LSP: enriched (gopls connected)

Status: resolved (single definition)

Definitions (1): [ast]
  ConfigSet [struct] (exported) at internal/domain/configctl/config.go:5
    package: configctl
    field: SetID string
    field: Versions []ConfigVersion

Structural references (4): [ast]
  receiver in AddVersion at internal/domain/configctl/config.go:18
  receiver in VersionCount at internal/domain/configctl/config.go:21
  return type in NewConfigSet at internal/domain/configctl/config.go:14
  return type in CreateDraft at internal/application/configctl/create_draft.go:7

Semantic references (3): [lsp]
  internal/application/configctl/create_draft.go:7
  internal/adapters/nats/codec.go:7
  internal/actors/scopes/configctl/supervisor.go:9

Type signature: type ConfigSet struct { ... } [lsp]

Packages involved (4):
  configctl
  nats

Contract connections: none

Recommended checks:
  $ raccoon-cli arch-guard
  $ raccoon-cli impact-map <symbol>

Scope: Trace combines structural AST indexing (declarations, signatures,
  struct fields) with gopls semantic analysis (type-resolved definitions,
  cross-package references including call sites). Each fact is tagged with
  its source: [ast] or [lsp].
```

### `lsp-enrich`

Enriches a symbol with semantic information from `gopls` via the LSP bridge. Combines deterministic AST facts from `codeintel` with type-resolved definitions, cross-package references, and hover/type information.

```sh
raccoon-cli lsp-enrich ConfigSet                    # enrich with gopls (if available)
raccoon-cli lsp-enrich --no-lsp ConfigSet           # AST-only, skip gopls
raccoon-cli --json lsp-enrich VersionLifecycle      # JSON output
raccoon-cli lsp-enrich --timeout 10 ConfigSet       # custom timeout (seconds)
```

**What it returns:**

| Section | Content | Source |
|---------|---------|--------|
| AST definitions | Type, function, constant, variable definitions with locations | `codeintel` (AST) |
| LSP definitions | Type-resolved definitions from gopls | `gopls` (LSP) |
| LSP references | Cross-package references from gopls | `gopls` (LSP) |
| Hover info | Resolved type signature and documentation | `gopls` (LSP) |

Every fact is tagged with its provenance: `ast`, `lsp`, or `unavailable`.

**Graceful degradation:** If `gopls` is not installed, the workspace is invalid, or a query times out, the command returns AST-only results with a clear `lsp_status: unavailable` indication and the reason. The CLI never fails because of LSP.

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--no-lsp` | off | Skip gopls entirely, return AST-only results |
| `--timeout` | `30` | Timeout in seconds for gopls requests |

### `rename-safety`

Evaluates the structural and semantic risk of renaming a Go symbol before performing the rename. Does **not** execute the rename — assessment only.

```sh
raccoon-cli rename-safety ConfigSet                           # assess rename risk
raccoon-cli rename-safety ConfigSet --to QualityConfigSet     # also check for naming conflicts
raccoon-cli rename-safety --lsp ConfigctlGateway              # enrich with gopls references
raccoon-cli --json rename-safety CreateDraftCommand            # JSON output
raccoon-cli -v rename-safety --lsp --to NewName OldName       # verbose + LSP + conflict check
```

**What it evaluates:**

| Section | Content | Provenance |
|---------|---------|------------|
| Definitions | Where the symbol is defined (type, file, line, visibility) | observed (AST) |
| Affected references | Struct fields, params, returns, receivers, embeds, aliases, const/var types | observed (AST) |
| LSP references | Function body call sites, cross-package usages (with `--lsp`) | lsp (gopls) |
| Sensitive areas | Which architectural layers are touched (domain, ports, contracts, adapters, actors, http) | observed |
| Contract surface | Port interfaces, message types, contract-layer definitions | observed |
| Name conflicts | Existing symbols with the proposed new name (with `--to`) | observed |
| Risk assessment | Overall risk level (low/medium/high/critical) with reasons | observed + inferred |
| Recommendations | Quality-gate profile, smoke scenarios, raccoon-cli checks to run | inferred |

**Risk levels:**

| Level | Meaning | Gate profile |
|-------|---------|--------------|
| `low` | Unexported, few references, no contracts | `fast` |
| `medium` | Exported symbol or moderate references | `ci` |
| `high` | Cross-layer impact, many references, ambiguous definitions | `deep` |
| `critical` | Port interfaces or message types — serialization/contract boundary | `deep` |

**Example output:**
```
=== Rename Safety Check: ConfigSet ===

Status: resolved (single definition)

Risk level: HIGH
  [observed] exported_symbol: symbol is exported — external packages may depend on it
  [observed] high_reference_count: 41 references found — broad blast radius
  [inferred] cross_layer_impact: rename spans domain and adapter layers — high coupling risk

Definitions (1): [observed]
  ConfigSet [struct] (exported) at internal/domain/configctl/config_set.go:26
    package: configctl

Affected references (41 total, 41 structural): [observed]
  param_type in SaveConfigSet at internal/adapters/repositories/memory/configctl/repository.go:40
  return_type in GetConfigSetByID at internal/adapters/repositories/memory/configctl/repository.go:94
  ...

Sensitive areas touched (3): [observed]
  adapters (3 files)
  application (5 files)
  domain (1 files)

--- Recommendations ---

Quality gate profile: deep
Suggested smoke scenarios:
  $ raccoon-cli scenario-smoke config-lifecycle
  $ raccoon-cli scenario-smoke happy-path
Recommended checks:
  $ raccoon-cli arch-guard
  $ raccoon-cli drift-detect
  $ raccoon-cli runtime-bindings
```

### `runtime-bindings`

Inspects the runtime binding chain: config declaration → Kafka topics → JetStream subjects → validator routing. Detects drift between declared configuration, active routing constants, and expected runtime topology.

```sh
raccoon-cli runtime-bindings
raccoon-cli --json runtime-bindings
raccoon-cli -v runtime-bindings    # show all findings including info
```

**Checks performed (11):**
| Check | What it validates |
|---|---|
| `subject-pattern` | Dataplane ingestion subject prefix found in source |
| `routing-constants` | DATA_PLANE_INGESTION stream and validator durable consumers exist |
| `lifecycle-events` | Activation/deactivation/runtime-changed events declared in domain |
| `config-bindings` | Binding definitions: uniqueness, field/rule presence |
| `fixture-bindings` | HTTP test fixtures contain example binding payloads |
| `resolved-bindings` | Config bindings cross-referenced with source routing constants |
| `topic-subject-mapping` | Derived JetStream subjects fall within stream subscription scope |
| `consumer-coverage` | Bootstrap client and topology builder exist in source |
| `validator-coverage` | Runtime cache and validation worker exist in source |
| `scope-consistency` | All bindings use consistent activation scopes |
| `drift-detection` | Subject prefix, durable stream targets, and bootstrap scopes aligned |

**What it detects:**
- Missing or drifted subject prefix (consumer/validator disagree on subjects)
- Durable consumers bound to wrong JetStream streams
- Bindings whose derived subjects fall outside stream subscription patterns
- Missing bootstrap client, topology builder, runtime cache, or validation worker
- Multiple activation scopes without explicit bootstrap coverage
- Lifecycle events missing from the domain event registry

### `drift-detect`

Detects drift between what the system declares, what it configures, what the source wires, and what the documentation says. Returns exit code 1 if any error-severity drift is found.

```sh
raccoon-cli drift-detect
raccoon-cli --json drift-detect
raccoon-cli -v drift-detect    # show all findings including warnings
```

**Drift classes detected (6):**
| Check | What it detects |
|---|---|
| `config-compose-drift` | Services in configs but not compose (or vice versa); transport config without matching compose dependency |
| `config-source-drift` | Stream/durable/subject constants in source vs expectations; orphan stream subjects |
| `binding-topology-drift` | Declared bindings vs routing infrastructure (DATA_PLANE_INGESTION stream, validator durable) |
| `workflow-drift` | DEVELOPMENT.md `make` targets not in Makefile; unknown `raccoon-cli` subcommands in docs; undocumented workflow targets |
| `contract-domain-drift` | Domain events without matching registry specs; registry specs without matching domain events |
| `compose-profile-drift` | Missing expected compose profiles (core, runtime, dataplane, all); unassigned services |

**What it catches:**
- Config declares Kafka brokers but compose service doesn't `depends_on: kafka` — service will crash on startup
- DEVELOPMENT.md says `make deploy-staging` but no such Makefile target exists — developers hit "No rule" errors
- Domain defines `config.rejected` event but no adapter registry spec wires it to a transport — event is silently lost
- Bindings reference topics but `DATA_PLANE_INGESTION` stream was removed from source — data pipeline is broken
- Compose profiles don't cover the `runtime` layer — `make up-runtime` starts nothing

**Example output:**
```
=== drift-detect ===

--- config-compose-drift: PASS ---
--- config-source-drift: PASS ---
--- binding-topology-drift: PASS ---
--- workflow-drift: FAIL ---
  [error] doc-target-drift: DEVELOPMENT.md references `make deploy-staging` but target not found in Makefile
--- contract-domain-drift: PASS ---
--- compose-profile-drift: PASS ---
Result: FAILED | 5 passed, 1 failed, 0 skipped

> Stop — 1 error must be fixed before proceeding.
```

### `runtime-smoke`

End-to-end smoke test against a live local environment. Requires `make up-dataplane` running.

```sh
make up-dataplane
raccoon-cli runtime-smoke
raccoon-cli runtime-smoke --base-url http://localhost:9090
raccoon-cli --json runtime-smoke
```

**Stages:**
| Stage | What it does |
|---|---|
| `bootstrap` | Verifies all 7 compose services are running |
| `readiness` | Polls `/healthz` + `/readyz` until 200 (timeout: 60s) |
| `inject` | Creates a smoke config (draft -> validate -> compile -> activate) |
| `route` | Confirms ingestion bindings are projected via `/runtime/ingestion/bindings` |
| `consume` | Waits for validation results (proves Kafka -> consumer -> JetStream -> validator pipeline) |
| `validate` | Checks results contain both `passed` and `failed` entries from emulator samples |

Stages execute sequentially; if any stage fails, remaining stages are skipped.

### `scenario-smoke`

Run named, reproducible validation scenarios against a live local environment. Each scenario declares preconditions, executes a deterministic sequence of checks, and reports structured pass/fail results.

```sh
raccoon-cli scenario-smoke --list                          # list all scenarios
raccoon-cli scenario-smoke happy-path                      # full E2E
raccoon-cli scenario-smoke config-lifecycle                # control plane only
raccoon-cli --json scenario-smoke invalid-payload          # JSON output
raccoon-cli scenario-smoke readiness-probe --base-url http://localhost:9090
```

**Scenarios:**
| Scenario | What it validates | Requires |
|---|---|---|
| `happy-path` | Full E2E: config lifecycle + data plane + validation results (passed + failed) | `make up-dataplane` |
| `config-lifecycle` | Control plane only: draft -> validate -> compile -> activate -> verify active config | Core services (nats, configctl, server) |
| `invalid-payload` | Activate config and verify validator catches invalid payloads with structured violations | `make up-dataplane` |
| `missing-binding` | Query non-existent scope/binding and verify empty results (no errors) | Server responding |
| `readiness-probe` | Quick cluster health check: bootstrap + readiness (healthz + readyz) | Compose services |

**Options:**
| Flag | Default | Description |
|------|---------|-------------|
| `--list` | off | List all available scenarios and exit |
| `--base-url` | `http://127.0.0.1:8080` | Quality-service HTTP API base URL |

**Why scenario-smoke over runtime-smoke:** `runtime-smoke` runs a single fixed pipeline. `scenario-smoke` provides named, composable scenarios that target specific cluster behaviors — you can run just the control plane test, just the readiness check, or specifically validate that the validator catches bad data.

### `trace-pack`

Collects diagnostic evidence from the running cluster into a self-contained trace pack. Produces a timestamped directory (or `.tar.gz`) with compose status, API responses, deploy configs, and recent service logs — everything needed to diagnose a failure without live cluster access.

```sh
raccoon-cli trace-pack                              # collect to current directory
raccoon-cli trace-pack --compress                   # output as .tar.gz
raccoon-cli trace-pack --output-dir /tmp/traces     # custom output location
raccoon-cli trace-pack --log-lines 500              # more log context
raccoon-cli --json trace-pack                       # JSON manifest output
```

**Collected evidence:**
| File | What it contains |
|------|-----------------|
| `compose-status.txt` | `docker compose ps --all` output — service health matrix |
| `healthz.json` | `/healthz` response — liveness state |
| `readyz.json` | `/readyz` response — readiness state |
| `active-config.json` | `/configctl/configs/active` — running quality config |
| `ingestion-bindings.json` | `/runtime/ingestion/bindings` — active data routing |
| `validator-runtime.json` | `/runtime/validator/active` — validator runtime state |
| `validation-results.json` | `/runtime/validator/results` — recent pass/fail outcomes |
| `configs/*.jsonc` | Deploy config files (server, consumer, validator, emulator, configctl) |
| `logs/*.log` | Recent container logs per service (default: last 200 lines) |
| `SUMMARY.md` | Human-readable manifest: what was collected, what failed, and how to use it |

Each evidence source is collected independently — failures in one do not block others. Unavailable items (service down, endpoint unreachable) are recorded in the summary rather than causing the command to fail.

**Pack structure:**
```
trace-pack-20260314-153042/
├── SUMMARY.md
├── compose-status.txt
├── healthz.json
├── readyz.json
├── active-config.json
├── ingestion-bindings.json
├── validator-runtime.json
├── validation-results.json
├── configs/
│   ├── server.jsonc
│   ├── consumer.jsonc
│   ├── validator.jsonc
│   ├── emulator.jsonc
│   └── configctl.jsonc
└── logs/
    ├── nats.log
    ├── kafka.log
    ├── configctl.log
    ├── server.log
    ├── consumer.log
    ├── validator.log
    └── emulator.log
```

**Options:**
| Flag | Default | Description |
|------|---------|-------------|
| `--base-url` | `http://127.0.0.1:8080` | Quality-service HTTP API base URL |
| `--output-dir` | `.` | Directory where the pack is created |
| `--log-lines` | `200` | Number of recent log lines per service |
| `--results-limit` | `20` | Maximum validation results to collect |
| `--compress` | off | Compress output as `.tar.gz` |

## Global Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON instead of human-readable text |
| `-v`, `--verbose` | Show detailed findings for all checks, not just failures |
| `--project-root <PATH>` | Path to the project root (default: `.`) |
| `--version` | Show version |
| `--help` | Show help |

All global flags must appear **before** the subcommand name:
```sh
raccoon-cli --json --project-root /path quality-gate --profile ci
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All checks passed |
| `1` | At least one check failed |
| `2` | Runtime error (IO, parsing, rendering) |

These codes are consistent across all commands and designed for direct use in CI `if` conditions or `set -e` scripts.

## JSON Schema (quality-gate)

```json
{
  "profile": "ci",
  "steps": [
    {
      "name": "doctor",
      "status": "pass",
      "duration_ms": 2,
      "check_count": 8,
      "error_count": 0,
      "warning_count": 0,
      "report": { "title": "...", "checks": ["..."], "passed": true }
    },
    {
      "name": "topology-doctor",
      "status": "fail",
      "duration_ms": 45,
      "check_count": 13,
      "error_count": 2,
      "warning_count": 1,
      "report": { "title": "...", "checks": ["..."], "passed": false }
    },
    {
      "name": "runtime-smoke",
      "status": "skip",
      "duration_ms": 0,
      "check_count": 0,
      "error_count": 0,
      "warning_count": 0,
      "skip_reason": "skipped in 'ci' profile — use --profile deep to include",
      "report": { "..." }
    }
  ],
  "summary": {
    "passed": 2,
    "failed": 1,
    "skipped": 3,
    "total_checks": 34,
    "total_errors": 2,
    "total_warnings": 1
  },
  "verdict": {
    "action": "stop",
    "message": "Stop — 2 errors must be fixed before proceeding.",
    "next_steps": [
      "Fix 'topology-doctor': run `raccoon-cli topology-doctor` — check configs, compose, and source wiring"
    ]
  },
  "total_duration_ms": 57,
  "passed": false
}
```

Key fields:
- `check_count`: number of individual checks within the step
- `error_count`, `warning_count`: finding-level counts per step (consistent between human and JSON output)
- `skip_reason`: present only for skipped steps, explains why
- `is_execution_error`: present only when `true` — distinguishes IO/parse failures from check-level findings
- `summary`: pre-computed step counts, total checks, and total errors/warnings (no client-side calculation needed)
- `verdict`: structured verdict for machine consumers — `action` is `"proceed"` or `"stop"`, `message` is human-readable, `next_steps` lists remediation hints (empty on proceed)

## Workflow Integration

For the complete developer workflow — when to run which command, how to troubleshoot, CI integration, and compose profiles — see **[`DEVELOPMENT.md`](../../DEVELOPMENT.md)** at the project root.

Quick summary of Makefile targets:

```sh
make check          # pre-code guard rail (quality-gate fast)
make verify         # post-change: Go tests + quality-gate
make check-deep     # full validation (requires make up-dataplane)
make smoke          # e2e smoke test
make trace-pack     # collect diagnostic evidence
make results-inspect # inspect validator results
```

## Architecture

### Layered Design

The CLI follows a strict layered architecture with unidirectional dependencies:

```
┌──────────────────────────────────────────────────────────┐
│  main.rs — CLI parsing (clap), subcommand dispatch       │
│            Depends on: all layers below                   │
└──────────────┬───────────────────────────────────────────┘
               │
┌──────────────▼───────────────────────────────────────────┐
│  gate/ — Quality-gate orchestration                      │
│          Profiles, step sequencing, timing, rendering    │
│          Depends on: analyzers, smoke, models, output    │
└──────────────┬───────────────────────────────────────────┘
               │
┌──────────────▼──────────────┬────────────────────────────┐
│  analyzers/                 │  smoke/                     │
│  ├── doctor.rs              │  ├── mod.rs (orchestration) │
│  ├── topology.rs            │  ├── api.rs (HTTP client)   │
│  │   └── configs, compose,  │  ├── compose.rs (detection) │
│  │       source (scanners)  │  └── stages.rs (6 stages)   │
│  ├── runtime_bindings.rs    │                             │
│  │   └── configs, source    │                             │
│  └── contracts.rs           │                             │
│      └── registry, envelope,│  Runtime checks             │
│          dataplane, events  │  (requires live environment) │
│      (scanners)             │                             │
│  Static analysis            │                             │
└──────────────┬──────────────┴──────────────┬─────────────┘
               │                             │
┌──────────────▼─────────────────────────────▼─────────────┐
│  output/ — Rendering (human, human-verbose, JSON)        │
│  models/ — Canonical data: Finding, CheckResult, Report  │
│  error/  — Unified error type (CliError)                 │
│                                                          │
│  Foundation layer — no upward dependencies               │
└──────────────────────────────────────────────────────────┘
```

### Module Responsibilities

| Layer | Module | Responsibility |
|-------|--------|----------------|
| **CLI** | `main.rs` | Argument parsing, format selection, exit code mapping. Zero analysis logic. |
| **Orchestration** | `gate/` | Profiles, step sequencing, timing, warning promotion, gate-level rendering. |
| **Analysis** | `analyzers/doctor` | Project structure validation (go.work, dirs, compose, configs). |
| **Analysis** | `analyzers/topology` | Pipeline topology: configs, compose, source scanning, cross-validation. |
| **Analysis** | `analyzers/contracts` | Messaging contracts: registries, envelope, codec, dataplane, events. |
| **Analysis** | `analyzers/runtime_bindings` | Runtime binding chain: config → kafka → jetstream → validator routing, drift detection. |
| **Analysis** | `analyzers/arch_guard` | Architectural boundary enforcement: layer deps, domain purity, cmd/tooling/deploy isolation. |
| **Runtime** | `smoke/` | E2E smoke test: compose detection, HTTP probes, stage sequencing, named scenarios. |
| **Diagnostics** | `trace_pack/` | Evidence collection: compose status, API snapshots, configs, logs. |
| **Presentation** | `output/` | Rendering logic for `Report` (human, verbose, JSON). |
| **Models** | `models/` | Canonical types: `Severity`, `Finding`, `CheckStatus`, `CheckResult`, `Report`. |
| **Errors** | `error/` | `CliError` enum with `From` impls for IO and JSON errors. |

### Canonical Data Model

All analyzers produce the same canonical types:

```
Report (title, checks[], passed)
└── CheckResult (name, status, findings[])
    └── Finding (severity, check, message, location?)
        └── Severity: Info | Warning | Error
```

- **`Finding`** — a single observation with severity, source check name, message, and optional fields:
  - `location` — file path where the issue was found
  - `why` — why this finding matters (the consequence if ignored)
  - `help` — recommended next step to fix the issue
- **`CheckResult`** — a named check with pass/fail/skip status, derived from its findings.
- **`Report`** — a titled collection of check results with aggregate pass/fail.

Every analyzer function follows the same contract: `fn analyze(&Path) -> Result<Report>`. The gate module wraps reports in `StepResult` (adding timing and profile-aware strictness) and aggregates them into `GateReport`.

### Analyzer Pattern

Each analyzer under `analyzers/` follows a consistent structure:

1. **Scan phase** — read and parse relevant files (configs, Go source, compose)
2. **Index phase** — build an in-memory index of discovered artifacts
3. **Check phase** — run validation checks against the index, producing `CheckResult`s
4. **Report** — collect all check results into a `Report`

Scanners are private submodules (e.g., `contracts/registry.rs`, `topology/configs.rs`) that return structured data. Checks are pure functions from index to `CheckResult`. This separation keeps scanning logic testable independently from validation logic.

### Design Decisions

- **No YAML/TOML parser deps** — compose and JSONC are parsed with minimal line-based parsers.
- **No runtime coupling** — the CLI never imports Go code. It reads files and scans source.
- **Three-phase analysis** — configs -> compose -> source -> cross-validate.
- **Exit codes** — 0 = all checks pass, 1 = at least one check failed, 2 = runtime error.
- **Minimal dependencies** — clap, serde, serde_json, ureq (HTTP client for runtime-smoke).
- **Verbose by default in standalone, compact in gate** — `quality-gate` hides passing details unless `-v` is used; standalone commands show failures by default and all details with `-v`.
- **Rendering separated from models** — `Report` defines data structure and `Display`; the `output/` module owns the configurable rendering (human/verbose/JSON).
- **Analysis separated from CLI** — all analyzer logic lives under `analyzers/`, not in `main.rs`. The CLI layer only handles parsing, dispatch, and exit codes.

### Validated Scenarios (Test Matrix)

560+ tests across unit, integration, and behavioral validation:

| Category                     | Tests | Scope |
|------------------------------|-------|-------|
| CLI argument parsing         |  29   | Unit: clap derive parsing, flag combinations, --fail-fast |
| Models (Severity, Finding, CheckResult, Report) | 32 | Unit: construction, status derivation, JSON serialization, why/help fields |
| Output rendering (human/verbose/JSON) | 15 | Unit: format selection, verdict, finding visibility, guard rail verdict |
| Error module (CliError)      |   6   | Unit: Display, From impls, trait conformance |
| Gate orchestration & profiles | 66   | Unit: promotion, step timing, skip reasons, finding counts, execution errors, guard rail verdict, fail-fast, runtime-bindings step, verdict struct |
| doctor analyzer              |  10   | Unit: file/dir checks, actionable messages, why/help coverage |
| topology-doctor analyzer     |  22   | Unit: config/compose/source checks, cross-validation |
| contract-audit analyzer      |  21   | Unit: subject convention, stream coverage, pattern matching |
| Topology parsers (JSONC, compose, source) | 22 | Unit: comment stripping, edge cases, malformed input |
| Contract scanners (registry, envelope, codec, dataplane, events) | 21 | Unit: Go source extraction |
| Smoke test stages            |   6   | Unit: graceful failure for each stage |
| Smoke orchestration          |   7   | Unit: config defaults, stage skipping, blocker reference |
| Scenario-smoke scenarios     |  15   | Unit: parsing, selection, execution, serialization, skip behavior |
| CLI integration (subprocess) |  28   | E2E: exit codes, JSON schema, help text, scenario-smoke |
| Validation matrix (subprocess) | 97  | E2E: determinism, profiles, stderr, fixtures, guard rail verdict, why/help, fail-fast, verdict JSON |
| API client                   |   2   | Unit: URL normalization, correlation ID |
| Compose service detection    |   3   | Unit: missing service identification |
| Trace-pack collector & rendering | 20 | Unit: evidence collection, summary generation, compression, graceful degradation |

Key contracts proven by tests:
- **Exit codes**: 0=pass, 1=fail, 2=runtime error — no exceptions
- **JSON schema**: stable field names, lowercase enums, `skip_reason` only when skipped
- **Determinism**: identical inputs → identical outputs (both human and JSON)
- **Graceful degradation**: nonexistent root, empty project, malformed JSONC — no panics
- **stderr isolation**: check failures go to stdout only
- **CI promotion**: warnings→errors, info untouched, errors untouched
- **Stage skipping**: blocker referenced in skip message
- **Guard rail verdict**: "Safe to proceed" on pass, "Stop — N errors must be fixed" on fail
- **Actionable findings**: error findings include `why` (consequence) and `help` (fix) fields in both human and JSON output

### Role in the Repository

`raccoon-cli` is the engineering quality platform for `quality-service`. It enforces discipline at four levels:

1. **Structure** (`doctor`) — validates the project layout so other tools can rely on conventions.
2. **Topology** (`topology-doctor`) — statically verifies that configs, compose, and Go source are wired consistently across the data pipeline.
3. **Contracts** (`contract-audit`) — audits messaging contracts, envelope invariants, and registry completeness.
4. **Bindings** (`runtime-bindings`) — validates the config → kafka → jetstream → validator routing chain and detects drift.
5. **Architecture** (`arch-guard`) — enforces clean architecture layer boundaries, domain purity, and runtime/tooling isolation.
6. **Behavior** (`runtime-smoke`) — proves the full pipeline works end-to-end against a live local environment.

### Why `quality-gate` is the canonical validation command

The `quality-gate` command is the single entrypoint you should use for all validation. It replaces ad-hoc combinations of individual commands with a disciplined, profile-driven pipeline:

- **Ordered execution**: doctor -> topology -> contracts -> runtime-bindings -> (runtime-smoke). Structure is verified before topology, topology before contracts, contracts before bindings. Each layer depends on the previous one being sound.
- **Profile-driven**: `--fast` for local dev (no infra needed), `--ci` for pipelines (zero-tolerance), `--deep` for pre-merge (full stack proof).
- **Single exit code**: 0 = all clear, 1 = check failure, 2 = execution error. Direct use in `set -e` scripts and CI `if` conditions.
- **Actionable output**: every failure tells you which step broke, how many errors/warnings it produced, and the exact command to run for full details.
- **Execution vs check errors**: IO failures and parse errors are clearly distinguished from check findings, so you know whether to fix the project or fix the environment.
- **JSON/human consistency**: both output formats expose the same data — step timing, finding counts, skip reasons — so scripts and humans see the same truth.

The CLI lives entirely in `tools/` and has zero coupling to the Go runtime. It can evolve independently, gain new analyzers, and serve as a foundation for future quality checks (architecture guards, dependency audits, coverage thresholds) without touching the application code.

### Code Intelligence Layer (`codeintel/`)

The `codeintel/` module provides structural indexing of Go source files — a deterministic, AST-like representation built without the Go compiler or any external tooling.

```
codeintel/
├── types.rs    — Canonical data structures (GoFile, GoPackage, GoType, GoFunc, ...)
├── walker.rs   — Directory walker (collects .go files, skips vendor/hidden)
├── parser.rs   — Single-file parser (source text → GoFile)
└── index.rs    — Cross-file indexer (GoFile[] → ProjectIndex with queries)
```

**What it indexes (Phase 1):**

| Fact | Structure | Example |
|------|-----------|---------|
| Package declarations | `GoFile.package` | `package configctl` |
| Imports (classified) | `GoImport { path, alias, kind }` | `ImportKind::Stdlib`, `Internal`, `External` |
| Struct definitions | `TypeKind::Struct { fields }` | Fields with name, type, tag, embedded flag |
| Interface definitions | `TypeKind::Interface { methods, embeds }` | Method signatures + embedded interfaces |
| Type aliases | `TypeKind::Alias { underlying }` | `type VersionLifecycle string` |
| Functions & methods | `GoFunc { name, receiver, params, returns }` | Pointer/value receivers, visibility |
| Constants | `GoConst { name, type_hint, value }` | Iota blocks, typed constants |
| Variables | `GoVar { name, type_hint, value }` | Package-level `var` declarations |
| File metadata | `GoFile { is_test, line_count }` | Test file detection |

**Query API on `ProjectIndex`:**

| Method | Returns |
|--------|---------|
| `find_type(name)` | All types with given name across packages |
| `find_func(name)` | All functions/methods with given name |
| `methods_of(type_name)` | All methods on a receiver type |
| `find_package(dir)` | Package by directory path |
| `all_interfaces()` | Every interface in the index |
| `all_structs()` | Every struct in the index |
| `files_in_dir(dir)` | All files in a directory |
| `import_frequency()` | Import paths ranked by usage count |
| `constants_of_type(type)` | All constants with a given type hint |

**Design principles:**

1. **Observable facts only** — every indexed item maps to a concrete source location (file:line). No type inference, no constant evaluation, no semantic binding.
2. **Deterministic** — same source files always produce the same index. No network calls, no caching side effects.
3. **Zero external dependencies** — uses only the Rust stdlib for parsing. Go's regular syntax makes line-based parsing reliable for declarations.
4. **Extensible** — the `ProjectIndex` query API is designed to grow. Future commands add query methods without changing the core structures.

**Deliberate Phase 1 limits:**

| Not in scope | Why |
|---|---|
| Type resolution across packages | Requires import path → filesystem mapping + module resolution |
| Interface satisfaction checking | Requires type resolution + method set comparison |
| Call graph construction | Requires function body analysis |
| Constant/expression evaluation | Requires Go semantics (iota, const folding) |
| Function body analysis | Signatures are sufficient for structural queries |
| LSP integration | Planned as enrichment layer on top of the index |
| Incremental updates | Full rebuild is fast enough for project-scale indexing |

**How it prepares future commands:**

| Command | Uses from codeintel |
|---------|-------------------|
| `impact-map` | `find_type`, `methods_of`, `import_frequency` → change propagation |
| `arch-guard` | `GoImport.kind`, `GoFile.package`, package-level imports → layer validation |
| `symbol-trace` | `find_type`, `find_func`, `constants_of_type` → symbol origin tracking |
| `tdd` | `files_in_dir`, `all_interfaces` → test coverage mapping |
| `coverage-map` | `all_structs`, `all_interfaces` → sensitive area identification |

### LSP Bridge (`lsp/`)

The `lsp/` module provides optional semantic enrichment via `gopls`, layered on top of the `codeintel` AST foundation.

```
lsp/
├── types.rs     — Semantic response types (EnrichedSymbol, FactSource, LspStatus)
├── protocol.rs  — LSP JSON-RPC wire format (encode/decode, LSP data types)
├── client.rs    — gopls process lifecycle (spawn, initialize, shutdown, queries)
└── bridge.rs    — High-level API: merges AST facts with LSP enrichment
```

**Integration strategy:**

```
                ┌─────────────────┐
                │  GoplsBridge    │  ← Single entry point for callers
                │  (bridge.rs)    │
                └───┬─────────┬───┘
                    │         │
           ┌────────▼──┐  ┌──▼──────────┐
           │ codeintel  │  │ GoplsClient │
           │ (AST index)│  │ (client.rs) │
           └────────────┘  └──────┬──────┘
                                  │ stdio JSON-RPC
                           ┌──────▼──────┐
                           │    gopls    │
                           │  (external) │
                           └─────────────┘
```

1. **`GoplsBridge`** is the public API. It builds the codeintel index for AST facts and optionally starts `gopls` for LSP enrichment. If gopls is unavailable, it returns AST-only results tagged with `LspStatus::Unavailable`.

2. **`GoplsClient`** manages the `gopls` child process over stdio. It handles the LSP `initialize` handshake, sends `textDocument/definition`, `textDocument/references`, and `textDocument/hover` requests, and shuts down cleanly on drop.

3. **`protocol`** encodes/decodes the LSP wire format (`Content-Length` header + JSON-RPC body). No external LSP library is needed.

4. **`types`** defines the semantic response model. Every fact carries a `FactSource` tag (`Ast`, `Lsp`, or `Unavailable { reason }`) so consumers know exactly where each piece of information came from.

**Graceful degradation:**

| Condition | Behavior |
|-----------|----------|
| `gopls` not on PATH | `LspStatus::Unavailable` with reason; AST facts returned |
| Workspace invalid | `LspStatus::Unavailable` with reason; AST facts returned |
| Initialize handshake fails | `LspStatus::Unavailable` with reason; AST facts returned |
| Request timeout | Individual query returns empty; other queries still run |
| gopls returns empty | `LspStatus::NoResults`; AST facts returned |
| `--no-lsp` flag | gopls not started; AST-only mode |

**What LSP enrichment adds over AST:**

| Capability | AST (codeintel) | LSP (gopls) |
|------------|-----------------|-------------|
| Definitions | Same-file, name-based | Cross-package, type-resolved |
| References | Structural (fields, params, receivers) | All usages including function bodies |
| Type info | Type expressions as strings | Fully resolved type signatures |
| Documentation | Not available | Go doc comments |
| Call sites | Not available | Via references in function bodies |

**Design decision: AST-first, LSP-second.** The AST foundation is deterministic, fast, and requires no external tools. LSP enrichment is valuable but inherently environment-dependent (requires `gopls`, valid Go workspace, network-free but process-dependent). By keeping them separate with clear provenance tags, consumers can always trust the AST layer and treat LSP results as bonus information.

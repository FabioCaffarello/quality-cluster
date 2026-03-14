# Development Workflow

Canonical workflow for developing, validating, and troubleshooting the quality-service cluster. The `raccoon-cli` (in `tools/raccoon-cli/`) enforces engineering discipline at every stage — structure, topology, contracts, bindings, architecture, drift, and runtime behavior — without touching the Go runtime.

## Quick Reference

```sh
make briefing       # briefing: concise context about a target area or symbol
make check          # guard rail before coding — are we in a known-good state?
make verify         # after changes — Go tests + quality-gate
make check-deep     # full proof — requires make up-dataplane
make smoke          # e2e against live cluster
make scenario-smoke SCENARIO=happy-path  # named scenario
make trace-pack     # collect evidence for debugging
make results-inspect # inspect validator results
make coverage-map   # show quality coverage map and gaps
make tdd            # TDD guide: what to validate for your changes
make arch-guard     # check architecture layer boundaries
make drift-detect   # cross-layer drift detection
make snapshot       # golden snapshot of code intelligence (JSON)
make recommend      # smart recommendations: what to validate after changes
make snapshot-diff SNAP1=before.json SNAP2=after.json  # compare snapshots
make baseline-drift BASELINE=baseline.json             # detect semantic drift
```

## The Flow

### 1. Inspect — understand before touching

Before writing any code, confirm the codebase is in a known-good state:

```sh
make check
```

This runs `raccoon-cli quality-gate` (fast profile): doctor, topology-doctor, contract-audit, runtime-bindings, arch-guard, and drift-detect. No infrastructure needed.

- **"Safe to proceed"** → go ahead.
- **"Stop — N errors"** → fix what's broken first. The output tells you exactly what and where.

To also see the cluster state:

```sh
make ps             # compose service status
make logs           # stream all logs (or SERVICE=server for one)
```

### 2. Pre-change guard — structural + architectural + drift analysis

If you're about to change configs, compose, contracts, or wiring, `make check` is your pre-flight. The full gate now catches:

- Missing or misnamed config files (doctor)
- Compose service gaps or dependency errors (topology-doctor)
- Kafka/NATS URL drift across configs (topology-doctor)
- Subject/stream/durable misalignment (contract-audit)
- Broken messaging contract invariants (contract-audit)
- Runtime binding chain drift (runtime-bindings)
- Layer boundary violations — domain purity, adapter isolation (arch-guard)
- Cross-layer declaration/config/source/doc misalignment (drift-detect)

For targeted pre-change analysis:

```sh
make arch-guard     # did my import break a layer boundary?
make drift-detect   # are my configs/compose/source/docs still aligned?
```

### 3. TDD — impact-driven, scenario first, then code

Before writing production code, understand what your change affects structurally:

```sh
make tdd            # auto-detect changed files via git status
```

Or for specific files you plan to change:

```sh
raccoon-cli tdd internal/adapters/nats/codec.go deploy/configs/consumer.jsonc
raccoon-cli -v tdd  # verbose: show exported symbols, risks, per-file details
raccoon-cli --json tdd  # structured output for tooling
```

The `tdd` command uses AST analysis and structural impact tracing to answer:

- **Which symbols/packages are affected?** — exported types, interfaces, dependents traced via codeintel
- **Which existing tests cover this code?** — nearby `_test.go` files in the same package
- **Which coverage gaps exist?** — areas with no Go tests and/or no runtime scenario
- **Which scenarios and gate profile should be run?** — e.g., `invalid-payload` for validator changes, `deep` profile for infra-touching areas
- **What to run BEFORE and AFTER?** — concrete commands, not guesswork

The discipline:

1. **Confirm baseline**: run the recommended BEFORE commands. If they fail, fix first.
2. **Write/update test or scenario**: define the new expected behavior — a Go test, a smoke scenario, or a coverage expectation. The coverage gaps section tells you what's missing.
3. **Implement the change**: now code.
4. **Prove safety**: run the AFTER commands. If they pass, the change is safe.

Without a passing baseline, you can't know your change is safe. Without a test for the new behavior, success is a guess.

### 4. Validate locally — fast feedback

After changes:

```sh
make verify         # runs: go test + quality-gate (fast)
```

This catches both Go compilation/test failures and structural/contract/architecture regressions in one command.

For targeted checks when you know what you changed:

```sh
raccoon-cli doctor              # just structure
raccoon-cli topology-doctor     # just topology wiring
raccoon-cli contract-audit      # just messaging contracts
raccoon-cli runtime-bindings    # just binding chain
raccoon-cli arch-guard          # just layer boundaries
raccoon-cli drift-detect        # just cross-layer alignment
raccoon-cli -v contract-audit   # verbose — show all findings
```

### 5. Validate for CI — strict mode

```sh
make quality-gate-ci    # warnings become errors, JSON output
```

This is what CI runs. Locally, use it to preview exactly what CI will see before pushing.

### 6. Validate with live environment — full proof

```sh
make up-dataplane       # start nats + kafka + configctl + server + validator + consumer + emulator
make check-deep         # quality-gate deep: static checks + runtime-smoke
```

The deep profile adds `runtime-smoke`, which proves the full pipeline end-to-end:
1. Checks all 7 compose services are running
2. Polls healthz/readyz until ready
3. Creates a smoke config through the full lifecycle (draft → validate → compile → activate)
4. Confirms ingestion bindings are projected
5. Waits for validation results (Kafka → consumer → JetStream → validator)
6. Checks results contain both passed and failed entries

### 7. Validate by scenario — targeted runtime proof

For specific runtime validation without the full gate:

```sh
make scenario-smoke SCENARIO=happy-path     # full E2E: lifecycle + data plane + results
make scenario-smoke SCENARIO=config-lifecycle  # control plane only
make scenario-smoke SCENARIO=invalid-payload   # validator catches bad payloads
make scenario-smoke SCENARIO=missing-binding   # query non-existent scope
make scenario-smoke SCENARIO=readiness-probe   # bootstrap + readiness
make scenario-smoke                            # list all available scenarios
```

Each scenario reuses relevant stages from runtime-smoke, giving targeted proof without running everything.

### 8. Troubleshoot — evidence before guessing

When something breaks:

```sh
# Collect everything into a compressed trace pack:
make trace-pack

# Inspect validation results:
make results-inspect

# Filter to failures only:
raccoon-cli results-inspect --failed-only

# Filter by binding:
raccoon-cli results-inspect --binding orders --latest 5

# Verbose individual analyzer for targeted debugging:
raccoon-cli -v topology-doctor
raccoon-cli -v runtime-bindings
raccoon-cli -v drift-detect
```

The trace pack collects compose status, API responses, deploy configs, and service logs into a single timestamped archive — everything needed to diagnose a failure without live cluster access.

### 9. Coverage map — know what's protected

To see which quality dimensions and scenarios cover each sensitive area:

```sh
make coverage-map
```

This shows:
- All quality dimensions (static and runtime)
- Which sensitive areas have full coverage
- Go test file distribution across packages
- Gaps that need attention

Use this before adding a new feature to check if existing scenarios will catch regressions, or after refactoring to confirm coverage still holds.

### 10. Recommend — what to validate after a change

Before running checks manually, ask the CLI what matters:

```sh
make recommend                           # auto-detect changed files via git status
raccoon-cli recommend internal/adapters/nats/codec.go   # specific files
raccoon-cli recommend --baseline baseline.json           # drift-aware mode
raccoon-cli --json recommend             # structured output for tooling
raccoon-cli -v recommend                 # verbose: file list, risk details
```

The `recommend` command composes signals from **impact-map** (AST structural analysis), **tdd** (coverage gaps), and optional **baseline-drift** (snapshot comparison) to produce:

- **Smoke scenarios** to run, with priority and rationale
- **Quality-gate profile** (fast/ci/deep) — calibrated to the change scope
- **Priority test areas** with coverage status (covered / uncovered / partial)
- **Architectural and contract risks** to review
- **BEFORE / AFTER command plan** — concrete commands, not guesswork

Every item is tagged with provenance: `[fact]` (observed from AST), `[inference]` (derived from patterns), or `[recommendation]` (actionable suggestion).

**When to use**: after staging changes and before running checks. The recommend output tells you *which* checks matter for *your* change, instead of running everything blindly.

### 11. Golden snapshots — baseline, comparison, and drift detection

Generate a deterministic snapshot of the repository's code intelligence state:

```sh
raccoon-cli snapshot                     # human summary
raccoon-cli --json snapshot              # full JSON snapshot
raccoon-cli --json snapshot -o snap.json # save to file
raccoon-cli -v snapshot                  # verbose: types, functions, imports
```

The snapshot captures packages, imports, types, functions, constants, interfaces, architecture layer classification, and detected contracts. Every fact is tagged with its provenance (`ast`, `lsp`, `inferred`, or `runtime`).

**Comparing snapshots** — semantic diff between two points in time:

```sh
raccoon-cli --json snapshot -o before.json
# ... make changes ...
raccoon-cli --json snapshot -o after.json
make snapshot-diff SNAP1=before.json SNAP2=after.json
raccoon-cli --json snapshot-diff before.json after.json   # JSON output
```

This produces a semantic diff (not textual): added/removed/modified types, functions, imports, interfaces, contracts, and architecture layers.

**Baseline drift** — structured analysis with severity and recommendations:

```sh
raccoon-cli --json snapshot -o baseline.json
# ... make changes ...
make baseline-drift BASELINE=baseline.json
raccoon-cli -v baseline-drift baseline.json   # verbose with evidence
raccoon-cli --json baseline-drift baseline.json   # JSON output
```

This detects 10 classes of semantic drift:

| Class | Basis | What it catches |
|-------|-------|-----------------|
| contract-surface-drift | observed | Removed/modified/added contracts |
| interface-breaking | observed | Removed interface methods |
| interface-expansion | observed | Added interface methods |
| layer-boundary-drift | observed | Architecture layer reclassification or removal |
| type-breaking | observed | Removed fields, type changes |
| api-signature-drift | observed | Exported function signature changes |
| coupling-increase | inferred | New cross-layer imports |
| isolation-loss | inferred | Domain/application importing infrastructure |
| contract-proliferation | heuristic | Rapid contract growth without validation |
| structural-scale-shift | heuristic | Large-scale type/line/package count changes |

Each finding includes severity (critical/warning/info), evidence basis (observed/inferred/heuristic), concrete evidence, and a recommended next step.

**Drift-aware recommendations** — combine baseline drift with recommend:

```sh
raccoon-cli recommend --baseline baseline.json
```

When a baseline is provided, `recommend` escalates scenarios and profile based on drift severity. Critical drift forces `deep` profile; contract drift adds `happy-path`; breaking changes add `invalid-payload`.

## Quality Gate Pipeline

The `quality-gate` command orchestrates all checks into a single verdict. Each profile includes progressively more checks:

| Step | fast | ci | deep |
|------|------|----|------|
| doctor | yes | yes (strict) | yes |
| topology-doctor | yes | yes (strict) | yes |
| contract-audit | yes | yes (strict) | yes |
| runtime-bindings | yes | yes (strict) | yes |
| arch-guard | yes | yes (strict) | yes |
| drift-detect | yes | yes (strict) | yes |
| runtime-smoke | skip | skip | yes |

- **fast** (default): all static analyzers, no infrastructure needed
- **ci**: same checks, warnings promoted to errors (zero tolerance)
- **deep**: static + runtime-smoke (requires `make up-dataplane`)

## Compose Profiles

The cluster is layered by profile:

| Target | Profile | Services |
|--------|---------|----------|
| `make up-core` | core | nats, configctl, server |
| `make up-runtime` | core + runtime | + validator |
| `make up-dataplane` | core + runtime + dataplane | + kafka, consumer, emulator |
| `make up-all` | all | everything |

Stop everything: `make down`

## CI Integration

```yaml
# .github/workflows/quality.yml
quality-gate:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Build raccoon-cli
      run: make raccoon-build
    - name: Quality gate (strict)
      run: make quality-gate-ci
```

The `ci` profile promotes warnings to errors — zero tolerance. `--json` output is machine-parseable.

## Scoping

```sh
make test MODULE=./internal/shared       # test one Go module
make build SERVICE=server                # build one binary
make logs SERVICE=validator              # logs for one service
make restart SERVICE=consumer            # restart one service
```

## CLI Reference

Full command documentation: [`tools/raccoon-cli/README.md`](tools/raccoon-cli/README.md)

| Command | What it does | Infra needed |
|---------|-------------|--------------|
| `doctor` | Project structure validation | No |
| `topology-doctor` | Config/compose/source consistency | No |
| `contract-audit` | Messaging contract invariants | No |
| `runtime-bindings` | Config → kafka → jetstream → validator chain | No |
| `arch-guard` | Clean architecture layer boundaries | No |
| `drift-detect` | Cross-layer declaration/config/source alignment | No |
| `coverage-map` | Quality coverage map and gap analysis | No |
| `tdd` | Impact-driven TDD guide (AST + structural analysis) | No |
| `quality-gate` | All above, orchestrated with profiles | No (fast/ci) |
| `runtime-smoke` | E2E pipeline proof | Yes (`up-dataplane`) |
| `scenario-smoke` | Named validation scenarios | Yes (varies) |
| `results-inspect` | Validator result inspection | Yes (running cluster) |
| `trace-pack` | Diagnostic evidence collection | Yes (running cluster) |
| `contract-usage-map` | Map contract definition/construction/propagation/consumption | No |
| `snapshot` | Golden snapshot of code intelligence state | No |
| `snapshot-diff` | Semantic diff between two snapshots | No |
| `baseline-drift` | Detect semantic drift against a saved baseline | No |
| `recommend` | Smart recommendations: scenarios, profile, risks from diff/baseline | No |

## The Semantic Evolution Cycle

The CLI closes the loop between understanding, changing, and proving. Every evolution of the repository follows this cycle:

```
  baseline → understand → plan → implement → recommend → validate → new baseline
     ↑                                                                    │
     └────────────────────────────────────────────────────────────────────┘
```

In practice:

```sh
# 1. Capture baseline (once, or at each stable point)
raccoon-cli --json snapshot -o baseline.json

# 2. Understand what you're about to touch
make briefing TARGETS="internal/adapters/nats"
make check                              # guard rails: known-good state?

# 3. Plan with structural awareness
make tdd                                # what impacts, what to test, what's missing

# 4. Implement (write tests first, then code)

# 5. Get smart recommendations for YOUR change
make recommend                          # or: raccoon-cli recommend --baseline baseline.json

# 6. Validate exactly what matters
make verify                             # Go tests + quality-gate
make scenario-smoke SCENARIO=happy-path # if recommend says so

# 7. Confirm no drift, update baseline
make baseline-drift BASELINE=baseline.json
raccoon-cli --json snapshot -o baseline.json   # new baseline
```

The cycle is self-reinforcing: each baseline captures the proven state, each recommend calibrates validation to the actual change, and each gate run produces structured evidence. No step is wasted.

## Principles

- **Test first, then implement, then prove.** Without a test for the new behavior, success is a guess. `make tdd` traces structural impact to tell you exactly what to validate and where coverage is weak.
- **Understand first, validate, then change.** Never code blind — `make check` takes seconds.
- **Guard rails over discipline.** The gate catches what humans forget. `arch-guard` enforces layer boundaries; `drift-detect` catches what drifted while you weren't looking.
- **Evidence before confidence.** The quality gate produces structured, reproducible proof.
- **Single source of truth.** `quality-gate` is the canonical validation command — don't compose ad-hoc check sequences.
- **No runtime contamination.** The CLI reads files and scans source — it never imports or executes Go code.
- **Failures are actionable.** Every error includes what, why, and how to fix.
- **Recommend before running.** `make recommend` tells you what checks matter for your change — don't guess.
- **Coverage awareness.** `make coverage-map` shows which areas are protected and which are exposed.
- **Baselines over memory.** Snapshots are deterministic proof of state — not "I think it was fine last week".
- **Scenarios over manual testing.** Use `scenario-smoke` for repeatable runtime proof — not ad-hoc curl sequences.

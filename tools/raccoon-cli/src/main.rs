mod analyzers;
mod codeintel;
mod error;
mod gate;
mod lsp;
mod models;
mod output;
mod results_inspect;
mod smoke;
mod trace_pack;

use clap::{Parser, Subcommand, ValueEnum};
use output::OutputFormat;
use std::process;

#[derive(Parser)]
#[command(
    name = "raccoon-cli",
    about = "Engineering quality toolkit for quality-service",
    long_about = "Engineering quality toolkit for quality-service.\n\n\
        Validates project structure, data pipeline topology, messaging contracts,\n\
        runtime bindings, and runtime behavior — fully isolated from the Go runtime.",
    version,
    propagate_version = true,
    after_help = "Quick start:\n  \
        raccoon-cli doctor                           # project structure check\n  \
        raccoon-cli quality-gate                     # fast static checks\n  \
        raccoon-cli quality-gate --profile ci --json # CI pipeline\n  \
        raccoon-cli quality-gate --profile deep      # full validation (requires running infra)"
)]
struct Cli {
    /// Output as JSON instead of human-readable text
    #[arg(long, global = true)]
    json: bool,

    /// Show detailed findings for all checks, not just failures
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Path to the project root (defaults to current directory)
    #[arg(long, global = true, default_value = ".")]
    project_root: std::path::PathBuf,

    #[command(subcommand)]
    command: Commands,
}

/// Execution profile for quality-gate
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum GateProfile {
    /// Static checks only (topology-doctor + contract-audit), no infra needed
    Fast,
    /// Same as fast, but warnings become failures (strict for CI)
    Ci,
    /// All checks including runtime-smoke (requires running environment)
    Deep,
}

impl From<GateProfile> for gate::Profile {
    fn from(p: GateProfile) -> Self {
        match p {
            GateProfile::Fast => gate::Profile::Fast,
            GateProfile::Ci => gate::Profile::Ci,
            GateProfile::Deep => gate::Profile::Deep,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Validate project structure (go.work, internal/, deploy/, tests/)
    #[command(after_help = "Examples:\n  \
        raccoon-cli doctor\n  \
        raccoon-cli --project-root /path/to/quality-service doctor")]
    Doctor,
    /// Audit the data pipeline topology: emulator -> kafka -> consumer -> jetstream -> validator
    #[command(after_help = "Examples:\n  \
        raccoon-cli topology-doctor\n  \
        raccoon-cli --json topology-doctor")]
    TopologyDoctor,
    /// Audit messaging contracts and invariants across Kafka, NATS/JetStream, and internal transports
    #[command(after_help = "Examples:\n  \
        raccoon-cli contract-audit\n  \
        raccoon-cli --json contract-audit | jq '.checks[] | select(.status == \"fail\")'")]
    ContractAudit,
    /// Inspect runtime bindings: config → kafka → jetstream → validator routing
    #[command(after_help = "Examples:\n  \
        raccoon-cli runtime-bindings\n  \
        raccoon-cli --json runtime-bindings")]
    RuntimeBindings,
    /// Detect drift between declared architecture, configuration, runtime topology, and documentation
    #[command(
        long_about = "Detect drift between what the system declares, what it configures, what the source \
            wires, and what the documentation says.\n\n\
            Drift classes:\n  \
              1. Config ↔ Compose: services in configs vs compose, transport dependency alignment\n  \
              2. Config ↔ Source: stream/durable/subject constants vs config declarations\n  \
              3. Binding ↔ Topology: declared bindings vs routing infrastructure\n  \
              4. Workflow ↔ Reality: DEVELOPMENT.md targets vs actual Makefile targets\n  \
              5. Contract ↔ Domain: registry event specs vs domain event definitions\n  \
              6. Compose ↔ Profiles: profile assignments vs Makefile up-* targets",
        after_help = "Examples:\n  \
            raccoon-cli drift-detect\n  \
            raccoon-cli --json drift-detect\n  \
            raccoon-cli -v drift-detect"
    )]
    DriftDetect,
    /// Guard architectural boundaries using AST-based semantic analysis
    #[command(
        long_about = "Detect violations of the clean architecture layer rules using structural \
            analysis (AST-based codeintel index). Goes beyond import-path checking to inspect \
            type definitions, struct fields, interface signatures, and function parameters.\n\n\
            Rules enforced:\n  \
              1.  Layer dependency direction (domain → application → adapters → actors → interfaces)\n  \
              2.  Domain purity (no infrastructure imports in domain/)\n  \
              3.  Application isolation (no direct adapter imports)\n  \
              4.  Interfaces isolation (no adapter/actor imports in HTTP handlers)\n  \
              5.  Cmd boundary (AST type counting — cmd/ wires, does not define models)\n  \
              6.  Tooling boundary (tools/ must not contain Go modules)\n  \
              7.  No cross-cmd imports (binaries are independently deployable)\n  \
              8.  Deploy boundary (no hardcoded deploy/ paths in Go source)\n  \
              9.  Port contract leaks (port interfaces must not reference infra types)\n  \
              10. Domain type contamination (struct fields must not embed infra types)\n  \
              11. Exported signature leaks (domain/application funcs must not expose infra types)",
        after_help = "Examples:\n  \
            raccoon-cli arch-guard\n  \
            raccoon-cli --json arch-guard\n  \
            raccoon-cli arch-guard --project-root /path/to/quality-service"
    )]
    ArchGuard,
    /// Trace a symbol across the repository: definitions, structural references, contracts, and packages
    #[command(
        long_about = "Trace a symbol (type, function, constant, variable) across the Go codebase.\n\n\
            Uses the codeintel AST index to find:\n  \
              - Where the symbol is defined (type, file, line, visibility)\n  \
              - Where it is structurally referenced (struct fields, function params/returns,\n    \
                receivers, interface embeds, type aliases, const/var type hints)\n  \
              - Which packages are involved\n  \
              - Contract connections (ports, message types, interfaces)\n  \
              - Recommended raccoon-cli checks\n\n\
            With --lsp, enriches results via gopls:\n  \
              - Type-resolved definitions (cross-package)\n  \
              - Semantic references (function body call sites)\n  \
              - Hover/type signature information\n  \
              Each fact is tagged [ast] or [lsp] in the output.\n\n\
            Limitations:\n  \
              - AST: no function body analysis without --lsp\n  \
              - LSP: depends on gopls workspace state; falls back cleanly",
        after_help = "Examples:\n  \
            raccoon-cli symbol-trace ConfigSet\n  \
            raccoon-cli symbol-trace --lsp ConfigSet              # enrich with gopls\n  \
            raccoon-cli symbol-trace VersionLifecycle\n  \
            raccoon-cli symbol-trace ConfigctlGateway\n  \
            raccoon-cli --json symbol-trace --lsp CreateDraftCommand\n  \
            raccoon-cli -v symbol-trace --lsp ConfigSet"
    )]
    SymbolTrace {
        /// The symbol name to trace (type, function, constant, or variable)
        symbol: String,
        /// Enrich with gopls definitions, references, and hover info
        #[arg(long)]
        lsp: bool,
        /// Skip gopls even if --lsp is set (useful for benchmarking AST-only)
        #[arg(long)]
        no_lsp: bool,
    },
    /// Run end-to-end smoke test against a live local environment (requires `make up-dataplane`)
    #[command(after_help = "Examples:\n  \
        raccoon-cli runtime-smoke\n  \
        raccoon-cli runtime-smoke --base-url http://localhost:9090")]
    RuntimeSmoke {
        /// Base URL for the quality-service HTTP API
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
    },
    /// Run a named validation scenario against a live local environment
    #[command(
        long_about = "Run a named, reproducible validation scenario against the quality-service cluster.\n\n\
            Each scenario declares preconditions, executes a deterministic sequence of checks,\n\
            and reports structured pass/fail results.\n\n\
            Available scenarios:\n  \
              happy-path        — full E2E: config lifecycle + data plane + validation results\n  \
              config-lifecycle  — control plane only: draft -> validate -> compile -> activate\n  \
              invalid-payload   — activate config and verify validator catches bad data\n  \
              missing-binding   — query non-existent scope and verify empty results\n  \
              readiness-probe   — quick cluster health check (bootstrap + readiness)",
        after_help = "Examples:\n  \
            raccoon-cli scenario-smoke happy-path\n  \
            raccoon-cli scenario-smoke config-lifecycle\n  \
            raccoon-cli --json scenario-smoke invalid-payload\n  \
            raccoon-cli scenario-smoke --list"
    )]
    ScenarioSmoke {
        /// Scenario name to execute (omit with --list to see all scenarios)
        #[arg(required_unless_present = "list")]
        scenario: Option<String>,
        /// Base URL for the quality-service HTTP API
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
        /// List all available scenarios and exit
        #[arg(long)]
        list: bool,
    },
    /// Inspect validation results from the running validator
    #[command(
        long_about = "Inspect validation results produced by the quality-service validator.\n\n\
            Shows summaries of pass/fail counts, binding breakdowns, violation rules,\n\
            and individual result details. Requires a running quality-service.",
        after_help = "Examples:\n  \
            raccoon-cli results-inspect\n  \
            raccoon-cli results-inspect --failed-only\n  \
            raccoon-cli results-inspect --latest 5 --json\n  \
            raccoon-cli results-inspect --binding orders --limit 50"
    )]
    ResultsInspect {
        /// Base URL for the quality-service HTTP API
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
        /// Scope kind for the results query
        #[arg(long, default_value = "global")]
        scope_kind: String,
        /// Scope key for the results query
        #[arg(long, default_value = "default")]
        scope_key: String,
        /// Filter by binding name
        #[arg(long)]
        binding: Option<String>,
        /// Filter by topic
        #[arg(long)]
        topic: Option<String>,
        /// Maximum number of results to fetch from the API (1-100)
        #[arg(long, default_value_t = 100)]
        limit: u32,
        /// Show only failed validation results
        #[arg(long)]
        failed_only: bool,
        /// Show only the N most recent results
        #[arg(long)]
        latest: Option<u32>,
    },
    /// Map structural impact of changed files, packages, or symbols
    #[command(
        long_about = "Map the potential impact of changes to files, packages, or symbols.\n\n\
            Uses the codeintel AST index to trace import relationships, exported symbols,\n\
            and contract surface. Differentiates observed facts from inferred risks.\n\n\
            Targets can be:\n  \
              - File paths: internal/domain/configctl/config.go\n  \
              - Package dirs: internal/domain/configctl\n  \
              - Symbol names: ConfigSet\n\n\
            If no targets are given, uses `git status` to detect changed files.",
        after_help = "Examples:\n  \
            raccoon-cli impact-map internal/domain/configctl/config.go\n  \
            raccoon-cli impact-map --lsp internal/adapters/nats/   # enrich with gopls\n  \
            raccoon-cli impact-map ConfigSet\n  \
            raccoon-cli impact-map  # uses git status\n  \
            raccoon-cli --json impact-map --lsp internal/application/ports/configctl.go"
    )]
    ImpactMap {
        /// Enrich with gopls references for exported symbols
        #[arg(long)]
        lsp: bool,
        /// Skip gopls even if --lsp is set
        #[arg(long)]
        no_lsp: bool,
        /// Targets to analyze (files, packages, or symbols). If omitted, uses git status.
        #[arg(trailing_var_arg = true)]
        targets: Vec<String>,
    },
    /// Show quality coverage map: which dimensions, scenarios, and Go tests cover each sensitive area
    #[command(
        long_about = "Show which quality dimensions and scenarios cover each sensitive area of the codebase.\n\n\
            Reports:\n  \
              - All quality dimensions (static and runtime)\n  \
              - Sensitive areas with their coverage status\n  \
              - Go test file distribution\n  \
              - Coverage gaps that need attention",
        after_help = "Examples:\n  \
            raccoon-cli coverage-map\n  \
            raccoon-cli --json coverage-map\n  \
            raccoon-cli -v coverage-map"
    )]
    CoverageMap,
    /// Impact-driven TDD guide: structural analysis of what to validate before and after changes
    #[command(
        long_about = "Impact-driven TDD guidance using AST analysis and structural impact tracing.\n\n\
            Given a list of files you plan to change (or auto-detected via `git status`):\n  \
              1. Traces exported symbols, dependents, and contract surface (via codeintel)\n  \
              2. Identifies affected sensitive areas and coverage gaps\n  \
              3. Finds existing tests near the changed code\n  \
              4. Recommends specific checks, scenarios, and gate profile\n  \
              5. Shows BEFORE/AFTER commands for disciplined TDD flow\n\n\
            Unlike simple file-pattern matching, the structural analysis understands\n\
            which packages depend on your changes, which interfaces are part of the\n\
            contract surface, and where test coverage is weak.",
        after_help = "Examples:\n  \
            raccoon-cli tdd internal/adapters/nats/codec.go\n  \
            raccoon-cli tdd deploy/configs/consumer.jsonc internal/actors/scopes/validator/supervisor.go\n  \
            raccoon-cli tdd  # uses git status to detect changed files\n  \
            raccoon-cli --json tdd internal/domain/configctl/config.go\n  \
            raccoon-cli -v tdd  # verbose: show symbols, risks, and per-file details"
    )]
    Tdd {
        /// Files you plan to change (if omitted, uses git status)
        #[arg(trailing_var_arg = true)]
        files: Vec<String>,
    },
    /// Run consolidated quality gate with per-step timing and a single exit code
    #[command(
        long_about = "Run consolidated quality gate: doctor + topology + contracts + runtime-bindings + smoke.\n\n\
            Profiles:\n  \
              fast  — doctor + topology-doctor + contract-audit + runtime-bindings (default, no infra needed)\n  \
              ci    — same as fast, warnings become failures (strict)\n  \
              deep  — all checks including runtime-smoke (requires `make up-dataplane`)",
        after_help = "Examples:\n  \
            raccoon-cli quality-gate\n  \
            raccoon-cli quality-gate --profile ci --json\n  \
            raccoon-cli quality-gate --profile deep --base-url http://localhost:9090\n  \
            raccoon-cli quality-gate --fail-fast"
    )]
    QualityGate {
        /// Execution profile (default: fast)
        #[arg(long, value_enum, default_value_t = GateProfile::Fast)]
        profile: GateProfile,
        /// Base URL for runtime-smoke (only used with --profile deep)
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
        /// Stop after the first failing step (skip remaining steps)
        #[arg(long)]
        fail_fast: bool,
    },
    /// Enrich a symbol with semantic information from gopls (optional LSP bridge)
    #[command(
        long_about = "Enrich a symbol with semantic information using the gopls LSP bridge.\n\n\
            Combines deterministic AST facts from codeintel with type-resolved\n\
            definitions, cross-package references, and hover/type info from gopls.\n\n\
            If gopls is not available, returns AST-only results with a clear\n\
            indication that LSP enrichment was unavailable.\n\n\
            Every fact is tagged with its provenance: ast, lsp, or unavailable.",
        after_help = "Examples:\n  \
            raccoon-cli lsp-enrich ConfigSet\n  \
            raccoon-cli --json lsp-enrich VersionLifecycle\n  \
            raccoon-cli lsp-enrich --no-lsp ConfigSet  # AST only, skip gopls\n  \
            raccoon-cli lsp-enrich --timeout 10 ConfigSet"
    )]
    LspEnrich {
        /// The symbol name to enrich
        symbol: String,
        /// Skip gopls and return AST-only results
        #[arg(long)]
        no_lsp: bool,
        /// Timeout in seconds for gopls requests (default: 30)
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Evaluate rename safety: structural impact, contract risk, and blast radius before renaming a symbol
    #[command(
        long_about = "Evaluate the safety of renaming a Go symbol before performing the rename.\n\n\
            Uses the codeintel AST index to trace definitions, structural references,\n\
            contract surface, and sensitive areas. Assesses overall risk and recommends\n\
            the appropriate quality-gate profile and smoke scenarios.\n\n\
            Does NOT execute the rename — this is assessment only.\n\n\
            With --lsp, enriches with gopls references for deeper coverage.\n\
            With --to <NAME>, also checks for naming conflicts.",
        after_help = "Examples:\n  \
            raccoon-cli rename-safety ConfigSet\n  \
            raccoon-cli rename-safety ConfigSet --to QualityConfigSet\n  \
            raccoon-cli rename-safety --lsp ConfigctlGateway\n  \
            raccoon-cli --json rename-safety CreateDraftCommand\n  \
            raccoon-cli -v rename-safety --lsp --to NewName OldName"
    )]
    RenameSafety {
        /// The symbol name to evaluate for renaming
        symbol: String,
        /// Optional new name (checks for conflicts)
        #[arg(long = "to")]
        new_name: Option<String>,
        /// Enrich with gopls references for deeper coverage
        #[arg(long)]
        lsp: bool,
        /// Skip gopls even if --lsp is set
        #[arg(long)]
        no_lsp: bool,
    },
    /// Map where contracts are defined, constructed, propagated, consumed, and validated
    #[command(
        long_about = "Map real contract usage across the repository using AST structural analysis.\n\n\
            For each contract type (envelopes, commands, queries, replies, events, records, bindings, etc.):\n  \
              - Definition: where the type is declared\n  \
              - Construction: factory functions, builder methods, struct literals\n  \
              - Propagation: parameters, returns, embeddings, interface methods\n  \
              - Consumption: handlers, decoders, field access\n  \
              - Validation: Validate/Normalize methods\n\n\
            Differentiates observed facts from heuristic inferences.\n\
            With --lsp, enriches with gopls references for function body call sites.",
        after_help = "Examples:\n  \
            raccoon-cli contract-usage-map\n  \
            raccoon-cli --json contract-usage-map\n  \
            raccoon-cli -v contract-usage-map               # show per-contract details\n  \
            raccoon-cli contract-usage-map --lsp             # enrich with gopls"
    )]
    ContractUsageMap {
        /// Enrich with gopls references for deeper coverage
        #[arg(long)]
        lsp: bool,
        /// Skip gopls even if --lsp is set
        #[arg(long)]
        no_lsp: bool,
    },
    /// Generate a concise, auditable briefing about an area, symbol, file, or change
    #[command(
        long_about = "Generate a short, dense briefing combining impact analysis, architecture checks,\n\
            contract health, and TDD guidance for a given set of targets.\n\n\
            Designed for pasting into agent context or reading during development.\n\
            Every item is tagged: [fact], [inferred], or [recommendation].\n\n\
            Targets can be:\n  \
              - File paths: internal/domain/configctl/config.go\n  \
              - Package dirs: internal/domain/configctl\n  \
              - Symbol names: ConfigSet (PascalCase)\n  \
              - Multiple targets: mix of the above\n\n\
            If no targets are given, uses `git status` to detect changed files.",
        after_help = "Examples:\n  \
            raccoon-cli briefing internal/domain/configctl/config.go\n  \
            raccoon-cli briefing ConfigSet\n  \
            raccoon-cli briefing --lsp internal/adapters/nats/\n  \
            raccoon-cli --json briefing internal/domain/configctl/\n  \
            raccoon-cli briefing  # auto-detect from git status"
    )]
    Briefing {
        /// Enrich with gopls references for deeper coverage
        #[arg(long)]
        lsp: bool,
        /// Skip gopls even if --lsp is set
        #[arg(long)]
        no_lsp: bool,
        /// Targets to analyze (files, packages, or symbols). If omitted, uses git status.
        #[arg(trailing_var_arg = true)]
        targets: Vec<String>,
    },
    /// Detect semantic drift between a baseline snapshot and the current repository state
    #[command(
        long_about = "Compare the current repository against a previously saved baseline snapshot\n\
            to detect semantic drift — structural changes that may indicate divergence\n\
            from expected architecture, contracts, or invariants.\n\n\
            Drift classes detected:\n  \
              1. Contract surface drift: removed/modified/added contracts\n  \
              2. Interface breaking: removed interface methods\n  \
              3. Interface expansion: added interface methods\n  \
              4. Layer boundary drift: architecture layer changes\n  \
              5. Type breaking: removed fields, type changes\n  \
              6. API signature drift: exported function signature changes\n  \
              7. Coupling increase: new cross-layer imports\n  \
              8. Isolation loss: domain/application importing infrastructure\n  \
              9. Contract proliferation: rapid growth without validation\n  \
              10. Structural scale shift: large-scale code changes\n\n\
            Every finding is tagged with its evidence basis:\n  \
              - observed: directly from the snapshot diff\n  \
              - inferred: derived from combining multiple facts\n  \
              - heuristic: statistical or pattern-based",
        after_help = "Examples:\n  \
            raccoon-cli baseline-drift baseline.json\n  \
            raccoon-cli --json baseline-drift baseline.json\n  \
            raccoon-cli -v baseline-drift baseline.json\n  \
            raccoon-cli --json snapshot -o baseline.json   # save baseline first"
    )]
    BaselineDrift {
        /// Path to the baseline snapshot JSON file
        baseline: std::path::PathBuf,
    },
    /// Generate a golden snapshot of the repository's code intelligence
    #[command(
        long_about = "Generate a deterministic, auditable snapshot of the repository's structural\n\
            and semantic state as observed by the codeintel layer.\n\n\
            The snapshot captures:\n  \
              - Packages, imports, types, functions, constants, interfaces\n  \
              - Architecture layer classification per package\n  \
              - Detected contract types and families\n  \
              - Aggregate statistics\n\n\
            Every fact is tagged with its provenance: ast, lsp, inferred, or runtime.\n\
            Output is sorted and deterministic — same source tree produces the same\n\
            snapshot (modulo metadata.generated_at).\n\n\
            Use for baseline comparison, drift detection, and debugging.",
        after_help = "Examples:\n  \
            raccoon-cli snapshot\n  \
            raccoon-cli --json snapshot\n  \
            raccoon-cli --json snapshot --output snapshot.json\n  \
            raccoon-cli -v snapshot                              # show types, functions, imports\n  \
            diff <(raccoon-cli --json snapshot) baseline.json    # detect drift"
    )]
    Snapshot {
        /// Save JSON output to a file instead of stdout
        #[arg(long, short)]
        output: Option<std::path::PathBuf>,
    },
    /// Compare two snapshots and produce a semantic diff report
    #[command(
        long_about = "Compare two code intelligence snapshots and produce a structured diff.\n\n\
            Highlights additions, removals, and modifications across all snapshot sections:\n\
            packages, imports, types, functions, constants, interfaces, arch layers, and contracts.\n\n\
            Changes are reported semantically (field added, signature changed, method removed)\n\
            rather than as raw text diffs. The report separates observed facts from derived\n\
            inferences about impact and risk.\n\n\
            Both snapshots must have the same format version. Corrupted or incompatible\n\
            snapshots are detected and reported clearly.",
        after_help = "Examples:\n  \
            raccoon-cli snapshot-diff before.json after.json\n  \
            raccoon-cli --json snapshot-diff baseline.json current.json\n  \
            raccoon-cli -v snapshot-diff old.json new.json\n  \
            raccoon-cli snapshot-diff before.json --after-live     # compare file vs live project"
    )]
    SnapshotDiff {
        /// Path to the 'before' snapshot JSON file
        before: std::path::PathBuf,
        /// Path to the 'after' snapshot JSON file (omit with --after-live to use current project)
        #[arg(required_unless_present = "after_live")]
        after: Option<std::path::PathBuf>,
        /// Use a live snapshot of the current project as 'after' instead of a file
        #[arg(long)]
        after_live: bool,
    },
    /// Collect diagnostic evidence from the running cluster into a trace pack
    #[command(
        long_about = "Collect diagnostic evidence from the running quality-service cluster.\n\n\
            Produces a timestamped directory (or .tar.gz) with compose status, API responses,\n\
            deploy configs, and recent service logs — everything needed to diagnose a failure\n\
            without live cluster access.",
        after_help = "Examples:\n  \
            raccoon-cli trace-pack\n  \
            raccoon-cli trace-pack --compress\n  \
            raccoon-cli trace-pack --log-lines 500 --output-dir /tmp/traces"
    )]
    TracePack {
        /// Base URL for the quality-service HTTP API
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        base_url: String,
        /// Directory where the trace pack will be written
        #[arg(long, default_value = ".")]
        output_dir: std::path::PathBuf,
        /// Number of recent log lines to collect per service
        #[arg(long, default_value_t = 200)]
        log_lines: u32,
        /// Maximum number of validation results to collect
        #[arg(long, default_value_t = 20)]
        results_limit: u32,
        /// Compress output as .tar.gz
        #[arg(long)]
        compress: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let format = if cli.json {
        OutputFormat::Json
    } else if cli.verbose {
        OutputFormat::HumanVerbose
    } else {
        OutputFormat::Human
    };

    // Results-inspect has its own report type
    if let Commands::ResultsInspect {
        ref base_url,
        ref scope_kind,
        ref scope_key,
        ref binding,
        ref topic,
        limit,
        failed_only,
        latest,
    } = cli.command
    {
        let config = results_inspect::InspectConfig {
            base_url: base_url.clone(),
            scope_kind: scope_kind.clone(),
            scope_key: scope_key.clone(),
            binding_name: binding.clone(),
            topic: topic.clone(),
            limit,
            failed_only,
            latest,
        };

        match results_inspect::run(&config) {
            Ok(report) => {
                let rendered = if cli.json {
                    match results_inspect::render_json(&report) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("error: failed to render output: {e}");
                            process::exit(2);
                        }
                    }
                } else {
                    results_inspect::render_human(&report, cli.verbose)
                };
                print!("{rendered}");
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
        return;
    }

    // Quality-gate has its own report type and renderer
    if let Commands::QualityGate {
        profile,
        ref base_url,
        fail_fast,
    } = cli.command
    {
        let gate_config = gate::GateConfig {
            project_root: cli.project_root.clone(),
            profile: profile.into(),
            base_url: base_url.clone(),
            fail_fast,
        };

        match gate::run(&gate_config) {
            Ok(gate_report) => {
                match gate::render(&gate_report, format) {
                    Ok(rendered) => print!("{rendered}"),
                    Err(e) => {
                        eprintln!("error: failed to render output: {e}");
                        process::exit(2);
                    }
                }
                if !gate_report.passed {
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
        return;
    }

    // Baseline drift has its own report type
    if let Commands::BaselineDrift { ref baseline } = cli.command {
        match analyzers::baseline_drift::analyze(baseline, &cli.project_root) {
            Ok(report) => {
                if cli.json {
                    match analyzers::baseline_drift::render_json(&report) {
                        Ok(json) => print!("{json}"),
                        Err(e) => {
                            eprintln!("error: failed to render output: {e}");
                            process::exit(2);
                        }
                    }
                } else {
                    print!(
                        "{}",
                        analyzers::baseline_drift::render_human(&report, cli.verbose)
                    );
                }
                if report.verdict == analyzers::baseline_drift::Verdict::Drifted {
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
        return;
    }

    // Snapshot has its own report type
    if let Commands::Snapshot { ref output } = cli.command {
        let snap = analyzers::snapshot::generate(&cli.project_root);

        if cli.json || output.is_some() {
            match analyzers::snapshot::render_json(&snap) {
                Ok(json) => {
                    if let Some(ref path) = output {
                        match std::fs::write(path, &json) {
                            Ok(_) => {
                                eprintln!("Snapshot written to {}", path.display());
                            }
                            Err(e) => {
                                eprintln!("error: failed to write snapshot: {e}");
                                process::exit(2);
                            }
                        }
                    } else {
                        print!("{json}");
                    }
                }
                Err(e) => {
                    eprintln!("error: failed to render snapshot: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", analyzers::snapshot::render_human(&snap, cli.verbose));
        }
        return;
    }

    // Snapshot diff has its own report type
    if let Commands::SnapshotDiff {
        ref before,
        ref after,
        after_live,
    } = cli.command
    {
        let before_snap = match analyzers::snapshot_diff::load_snapshot(before) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: failed to load 'before' snapshot: {e}");
                process::exit(2);
            }
        };

        let after_snap = if after_live {
            analyzers::snapshot::generate(&cli.project_root)
        } else {
            match after {
                Some(ref path) => match analyzers::snapshot_diff::load_snapshot(path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("error: failed to load 'after' snapshot: {e}");
                        process::exit(2);
                    }
                },
                None => {
                    eprintln!("error: 'after' snapshot path required (or use --after-live)");
                    process::exit(2);
                }
            }
        };

        match analyzers::snapshot_diff::diff(&before_snap, &after_snap) {
            Ok(d) => {
                if cli.json {
                    match analyzers::snapshot_diff::render_json(&d) {
                        Ok(json) => print!("{json}"),
                        Err(e) => {
                            eprintln!("error: failed to render diff: {e}");
                            process::exit(2);
                        }
                    }
                } else {
                    print!("{}", analyzers::snapshot_diff::render_human(&d, cli.verbose));
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
        return;
    }

    // Trace-pack has its own output (directory/tarball, not a Report)
    if let Commands::TracePack {
        ref base_url,
        ref output_dir,
        log_lines,
        results_limit,
        compress,
    } = cli.command
    {
        let config = trace_pack::TracePackConfig {
            project_root: cli.project_root.clone(),
            base_url: base_url.clone(),
            output_dir: output_dir.clone(),
            log_lines,
            results_limit,
            compress,
        };

        match trace_pack::run(&config) {
            Ok(report) => {
                let rendered = if cli.json {
                    match trace_pack::render_json(&report) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("error: failed to render output: {e}");
                            process::exit(2);
                        }
                    }
                } else {
                    trace_pack::render_human(&report)
                };
                print!("{rendered}");
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
        return;
    }

    // LSP enrich has its own report type
    if let Commands::LspEnrich {
        ref symbol,
        no_lsp,
        timeout: _,
    } = cli.command
    {
        let mut bridge = if no_lsp {
            lsp::GoplsBridge::unavailable("--no-lsp flag: LSP enrichment disabled by user")
        } else {
            lsp::GoplsBridge::new(&cli.project_root)
        };

        let enriched = bridge.enrich_symbol(&cli.project_root, symbol);

        if cli.json {
            match serde_json::to_string_pretty(&enriched) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", render_enriched_human(&enriched, cli.verbose));
        }
        return;
    }

    // Contract usage map has its own report type
    if let Commands::ContractUsageMap { lsp, no_lsp } = cli.command {
        let report = if lsp && !no_lsp {
            let mut bridge = lsp::GoplsBridge::new(&cli.project_root);
            let r = analyzers::contract_usage_map::analyze_with_lsp(&cli.project_root, &mut bridge);
            bridge.shutdown();
            r
        } else {
            analyzers::contract_usage_map::analyze(&cli.project_root)
        };

        if cli.json {
            match analyzers::contract_usage_map::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!(
                "{}",
                analyzers::contract_usage_map::render_human(&report, cli.verbose)
            );
        }
        return;
    }

    // Rename safety has its own report type
    if let Commands::RenameSafety {
        ref symbol,
        ref new_name,
        lsp,
        no_lsp,
    } = cli.command
    {
        let nn = new_name.as_deref();
        let report = if lsp && !no_lsp {
            let mut bridge = lsp::GoplsBridge::new(&cli.project_root);
            let r = analyzers::rename_safety::check_with_lsp(
                &cli.project_root,
                symbol,
                nn,
                &mut bridge,
            );
            bridge.shutdown();
            r
        } else {
            analyzers::rename_safety::check(&cli.project_root, symbol, nn)
        };

        if cli.json {
            match analyzers::rename_safety::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!(
                "{}",
                analyzers::rename_safety::render_human(&report, cli.verbose)
            );
        }
        return;
    }

    // Symbol trace has its own report type
    if let Commands::SymbolTrace { ref symbol, lsp, no_lsp } = cli.command {
        let report = if lsp && !no_lsp {
            let mut bridge = lsp::GoplsBridge::new(&cli.project_root);
            let r = analyzers::symbol_trace::trace_with_lsp(&cli.project_root, symbol, &mut bridge);
            bridge.shutdown();
            r
        } else {
            analyzers::symbol_trace::trace(&cli.project_root, symbol)
        };

        if cli.json {
            match analyzers::symbol_trace::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", analyzers::symbol_trace::render_human(&report, cli.verbose));
        }
        return;
    }

    // Briefing has its own report type
    if let Commands::Briefing { ref targets, lsp, no_lsp } = cli.command {
        let changed = if targets.is_empty() {
            detect_changed_files(&cli.project_root)
        } else {
            targets.clone()
        };

        let report = if lsp && !no_lsp {
            let mut bridge = lsp::GoplsBridge::new(&cli.project_root);
            let r = analyzers::briefing::analyze_with_lsp(&cli.project_root, &changed, &mut bridge);
            bridge.shutdown();
            r
        } else {
            analyzers::briefing::analyze(&cli.project_root, &changed)
        };

        if cli.json {
            match analyzers::briefing::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", analyzers::briefing::render_human(&report, cli.verbose));
        }
        return;
    }

    // Impact map has its own report type
    if let Commands::ImpactMap { ref targets, lsp, no_lsp } = cli.command {
        let changed = if targets.is_empty() {
            detect_changed_files(&cli.project_root)
        } else {
            targets.clone()
        };

        let report = if lsp && !no_lsp {
            let mut bridge = lsp::GoplsBridge::new(&cli.project_root);
            let r = analyzers::impact_map::analyze_with_lsp(&cli.project_root, &changed, &mut bridge);
            bridge.shutdown();
            r
        } else {
            analyzers::impact_map::analyze(&cli.project_root, &changed)
        };

        if cli.json {
            match analyzers::impact_map::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", analyzers::impact_map::render_human(&report, cli.verbose));
        }
        return;
    }

    // TDD flow helper — impact-driven guidance
    if let Commands::Tdd { ref files } = cli.command {
        let changed = if files.is_empty() {
            detect_changed_files(&cli.project_root)
        } else {
            files.clone()
        };

        let report = analyzers::tdd::analyze(&cli.project_root, &changed);

        if cli.json {
            match analyzers::tdd::render_json(&report) {
                Ok(s) => print!("{s}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
        } else {
            print!("{}", analyzers::tdd::render_human(&report, cli.verbose));
        }
        return;
    }

    // Scenario-smoke has its own dispatch logic
    if let Commands::ScenarioSmoke {
        ref scenario,
        ref base_url,
        list,
    } = cli.command
    {
        if list {
            let scenarios = smoke::scenarios::list_scenarios();
            if cli.json {
                let items: Vec<serde_json::Value> = scenarios
                    .iter()
                    .map(|(name, desc)| {
                        serde_json::json!({
                            "name": name,
                            "description": desc,
                            "preconditions": smoke::scenarios::Scenario::parse(name)
                                .map(|s| s.preconditions())
                                .unwrap_or(&[]),
                        })
                    })
                    .collect();
                let json = serde_json::json!({ "scenarios": items });
                match serde_json::to_string_pretty(&json) {
                    Ok(s) => print!("{s}"),
                    Err(e) => {
                        eprintln!("error: failed to render output: {e}");
                        process::exit(2);
                    }
                }
            } else {
                println!("Available scenarios:\n");
                for (name, desc) in &scenarios {
                    println!("  {name:<20} {desc}");
                }
                println!("\nUsage: raccoon-cli scenario-smoke <SCENARIO>");
            }
            return;
        }

        let scenario_name = scenario.as_deref().unwrap_or("");
        let scenario = match smoke::scenarios::Scenario::parse(scenario_name) {
            Some(s) => s,
            None => {
                eprintln!(
                    "error: unknown scenario '{scenario_name}'. Available: {}",
                    smoke::scenarios::Scenario::all_names().join(", ")
                );
                process::exit(2);
            }
        };

        let smoke_config = smoke::SmokeConfig::new(&cli.project_root, Some(base_url));
        let report = smoke::scenarios::run_scenario(scenario, &smoke_config);

        match output::render(&report, format) {
            Ok(rendered) => print!("{rendered}"),
            Err(e) => {
                eprintln!("error: failed to render output: {e}");
                process::exit(2);
            }
        }
        if !report.passed() {
            process::exit(1);
        }
        return;
    }

    let result = match cli.command {
        Commands::Doctor => analyzers::doctor::analyze(&cli.project_root),
        Commands::TopologyDoctor => analyzers::topology::analyze(&cli.project_root),
        Commands::ContractAudit => analyzers::contracts::analyze(&cli.project_root),
        Commands::RuntimeBindings => analyzers::runtime_bindings::analyze(&cli.project_root),
        Commands::DriftDetect => analyzers::drift_detect::analyze(&cli.project_root),
        Commands::ArchGuard => analyzers::arch_guard::analyze(&cli.project_root),
        Commands::CoverageMap => analyzers::coverage_map::analyze(&cli.project_root),
        Commands::RuntimeSmoke { ref base_url } => {
            let config = smoke::SmokeConfig::new(&cli.project_root, Some(base_url));
            smoke::run(&config)
        }
        Commands::ResultsInspect { .. } => unreachable!(),
        Commands::QualityGate { .. } => unreachable!(),
        Commands::BaselineDrift { .. } => unreachable!(),
        Commands::Snapshot { .. } => unreachable!(),
        Commands::SnapshotDiff { .. } => unreachable!(),
        Commands::TracePack { .. } => unreachable!(),
        Commands::ScenarioSmoke { .. } => unreachable!(),
        Commands::ImpactMap { .. } => unreachable!(),
        Commands::SymbolTrace { .. } => unreachable!(),
        Commands::Tdd { .. } => unreachable!(),
        Commands::LspEnrich { .. } => unreachable!(),
        Commands::RenameSafety { .. } => unreachable!(),
        Commands::ContractUsageMap { .. } => unreachable!(),
        Commands::Briefing { .. } => unreachable!(),
    };

    match result {
        Ok(report) => {
            match output::render(&report, format) {
                Ok(rendered) => print!("{rendered}"),
                Err(e) => {
                    eprintln!("error: failed to render output: {e}");
                    process::exit(2);
                }
            }
            if !report.passed() {
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(2);
        }
    }
}

/// Render an enriched symbol report in human-readable format.
fn render_enriched_human(enriched: &lsp::EnrichedSymbol, verbose: bool) -> String {
    use lsp::types::LspStatus;

    let mut out = String::new();
    out.push_str(&format!("Symbol: {}\n\n", enriched.symbol));

    // LSP status
    match &enriched.lsp_status {
        LspStatus::Enriched => out.push_str("LSP: enriched (gopls connected)\n"),
        LspStatus::NoResults => out.push_str("LSP: connected but no additional results\n"),
        LspStatus::Unavailable { reason } => {
            out.push_str(&format!("LSP: unavailable ({reason})\n"));
        }
    }
    out.push('\n');

    // AST definitions
    if enriched.ast_definitions.is_empty() {
        out.push_str("AST definitions: none found\n");
    } else {
        out.push_str(&format!(
            "AST definitions ({}):\n",
            enriched.ast_definitions.len()
        ));
        for def in &enriched.ast_definitions {
            let name = def.qualified_name.as_deref().unwrap_or(&enriched.symbol);
            out.push_str(&format!("  {} at {}:{}\n", name, def.location.file, def.location.line));
        }
    }

    // LSP definitions
    if !enriched.lsp_definitions.is_empty() {
        out.push_str(&format!(
            "\nLSP definitions ({}):\n",
            enriched.lsp_definitions.len()
        ));
        for def in &enriched.lsp_definitions {
            out.push_str(&format!("  {}:{}\n", def.location.file, def.location.line));
        }
    }

    // LSP references
    if !enriched.lsp_references.is_empty() {
        out.push_str(&format!(
            "\nLSP references ({}):\n",
            enriched.lsp_references.len()
        ));
        let limit = if verbose { enriched.lsp_references.len() } else { 10 };
        for r in enriched.lsp_references.iter().take(limit) {
            out.push_str(&format!("  {}:{}\n", r.location.file, r.location.line));
        }
        if !verbose && enriched.lsp_references.len() > 10 {
            out.push_str(&format!(
                "  ... and {} more (use -v to show all)\n",
                enriched.lsp_references.len() - 10
            ));
        }
    }

    // Hover info
    if let Some(ref hover) = enriched.hover {
        out.push('\n');
        if let Some(ref sig) = hover.signature {
            out.push_str(&format!("Type: {sig}\n"));
        }
        if verbose {
            if let Some(ref doc) = hover.documentation {
                out.push_str(&format!("Doc: {doc}\n"));
            }
        }
    }

    out
}

/// Detect changed files using git status (unstaged + staged).
fn detect_changed_files(project_root: &std::path::Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "-u"])
        .current_dir(project_root)
        .output();
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .filter_map(|line| {
                    if line.len() > 3 {
                        Some(line[3..].trim().to_string())
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parses_doctor() {
        let cli = Cli::try_parse_from(["raccoon", "doctor"]).unwrap();
        assert!(!cli.json);
        assert!(!cli.verbose);
        assert!(matches!(cli.command, Commands::Doctor));
    }

    #[test]
    fn cli_parses_topology_doctor() {
        let cli = Cli::try_parse_from(["raccoon", "topology-doctor"]).unwrap();
        assert!(matches!(cli.command, Commands::TopologyDoctor));
    }

    #[test]
    fn cli_parses_json_flag() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "doctor"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn cli_parses_verbose_long() {
        let cli = Cli::try_parse_from(["raccoon", "--verbose", "doctor"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn cli_parses_verbose_short() {
        let cli = Cli::try_parse_from(["raccoon", "-v", "doctor"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn cli_parses_lsp_enrich() {
        let cli = Cli::try_parse_from(["raccoon", "lsp-enrich", "ConfigSet"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::LspEnrich { ref symbol, no_lsp, .. } if symbol == "ConfigSet" && !no_lsp
        ));
    }

    #[test]
    fn cli_parses_lsp_enrich_no_lsp() {
        let cli = Cli::try_parse_from(["raccoon", "lsp-enrich", "--no-lsp", "Foo"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::LspEnrich { no_lsp: true, .. }
        ));
    }

    #[test]
    fn cli_parses_lsp_enrich_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "lsp-enrich", "Bar"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::LspEnrich { .. }));
    }

    #[test]
    fn cli_parses_project_root() {
        let cli = Cli::try_parse_from(["raccoon", "--project-root", "/tmp", "doctor"]).unwrap();
        assert_eq!(cli.project_root, std::path::PathBuf::from("/tmp"));
    }

    #[test]
    fn cli_parses_contract_audit() {
        let cli = Cli::try_parse_from(["raccoon", "contract-audit"]).unwrap();
        assert!(matches!(cli.command, Commands::ContractAudit));
    }

    #[test]
    fn cli_parses_runtime_bindings() {
        let cli = Cli::try_parse_from(["raccoon", "runtime-bindings"]).unwrap();
        assert!(matches!(cli.command, Commands::RuntimeBindings));
    }

    #[test]
    fn cli_parses_runtime_bindings_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "runtime-bindings"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::RuntimeBindings));
    }

    #[test]
    fn cli_parses_runtime_smoke() {
        let cli = Cli::try_parse_from(["raccoon", "runtime-smoke"]).unwrap();
        assert!(matches!(cli.command, Commands::RuntimeSmoke { .. }));
    }

    #[test]
    fn cli_parses_runtime_smoke_with_base_url() {
        let cli = Cli::try_parse_from([
            "raccoon",
            "runtime-smoke",
            "--base-url",
            "http://localhost:9090",
        ])
        .unwrap();
        match cli.command {
            Commands::RuntimeSmoke { ref base_url } => {
                assert_eq!(base_url, "http://localhost:9090");
            }
            _ => panic!("expected RuntimeSmoke"),
        }
    }

    #[test]
    fn cli_parses_runtime_smoke_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "runtime-smoke"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::RuntimeSmoke { .. }));
    }

    #[test]
    fn cli_parses_quality_gate_default() {
        let cli = Cli::try_parse_from(["raccoon-cli", "quality-gate"]).unwrap();
        match cli.command {
            Commands::QualityGate { profile, .. } => {
                assert_eq!(profile, GateProfile::Fast);
            }
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_parses_quality_gate_fast() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "quality-gate", "--profile", "fast"]).unwrap();
        match cli.command {
            Commands::QualityGate { profile, .. } => assert_eq!(profile, GateProfile::Fast),
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_parses_quality_gate_ci() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "quality-gate", "--profile", "ci"]).unwrap();
        match cli.command {
            Commands::QualityGate { profile, .. } => assert_eq!(profile, GateProfile::Ci),
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_parses_quality_gate_deep() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "quality-gate", "--profile", "deep"]).unwrap();
        match cli.command {
            Commands::QualityGate { profile, .. } => assert_eq!(profile, GateProfile::Deep),
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_rejects_quality_gate_invalid_profile() {
        assert!(
            Cli::try_parse_from(["raccoon-cli", "quality-gate", "--profile", "turbo"]).is_err()
        );
    }

    #[test]
    fn cli_parses_quality_gate_json() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "--json",
            "quality-gate",
            "--profile",
            "fast",
        ])
        .unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::QualityGate { .. }));
    }

    #[test]
    fn cli_parses_drift_detect() {
        let cli = Cli::try_parse_from(["raccoon", "drift-detect"]).unwrap();
        assert!(matches!(cli.command, Commands::DriftDetect));
    }

    #[test]
    fn cli_parses_drift_detect_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "drift-detect"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::DriftDetect));
    }

    #[test]
    fn cli_parses_arch_guard() {
        let cli = Cli::try_parse_from(["raccoon", "arch-guard"]).unwrap();
        assert!(matches!(cli.command, Commands::ArchGuard));
    }

    #[test]
    fn cli_parses_arch_guard_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "arch-guard"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::ArchGuard));
    }

    #[test]
    fn cli_parses_symbol_trace() {
        let cli = Cli::try_parse_from(["raccoon", "symbol-trace", "ConfigSet"]).unwrap();
        match cli.command {
            Commands::SymbolTrace { ref symbol, lsp, no_lsp } => {
                assert_eq!(symbol, "ConfigSet");
                assert!(!lsp);
                assert!(!no_lsp);
            }
            _ => panic!("expected SymbolTrace"),
        }
    }

    #[test]
    fn cli_parses_symbol_trace_with_lsp() {
        let cli = Cli::try_parse_from(["raccoon", "symbol-trace", "--lsp", "ConfigSet"]).unwrap();
        match cli.command {
            Commands::SymbolTrace { ref symbol, lsp, no_lsp } => {
                assert_eq!(symbol, "ConfigSet");
                assert!(lsp);
                assert!(!no_lsp);
            }
            _ => panic!("expected SymbolTrace"),
        }
    }

    #[test]
    fn cli_parses_symbol_trace_json() {
        let cli = Cli::try_parse_from(["raccoon", "--json", "symbol-trace", "Foo"]).unwrap();
        assert!(cli.json);
        match cli.command {
            Commands::SymbolTrace { ref symbol, .. } => assert_eq!(symbol, "Foo"),
            _ => panic!("expected SymbolTrace"),
        }
    }

    #[test]
    fn cli_symbol_trace_requires_symbol() {
        assert!(Cli::try_parse_from(["raccoon", "symbol-trace"]).is_err());
    }

    #[test]
    fn cli_rejects_unknown_command() {
        assert!(Cli::try_parse_from(["raccoon", "foobar"]).is_err());
    }

    #[test]
    fn cli_project_root_defaults_to_current_dir() {
        let cli = Cli::try_parse_from(["raccoon-cli", "doctor"]).unwrap();
        assert_eq!(cli.project_root, std::path::PathBuf::from("."));
    }

    #[test]
    fn cli_json_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "topology-doctor"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::TopologyDoctor));
    }

    #[test]
    fn cli_quality_gate_base_url_with_deep() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "quality-gate",
            "--profile",
            "deep",
            "--base-url",
            "http://localhost:9090",
        ])
        .unwrap();
        match cli.command {
            Commands::QualityGate { profile, base_url, fail_fast } => {
                assert_eq!(profile, GateProfile::Deep);
                assert_eq!(base_url, "http://localhost:9090");
                assert!(!fail_fast);
            }
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_parses_quality_gate_fail_fast() {
        let cli = Cli::try_parse_from(["raccoon-cli", "quality-gate", "--fail-fast"]).unwrap();
        match cli.command {
            Commands::QualityGate { fail_fast, .. } => assert!(fail_fast),
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn cli_quality_gate_fail_fast_default_is_false() {
        let cli = Cli::try_parse_from(["raccoon-cli", "quality-gate"]).unwrap();
        match cli.command {
            Commands::QualityGate { fail_fast, .. } => assert!(!fail_fast),
            _ => panic!("expected QualityGate"),
        }
    }

    #[test]
    fn json_and_verbose_are_independent() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "-v", "doctor"]).unwrap();
        assert!(cli.json);
        assert!(cli.verbose);
    }

    #[test]
    fn verbose_flag_global_with_quality_gate() {
        let cli = Cli::try_parse_from(["raccoon-cli", "-v", "quality-gate"]).unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::QualityGate { .. }));
    }

    #[test]
    fn gate_profile_from_converts_correctly() {
        assert_eq!(gate::Profile::from(GateProfile::Fast), gate::Profile::Fast);
        assert_eq!(gate::Profile::from(GateProfile::Ci), gate::Profile::Ci);
        assert_eq!(gate::Profile::from(GateProfile::Deep), gate::Profile::Deep);
    }

    // ── snapshot parsing ──────────────────────────────────────

    #[test]
    fn cli_parses_snapshot() {
        let cli = Cli::try_parse_from(["raccoon-cli", "snapshot"]).unwrap();
        assert!(matches!(cli.command, Commands::Snapshot { output: None }));
    }

    #[test]
    fn cli_parses_snapshot_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "snapshot"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::Snapshot { .. }));
    }

    #[test]
    fn cli_parses_snapshot_with_output() {
        let cli = Cli::try_parse_from(["raccoon-cli", "snapshot", "--output", "snap.json"]).unwrap();
        match cli.command {
            Commands::Snapshot { output } => {
                assert_eq!(output, Some(std::path::PathBuf::from("snap.json")));
            }
            _ => panic!("expected Snapshot"),
        }
    }

    #[test]
    fn cli_parses_snapshot_with_short_output() {
        let cli = Cli::try_parse_from(["raccoon-cli", "snapshot", "-o", "out.json"]).unwrap();
        match cli.command {
            Commands::Snapshot { output } => {
                assert_eq!(output, Some(std::path::PathBuf::from("out.json")));
            }
            _ => panic!("expected Snapshot"),
        }
    }

    // ── results-inspect parsing ──────────────────────────────────

    // ── trace-pack parsing ──────────────────────────────────────

    #[test]
    fn cli_parses_trace_pack_defaults() {
        let cli = Cli::try_parse_from(["raccoon-cli", "trace-pack"]).unwrap();
        match cli.command {
            Commands::TracePack {
                base_url,
                output_dir,
                log_lines,
                results_limit,
                compress,
            } => {
                assert_eq!(base_url, "http://127.0.0.1:8080");
                assert_eq!(output_dir, std::path::PathBuf::from("."));
                assert_eq!(log_lines, 200);
                assert_eq!(results_limit, 20);
                assert!(!compress);
            }
            _ => panic!("expected TracePack"),
        }
    }

    #[test]
    fn cli_parses_trace_pack_with_all_flags() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "trace-pack",
            "--base-url",
            "http://localhost:9090",
            "--output-dir",
            "/tmp/traces",
            "--log-lines",
            "500",
            "--results-limit",
            "50",
            "--compress",
        ])
        .unwrap();
        match cli.command {
            Commands::TracePack {
                base_url,
                output_dir,
                log_lines,
                results_limit,
                compress,
            } => {
                assert_eq!(base_url, "http://localhost:9090");
                assert_eq!(output_dir, std::path::PathBuf::from("/tmp/traces"));
                assert_eq!(log_lines, 500);
                assert_eq!(results_limit, 50);
                assert!(compress);
            }
            _ => panic!("expected TracePack"),
        }
    }

    #[test]
    fn cli_parses_trace_pack_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "trace-pack"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::TracePack { .. }));
    }

    // ── results-inspect parsing ──────────────────────────────────

    #[test]
    fn cli_parses_results_inspect_defaults() {
        let cli = Cli::try_parse_from(["raccoon-cli", "results-inspect"]).unwrap();
        match cli.command {
            Commands::ResultsInspect {
                base_url,
                scope_kind,
                scope_key,
                binding,
                topic,
                limit,
                failed_only,
                latest,
            } => {
                assert_eq!(base_url, "http://127.0.0.1:8080");
                assert_eq!(scope_kind, "global");
                assert_eq!(scope_key, "default");
                assert!(binding.is_none());
                assert!(topic.is_none());
                assert_eq!(limit, 100);
                assert!(!failed_only);
                assert!(latest.is_none());
            }
            _ => panic!("expected ResultsInspect"),
        }
    }

    #[test]
    fn cli_parses_results_inspect_with_all_flags() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "results-inspect",
            "--base-url",
            "http://localhost:9090",
            "--scope-kind",
            "tenant",
            "--scope-key",
            "br",
            "--binding",
            "orders",
            "--topic",
            "orders.v1",
            "--limit",
            "50",
            "--failed-only",
            "--latest",
            "10",
        ])
        .unwrap();
        match cli.command {
            Commands::ResultsInspect {
                base_url,
                scope_kind,
                scope_key,
                binding,
                topic,
                limit,
                failed_only,
                latest,
            } => {
                assert_eq!(base_url, "http://localhost:9090");
                assert_eq!(scope_kind, "tenant");
                assert_eq!(scope_key, "br");
                assert_eq!(binding.as_deref(), Some("orders"));
                assert_eq!(topic.as_deref(), Some("orders.v1"));
                assert_eq!(limit, 50);
                assert!(failed_only);
                assert_eq!(latest, Some(10));
            }
            _ => panic!("expected ResultsInspect"),
        }
    }

    #[test]
    fn cli_parses_results_inspect_json() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "--json", "results-inspect"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::ResultsInspect { .. }));
    }

    // ── scenario-smoke parsing ──────────────────────────────────

    #[test]
    fn cli_parses_scenario_smoke_with_scenario() {
        let cli = Cli::try_parse_from(["raccoon-cli", "scenario-smoke", "happy-path"]).unwrap();
        match cli.command {
            Commands::ScenarioSmoke { scenario, base_url, list } => {
                assert_eq!(scenario.as_deref(), Some("happy-path"));
                assert_eq!(base_url, "http://127.0.0.1:8080");
                assert!(!list);
            }
            _ => panic!("expected ScenarioSmoke"),
        }
    }

    #[test]
    fn cli_parses_scenario_smoke_list() {
        let cli = Cli::try_parse_from(["raccoon-cli", "scenario-smoke", "--list"]).unwrap();
        match cli.command {
            Commands::ScenarioSmoke { scenario, list, .. } => {
                assert!(list);
                assert!(scenario.is_none());
            }
            _ => panic!("expected ScenarioSmoke"),
        }
    }

    #[test]
    fn cli_parses_scenario_smoke_with_base_url() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "scenario-smoke",
            "--base-url",
            "http://localhost:9090",
            "config-lifecycle",
        ])
        .unwrap();
        match cli.command {
            Commands::ScenarioSmoke { scenario, base_url, .. } => {
                assert_eq!(scenario.as_deref(), Some("config-lifecycle"));
                assert_eq!(base_url, "http://localhost:9090");
            }
            _ => panic!("expected ScenarioSmoke"),
        }
    }

    #[test]
    fn cli_parses_scenario_smoke_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "scenario-smoke", "happy-path"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::ScenarioSmoke { .. }));
    }

    #[test]
    fn cli_scenario_smoke_requires_scenario_or_list() {
        assert!(Cli::try_parse_from(["raccoon-cli", "scenario-smoke"]).is_err());
    }

    // ── rename-safety parsing ──────────────────────────────────

    #[test]
    fn cli_parses_rename_safety() {
        let cli = Cli::try_parse_from(["raccoon-cli", "rename-safety", "ConfigSet"]).unwrap();
        match cli.command {
            Commands::RenameSafety {
                ref symbol,
                ref new_name,
                lsp,
                no_lsp,
            } => {
                assert_eq!(symbol, "ConfigSet");
                assert!(new_name.is_none());
                assert!(!lsp);
                assert!(!no_lsp);
            }
            _ => panic!("expected RenameSafety"),
        }
    }

    #[test]
    fn cli_parses_rename_safety_with_new_name() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "rename-safety",
            "ConfigSet",
            "--to",
            "QualityConfigSet",
        ])
        .unwrap();
        match cli.command {
            Commands::RenameSafety {
                ref symbol,
                ref new_name,
                ..
            } => {
                assert_eq!(symbol, "ConfigSet");
                assert_eq!(new_name.as_deref(), Some("QualityConfigSet"));
            }
            _ => panic!("expected RenameSafety"),
        }
    }

    #[test]
    fn cli_parses_rename_safety_with_lsp() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "rename-safety", "--lsp", "ConfigSet"]).unwrap();
        match cli.command {
            Commands::RenameSafety { lsp, no_lsp, .. } => {
                assert!(lsp);
                assert!(!no_lsp);
            }
            _ => panic!("expected RenameSafety"),
        }
    }

    #[test]
    fn cli_parses_rename_safety_json() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "--json", "rename-safety", "Foo"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::RenameSafety { .. }));
    }

    #[test]
    fn cli_rename_safety_requires_symbol() {
        assert!(Cli::try_parse_from(["raccoon-cli", "rename-safety"]).is_err());
    }

    #[test]
    fn cli_parses_results_inspect_verbose() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "-v", "results-inspect"]).unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::ResultsInspect { .. }));
    }

    #[test]
    fn cli_parses_results_inspect_failed_only() {
        let cli =
            Cli::try_parse_from(["raccoon-cli", "results-inspect", "--failed-only"]).unwrap();
        match cli.command {
            Commands::ResultsInspect { failed_only, .. } => assert!(failed_only),
            _ => panic!("expected ResultsInspect"),
        }
    }

    // ── contract-usage-map parsing ──────────────────────────────────

    #[test]
    fn cli_parses_contract_usage_map() {
        let cli = Cli::try_parse_from(["raccoon-cli", "contract-usage-map"]).unwrap();
        assert!(matches!(cli.command, Commands::ContractUsageMap { lsp: false, no_lsp: false }));
    }

    #[test]
    fn cli_parses_contract_usage_map_with_lsp() {
        let cli = Cli::try_parse_from(["raccoon-cli", "contract-usage-map", "--lsp"]).unwrap();
        match cli.command {
            Commands::ContractUsageMap { lsp, no_lsp } => {
                assert!(lsp);
                assert!(!no_lsp);
            }
            _ => panic!("expected ContractUsageMap"),
        }
    }

    #[test]
    fn cli_parses_contract_usage_map_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "contract-usage-map"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::ContractUsageMap { .. }));
    }

    #[test]
    fn cli_parses_contract_usage_map_verbose() {
        let cli = Cli::try_parse_from(["raccoon-cli", "-v", "contract-usage-map"]).unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::ContractUsageMap { .. }));
    }

    // ── coverage-map parsing ──────────────────────────────────────

    #[test]
    fn cli_parses_coverage_map() {
        let cli = Cli::try_parse_from(["raccoon-cli", "coverage-map"]).unwrap();
        assert!(matches!(cli.command, Commands::CoverageMap));
    }

    #[test]
    fn cli_parses_coverage_map_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "coverage-map"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::CoverageMap));
    }

    #[test]
    fn cli_parses_coverage_map_verbose() {
        let cli = Cli::try_parse_from(["raccoon-cli", "-v", "coverage-map"]).unwrap();
        assert!(cli.verbose);
        assert!(matches!(cli.command, Commands::CoverageMap));
    }

    // ── tdd parsing ──────────────────────────────────────────────

    #[test]
    fn cli_parses_tdd_no_files() {
        let cli = Cli::try_parse_from(["raccoon-cli", "tdd"]).unwrap();
        match cli.command {
            Commands::Tdd { files } => assert!(files.is_empty()),
            _ => panic!("expected Tdd"),
        }
    }

    #[test]
    fn cli_parses_tdd_with_files() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "tdd",
            "internal/adapters/nats/codec.go",
            "deploy/configs/consumer.jsonc",
        ])
        .unwrap();
        match cli.command {
            Commands::Tdd { files } => {
                assert_eq!(files.len(), 2);
                assert_eq!(files[0], "internal/adapters/nats/codec.go");
                assert_eq!(files[1], "deploy/configs/consumer.jsonc");
            }
            _ => panic!("expected Tdd"),
        }
    }

    #[test]
    fn cli_parses_tdd_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "tdd", "file.go"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::Tdd { .. }));
    }

    #[test]
    fn cli_parses_impact_map_no_targets() {
        let cli = Cli::try_parse_from(["raccoon-cli", "impact-map"]).unwrap();
        match cli.command {
            Commands::ImpactMap { targets, .. } => assert!(targets.is_empty()),
            _ => panic!("expected ImpactMap"),
        }
    }

    #[test]
    fn cli_parses_impact_map_with_targets() {
        let cli = Cli::try_parse_from([
            "raccoon-cli",
            "impact-map",
            "internal/domain/configctl/config.go",
            "ConfigSet",
        ])
        .unwrap();
        match cli.command {
            Commands::ImpactMap { targets, .. } => {
                assert_eq!(targets.len(), 2);
                assert_eq!(targets[0], "internal/domain/configctl/config.go");
                assert_eq!(targets[1], "ConfigSet");
            }
            _ => panic!("expected ImpactMap"),
        }
    }

    #[test]
    fn cli_parses_impact_map_json() {
        let cli = Cli::try_parse_from(["raccoon-cli", "--json", "impact-map", "file.go"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::ImpactMap { .. }));
    }

    #[test]
    fn cli_parses_impact_map_with_lsp() {
        let cli = Cli::try_parse_from(["raccoon-cli", "impact-map", "--lsp", "ConfigSet"]).unwrap();
        match cli.command {
            Commands::ImpactMap { lsp, no_lsp, targets } => {
                assert!(lsp);
                assert!(!no_lsp);
                assert_eq!(targets, vec!["ConfigSet"]);
            }
            _ => panic!("expected ImpactMap"),
        }
    }
}

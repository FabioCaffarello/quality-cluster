
SHELL := /usr/bin/env bash

GO ?= go
DOCKER ?= docker
COMPOSE_FILE ?= deploy/compose/docker-compose.yaml
COMPOSE := $(DOCKER) compose -f $(COMPOSE_FILE)
BUILD_DIR ?= bin
BUILDABLE_SERVICES := configctl server validator consumer emulator

RACCOON_DIR := tools/raccoon-cli
RACCOON_BIN := $(RACCOON_DIR)/target/release/raccoon-cli

.DEFAULT_GOAL := help

define RUN_IN_MODULES
	@MODULE='$(MODULE)' ./scripts/utils/for-each-module.sh $(1)
endef

.PHONY: help tidy test build docker-build compose-config up-core up-runtime up-dataplane up-all down restart logs ps clean \
       raccoon-build raccoon-test quality-gate quality-gate-ci quality-gate-deep \
       check check-deep verify smoke scenario-smoke trace-pack results-inspect \
       coverage-map tdd arch-guard drift-detect snapshot

help:
	@echo "Targets:"
	@echo "  make tidy                 - run go mod tidy in workspace modules"
	@echo "  make test                 - run go test ./... in workspace modules"
	@echo "  make build                - build local service binaries into $(BUILD_DIR)/"
	@echo "  make docker-build         - build docker images for local services"
	@echo "  make compose-config       - render and validate the compose file"
	@echo "  make up-core              - start nats + configctl + server"
	@echo "  make up-runtime           - start core stack plus validator"
	@echo "  make up-dataplane         - start core + runtime + kafka + consumer + emulator"
	@echo "  make up-all               - start every currently supported compose profile"
	@echo "  make down                 - stop the compose stack"
	@echo "  make restart              - restart the whole stack or SERVICE=<name>"
	@echo "  make logs                 - stream logs for the whole stack or SERVICE=<name>"
	@echo "  make ps                   - show compose service status"
	@echo "  make clean                - remove local build artifacts and Go caches"
	@echo ""
	@echo "Workflow (recommended):"
	@echo "  make check                - pre-code guard rail (quality-gate fast)"
	@echo "  make verify               - post-change: Go tests + quality-gate"
	@echo "  make check-deep           - full validation (requires make up-dataplane)"
	@echo "  make smoke                - e2e smoke test against live environment"
	@echo "  make scenario-smoke       - named scenario smoke (SCENARIO=happy-path)"
	@echo "  make trace-pack           - collect diagnostic evidence from cluster"
	@echo "  make results-inspect      - inspect validation results from validator"
	@echo "  make coverage-map         - show quality coverage map and gaps"
	@echo "  make tdd                  - TDD guide: what to validate for your changes"
	@echo "  make arch-guard           - architecture layer boundary check"
	@echo "  make drift-detect         - cross-layer drift detection"
	@echo "  make snapshot             - golden snapshot of code intelligence (JSON)"
	@echo ""
	@echo "Quality (raccoon-cli):"
	@echo "  make quality-gate         - fast static checks (local dev, pre-commit)"
	@echo "  make quality-gate-ci      - CI pipeline checks (JSON output)"
	@echo "  make quality-gate-deep    - full validation (requires make up-dataplane)"
	@echo "  make raccoon-build        - build raccoon-cli release binary"
	@echo "  make raccoon-test         - run raccoon-cli tests"
	@echo ""
	@echo "Optional:"
	@echo "  MODULE=./internal/shared  - scope tidy/test to one Go module"
	@echo "  SERVICE=server            - scope build/docker-build/logs/restart to one service"

tidy:
	$(call RUN_IN_MODULES,$(GO) mod tidy)

test:
	@modules=(); \
	if [[ -n "$(MODULE)" ]]; then \
		modules+=("$(MODULE)"); \
	else \
		while IFS= read -r module; do \
			modules+=("$$module"); \
		done < <(./scripts/utils/list-modules.sh); \
	fi; \
	for module in "$${modules[@]}"; do \
		[[ -z "$$module" ]] && continue; \
		echo ">>> $$module: $(GO) test ./..."; \
		packages="$$(cd "$$module" && $(GO) list ./... 2>/dev/null || true)"; \
		if [[ -z "$$packages" ]]; then \
			echo "no packages to test"; \
			continue; \
		fi; \
		(cd "$$module" && $(GO) test $$packages); \
	done

build:
	@mkdir -p $(BUILD_DIR)
	@if [[ -n "$(SERVICE)" ]]; then \
		case " $(BUILDABLE_SERVICES) " in \
			*" $(SERVICE) "*) ;; \
			*) echo "unsupported SERVICE=$(SERVICE). Supported: $(BUILDABLE_SERVICES)" >&2; exit 1 ;; \
		esac; \
		echo ">>> $(SERVICE)"; \
		$(GO) build -o $(BUILD_DIR)/$(SERVICE) ./cmd/$(SERVICE); \
	else \
		for service in $(BUILDABLE_SERVICES); do \
			echo ">>> $$service"; \
			$(GO) build -o $(BUILD_DIR)/$$service ./cmd/$$service; \
		done; \
	fi

docker-build:
	@if [[ -n "$(SERVICE)" ]]; then \
		case " $(BUILDABLE_SERVICES) " in \
			*" $(SERVICE) "*) ;; \
			*) echo "unsupported SERVICE=$(SERVICE). Supported: $(BUILDABLE_SERVICES)" >&2; exit 1 ;; \
		esac; \
		$(COMPOSE) build $(SERVICE); \
	else \
		$(COMPOSE) build $(BUILDABLE_SERVICES); \
	fi

compose-config:
	@$(COMPOSE) config > /dev/null
	@echo "compose config is valid"

up-core:
	$(COMPOSE) --profile core up -d --build

up-runtime:
	$(COMPOSE) --profile core --profile runtime up -d --build

up-dataplane:
	$(COMPOSE) --profile core --profile runtime --profile dataplane up -d --build

up-all:
	$(COMPOSE) --profile all up -d --build

down:
	$(COMPOSE) --profile all down --remove-orphans

restart:
	@if [[ -n "$(SERVICE)" ]]; then \
		$(COMPOSE) restart $(SERVICE); \
	else \
		$(COMPOSE) restart; \
	fi

logs:
	@if [[ -n "$(SERVICE)" ]]; then \
		$(COMPOSE) logs -f --tail=200 $(SERVICE); \
	else \
		$(COMPOSE) logs -f --tail=200; \
	fi

ps:
	$(COMPOSE) ps

clean:
	rm -rf $(BUILD_DIR)
	$(GO) clean -cache -testcache

# --- raccoon-cli (quality tooling) ---

$(RACCOON_BIN): $(shell find $(RACCOON_DIR)/src -type f -name '*.rs' 2>/dev/null) $(RACCOON_DIR)/Cargo.toml
	cargo build --release --manifest-path $(RACCOON_DIR)/Cargo.toml

raccoon-build: $(RACCOON_BIN)

raccoon-test:
	cargo test --manifest-path $(RACCOON_DIR)/Cargo.toml

quality-gate: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . quality-gate

quality-gate-ci: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . quality-gate --profile ci --json

quality-gate-deep: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . quality-gate --profile deep

# --- workflow targets (developer-facing) ---

check: quality-gate

check-deep: quality-gate-deep

verify: test quality-gate

smoke: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . runtime-smoke

trace-pack: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . trace-pack --output-dir . --compress

results-inspect: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . results-inspect

coverage-map: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . coverage-map

tdd: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . tdd

briefing: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . briefing $(TARGETS)

arch-guard: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . arch-guard

drift-detect: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . drift-detect

snapshot: $(RACCOON_BIN)
	$(RACCOON_BIN) --project-root . --json snapshot

scenario-smoke: $(RACCOON_BIN)
	@if [[ -n "$(SCENARIO)" ]]; then \
		$(RACCOON_BIN) --project-root . scenario-smoke $(SCENARIO); \
	else \
		$(RACCOON_BIN) --project-root . scenario-smoke --list; \
	fi

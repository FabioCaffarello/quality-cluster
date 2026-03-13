
SHELL := /usr/bin/env bash

GO ?= go
DOCKER ?= docker
COMPOSE_FILE ?= deploy/compose/docker-compose.yaml
COMPOSE := $(DOCKER) compose -f $(COMPOSE_FILE)
BUILD_DIR ?= bin
BUILDABLE_SERVICES := configctl server validator

.DEFAULT_GOAL := help

define RUN_IN_MODULES
	@MODULE='$(MODULE)' ./scripts/utils/for-each-module.sh $(1)
endef

.PHONY: help tidy test build docker-build compose-config up-core up-runtime up-all down restart logs ps clean

help:
	@echo "Targets:"
	@echo "  make tidy                 - run go mod tidy in workspace modules"
	@echo "  make test                 - run go test ./... in workspace modules"
	@echo "  make build                - build local service binaries into $(BUILD_DIR)/"
	@echo "  make docker-build         - build docker images for local services"
	@echo "  make compose-config       - render and validate the compose file"
	@echo "  make up-core              - start nats + configctl + server"
	@echo "  make up-runtime           - start core stack plus validator"
	@echo "  make up-all               - start every currently supported compose profile"
	@echo "  make down                 - stop the compose stack"
	@echo "  make restart              - restart the whole stack or SERVICE=<name>"
	@echo "  make logs                 - stream logs for the whole stack or SERVICE=<name>"
	@echo "  make ps                   - show compose service status"
	@echo "  make clean                - remove local build artifacts and Go caches"
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

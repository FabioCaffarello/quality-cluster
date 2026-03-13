
GO ?= go

define RUN_IN_MODULES
	@MODULE='$(MODULE)' ./scripts/utils/for-each-module.sh $(1)
endef

help:
	@echo "Targets:"
	@echo "  make tidy               - run go mod tidy in workspace modules"
	@echo ""
	@echo "Optional: MODULE=./pkg/hello-lib to target a single module"

tidy:
	$(call RUN_IN_MODULES,$(GO) mod tidy)
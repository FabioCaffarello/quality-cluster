package main

import (
	actorcommon "internal/actors/common"
	validatoractor "internal/actors/scopes/validator"
	"internal/shared/bootstrap"
	"internal/shared/settings"
	"log/slog"
	"os"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	if prob := validatoractor.ValidateConfig(config); prob != nil {
		logger.Error("invalid validator config", "error", prob)
		os.Exit(1)
	}

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		logger.Error("create actor engine", "error", err)
		os.Exit(1)
	}

	pid := engine.Spawn(validatoractor.NewSupervisor(config), "validator")
	actorcommon.WaitTillShutdown(engine, pid)
}

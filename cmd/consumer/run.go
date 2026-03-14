package main

import (
	actorcommon "internal/actors/common"
	consumeractor "internal/actors/scopes/consumer"
	"internal/shared/bootstrap"
	"internal/shared/settings"
	"log/slog"
	"os"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	if prob := consumeractor.ValidateConfig(config); prob != nil {
		logger.Error("invalid consumer config", "error", prob)
		os.Exit(1)
	}

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		logger.Error("create actor engine", "error", err)
		os.Exit(1)
	}

	pid := engine.Spawn(consumeractor.NewSupervisor(config), "consumer")
	actorcommon.WaitTillShutdown(engine, pid)
}

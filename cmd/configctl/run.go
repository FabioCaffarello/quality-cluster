package main

import (
	actorcommon "internal/actors/common"
	configactor "internal/actors/scopes/configctl"
	"internal/shared/bootstrap"
	"internal/shared/settings"
	"log/slog"
	"os"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	logger.Info("configctl starting")
	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		logger.Error("create actor engine", "error", err)
		os.Exit(1)
	}

	pid := engine.Spawn(configactor.NewConfigSupervisor(config), "configctl")
	actorcommon.WaitTillShutdown(engine, pid)
}

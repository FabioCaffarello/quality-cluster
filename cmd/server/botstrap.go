package main

import (
	actorcommon "internal/actors/common"
	actorserver "internal/actors/scopes/server"
	"internal/shared/bootstrap"
	"internal/shared/settings"
	"log/slog"
	"os"
)

func Run(config settings.AppConfig) {
	logger := bootstrap.BuildLogger(config.Log)
	slog.SetDefault(logger)

	logger.Info("server starting", "addr", config.HTTP.Addr)
	engine, prob := actorcommon.NewDeafultEngine()
	if prob != nil {
		logger.Error("create actor engine", "error", prob)
		os.Exit(1)
	}

	pid := engine.Spawn(actorserver.NewServer(config.HTTP), "server")
	actorcommon.WaitTillShutdown(engine, pid)
}
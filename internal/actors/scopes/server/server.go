package actorserver

import (
	"context"
	webroutes "internal/interfaces/http/routes"
	"internal/interfaces/http/webserver"
	"internal/shared/settings"
	"log/slog"
	"math"
	"time"

	"github.com/anthdm/hollywood/actor"
)

type Server struct {
	ctx       *actor.Context
	cfg       settings.HTTPConfig
	webServer *webserver.WebServer
	routerPID *actor.PID
	quitch    chan struct{}
}

func NewServer(config settings.HTTPConfig) actor.Producer {
	return func() actor.Receiver {
		return &Server{
			cfg:    config,
			quitch: make(chan struct{}),
		}
	}
}

func (s *Server) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		// TODO: SETUP METRICS

		s.start(c)
		_ = msg

		go func() {
			ticker := time.NewTicker(time.Second * 1)
			for {
				select {
				case <-s.quitch:
					ticker.Stop()
					return
				case <-ticker.C:
					// TODO: ReportServerActiveConnectionsCount
				}
			}
		}()
	case actor.Stopped:
		close(s.quitch)

		if s.webServer != nil {
			shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()

			if err := s.webServer.Shutdown(shutdownCtx); err != nil {
				slog.Error("failed to stop server", "error", err)
			}
		}
	}
}

func (s *Server) start(ctx *actor.Context) {
	s.routerPID = ctx.SpawnChild(
		NewServerRouter(),
		"router",
		actor.WithMaxRestarts(math.MaxInt),
		actor.WithID("1"),
	)

	routes := webroutes.DefaultRoutes()
	s.webServer = webserver.NewWebServer(s.cfg)
	s.webServer.RegisterRoutes(routes)

	go func() {
		err := s.webServer.Start()
		if err != nil {
			slog.Error("failed to start server", "err", err)
			ctx.Engine().Poison(ctx.PID())
		}
		slog.Info("server stopped")
	}()
}

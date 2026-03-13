package actorserver

import (
	"context"
	"internal/interfaces/http/webserver"
	"internal/shared/settings"
	"log/slog"
	"time"

	"github.com/anthdm/hollywood/actor"
)

type Server struct {
	cfg       settings.HTTPConfig
	routes    []webserver.Route
	webServer *webserver.WebServer
}

func NewServer(config settings.HTTPConfig, routes []webserver.Route) actor.Producer {
	return func() actor.Receiver {
		return &Server{
			cfg:    config,
			routes: append([]webserver.Route(nil), routes...),
		}
	}
}

func (s *Server) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		// TODO: SETUP METRICS
		_ = msg
		s.start(c)
		// TODO: send active connection count metrics
	case actor.Stopped:
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
	s.webServer = webserver.NewWebServer(s.cfg)
	s.webServer.RegisterRoutes(s.routes)

	go func() {
		err := s.webServer.Start()
		if err != nil {
			slog.Error("failed to start server", "err", err)
			ctx.Engine().Poison(ctx.PID())
		}
		slog.Info("server stopped")
	}()
}

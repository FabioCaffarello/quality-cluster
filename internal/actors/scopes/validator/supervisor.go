package validator

import (
	"fmt"
	"log/slog"

	adapternats "internal/adapters/nats"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type Supervisor struct {
	cfg    settings.AppConfig
	logger *slog.Logger
}

func NewSupervisor(cfg settings.AppConfig) actor.Producer {
	return func() actor.Receiver {
		return &Supervisor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func (s *Supervisor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		if err := s.start(c); err != nil {
			s.logger.Error("start validator supervisor", "error", err)
			c.Engine().Poison(c.PID())
		}
	case actor.Stopped:
	case actor.Initialized:
	default:
		s.logger.Warn("validator supervisor: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (s *Supervisor) start(c *actor.Context) error {
	if !s.cfg.NATS.Enabled {
		return fmt.Errorf("nats must be enabled for validator")
	}

	cachePID := c.SpawnChild(NewRuntimeCacheActor(), "runtime-cache")
	c.SpawnChild(NewRuntimeConsumerActor(RuntimeConsumerConfig{
		URL:      s.cfg.NATS.URL,
		Registry: adapternats.DefaultConfigctlRegistry(),
		CachePID: cachePID,
	}), "runtime-consumer")

	s.logger.Info("validator runtime started")
	return nil
}

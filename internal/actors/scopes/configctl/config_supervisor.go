package configctl

import (
	"fmt"
	"log/slog"

	adapternats "internal/adapters/nats"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type ConfigSupervisor struct {
	cfg    settings.AppConfig
	logger *slog.Logger
}

func NewConfigSupervisor(config settings.AppConfig) actor.Producer {
	return func() actor.Receiver {
		return &ConfigSupervisor{
			cfg:    config,
			logger: slog.Default(),
		}
	}
}

func (s *ConfigSupervisor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		if err := s.start(c); err != nil {
			s.logger.Error("start config supervisor", "error", err)
			c.Engine().Poison(c.PID())
		}
	case actor.Stopped:
	case actor.Initialized:
	default:
		s.logger.Warn("config supervisor: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (s *ConfigSupervisor) start(ctx *actor.Context) error {
	if !s.cfg.NATS.Enabled {
		return fmt.Errorf("nats must be enabled for configctl")
	}

	registry := adapternats.DefaultConfigctlRegistry()
	eventPID := ctx.SpawnChild(NewEventRouterActor(EventRouterConfig{
		URL:      s.cfg.NATS.URL,
		Source:   "configctl.event-router",
		Registry: registry,
	}), "event-router")

	controlPID := ctx.SpawnChild(NewControlRouterActor(ControlRouterConfig{
		EventRouterPID: eventPID,
		RequestTimeout: s.cfg.NATS.RequestTimeoutDuration(),
	}), "control-router")

	ctx.SpawnChild(NewControlResponderActor(ControlResponderConfig{
		URL:            s.cfg.NATS.URL,
		Source:         "configctl.control-plane",
		ControlRouter:  controlPID,
		Registry:       registry,
		RequestTimeout: s.cfg.NATS.RequestTimeoutDuration(),
	}), "control-responder")

	s.logger.Info("configctl runtime started")
	return nil
}

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
	if prob := ValidateConfig(s.cfg); prob != nil {
		return prob
	}

	cachePID := c.SpawnChild(NewRuntimeCacheActor(), "runtime-cache")
	resultsPID := c.SpawnChild(NewValidationResultsStoreActor(), "results-store")
	routerPID := c.SpawnChild(NewValidationRouterActor(ValidationRouterConfig{
		RuntimeCachePID: cachePID,
		ResultsStorePID: resultsPID,
		RequestTimeout:  s.cfg.NATS.RequestTimeoutDuration(),
	}), "validation-router")
	c.SpawnChild(NewRuntimeConsumerActor(RuntimeConsumerConfig{
		URL:      s.cfg.NATS.URL,
		Registry: adapternats.DefaultConfigctlRegistry(),
		CachePID: cachePID,
	}), "runtime-consumer")
	c.SpawnChild(NewDataPlaneConsumerActor(DataPlaneConsumerConfig{
		URL:            s.cfg.NATS.URL,
		Registry:       adapternats.DefaultDataPlaneRegistry(),
		RouterPID:      routerPID,
		RequestTimeout: s.cfg.NATS.RequestTimeoutDuration(),
	}), "dataplane-consumer")
	c.SpawnChild(NewRuntimeQueryResponderActor(RuntimeQueryResponderConfig{
		URL:             s.cfg.NATS.URL,
		Source:          "validator.runtime",
		Registry:        adapternats.DefaultValidatorRuntimeRegistry(),
		RuntimeCachePID: cachePID,
		RequestTimeout:  s.cfg.NATS.RequestTimeoutDuration(),
	}), "runtime-query-responder")
	c.SpawnChild(NewResultsQueryResponderActor(ResultsQueryResponderConfig{
		URL:             s.cfg.NATS.URL,
		Source:          "validator.results",
		Registry:        adapternats.DefaultValidatorResultsRegistry(),
		ResultsStorePID: resultsPID,
		RequestTimeout:  s.cfg.NATS.RequestTimeoutDuration(),
	}), "results-query-responder")

	s.logger.Info("validator started")
	return nil
}

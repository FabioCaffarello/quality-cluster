package validator

import (
	"fmt"
	"log/slog"

	adapternats "internal/adapters/nats"

	"github.com/anthdm/hollywood/actor"
)

type RuntimeConsumerConfig struct {
	URL      string
	Registry adapternats.ConfigctlRegistry
	CachePID *actor.PID
}

type RuntimeConsumerActor struct {
	cfg      RuntimeConsumerConfig
	logger   *slog.Logger
	engine   *actor.Engine
	consumer *adapternats.RuntimeUpdatedConsumer
}

func NewRuntimeConsumerActor(cfg RuntimeConsumerConfig) actor.Producer {
	return func() actor.Receiver {
		return &RuntimeConsumerActor{cfg: cfg}
	}
}

func (a *RuntimeConsumerActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		consumer := adapternats.NewRuntimeUpdatedConsumer(a.cfg.URL, a.cfg.Registry.ValidatorCache, &cacheForwarder{
			engine:   a.engine,
			cachePID: a.cfg.CachePID,
		})
		if err := consumer.Start(); err != nil {
			a.logger.Error("start validator runtime consumer", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.consumer = consumer
	case actor.Stopped:
		if a.consumer != nil {
			if err := a.consumer.Close(); err != nil {
				a.logger.Error("close validator runtime consumer", "error", err)
			}
		}
	default:
		a.logger.Warn("validator runtime consumer: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

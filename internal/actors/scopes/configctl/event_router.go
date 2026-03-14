package configctl

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	adapternats "internal/adapters/nats"

	"github.com/anthdm/hollywood/actor"
)

type EventRouterConfig struct {
	URL      string
	Source   string
	Registry adapternats.ConfigctlRegistry
}

type EventRouterActor struct {
	cfg       EventRouterConfig
	logger    *slog.Logger
	publisher *adapternats.DomainEventPublisher
}

func NewEventRouterActor(cfg EventRouterConfig) actor.Producer {
	return func() actor.Receiver {
		return &EventRouterActor{cfg: cfg}
	}
}

func (a *EventRouterActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		publisher := adapternats.NewDomainEventPublisher(a.cfg.URL, a.cfg.Source, a.cfg.Registry)
		if err := publisher.Start(); err != nil {
			a.logger.Error("start domain event publisher", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.publisher = publisher
	case actor.Stopped:
		if a.publisher != nil {
			if err := a.publisher.Close(); err != nil {
				a.logger.Error("close domain event publisher", "error", err)
			}
		}
	case publishDomainEventMessage:
		reply := publishDomainEventResult{}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		reply.Prob = a.publisher.Publish(ctx, msg.Event)
		cancel()
		if sender := c.Sender(); sender != nil {
			c.Send(sender, reply)
		}
	default:
		a.logger.Warn("configctl event router: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

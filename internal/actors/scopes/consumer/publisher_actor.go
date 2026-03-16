package consumer

import (
	"fmt"
	"log/slog"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type DataPlanePublisherConfig struct {
	URL      string
	Source   string
	Registry adapternats.DataPlaneRegistry
}

type DataPlanePublisherActor struct {
	cfg       DataPlanePublisherConfig
	logger    *slog.Logger
	publisher *adapternats.DataPlanePublisher
}

func NewDataPlanePublisherActor(cfg DataPlanePublisherConfig) actor.Producer {
	return func() actor.Receiver {
		return &DataPlanePublisherActor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func (a *DataPlanePublisherActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		publisher := adapternats.NewDataPlanePublisher(a.cfg.URL, a.cfg.Source, a.cfg.Registry)
		if err := publisher.Start(); err != nil {
			c.Send(c.Parent(), dataPlanePublisherFailedMessage{Err: err})
			c.Engine().Poison(c.PID())
			return
		}
		a.publisher = publisher
		c.Send(c.Parent(), dataPlanePublisherReadyMessage{})
	case publishRoutedMessageMessage:
		var prob *problem.Problem
		if a.publisher == nil {
			prob = problem.New(problem.Unavailable, "data plane publisher is unavailable").MarkRetryable()
		} else {
			prob = a.publisher.Publish(c.Context(), msg.Message.Route.JetStreamSubject, msg.Message.CorrelationID, msg.Message.Message)
		}
		c.Respond(publishRoutedMessageResult{Prob: prob})
	case actor.Stopped:
		if a.publisher != nil {
			if err := a.publisher.Close(); err != nil {
				a.logger.Error("close data plane publisher", "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("consumer publisher: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

package validator

import (
	"context"
	"fmt"
	"log/slog"

	"internal/application/configctl/contracts"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type applyRuntimeUpdateMessage struct {
	Event contracts.RuntimeUpdatedEvent
}

type RuntimeCacheActor struct {
	logger   *slog.Logger
	snapshot contracts.RuntimeSnapshot
}

func NewRuntimeCacheActor() actor.Producer {
	return func() actor.Receiver {
		return &RuntimeCacheActor{
			logger: slog.Default(),
		}
	}
}

func (a *RuntimeCacheActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		a.logger.Info("validator runtime cache started")
	case applyRuntimeUpdateMessage:
		a.snapshot = msg.Event.Snapshot
		a.logger.Info("validator runtime cache updated", "version", a.snapshot.Version, "configs", len(a.snapshot.Configs))
	default:
		a.logger.Warn("validator runtime cache: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

type cacheForwarder struct {
	engine   *actor.Engine
	cachePID *actor.PID
}

func (f *cacheForwarder) HandleRuntimeUpdated(_ context.Context, event contracts.RuntimeUpdatedEvent) *problem.Problem {
	if f == nil || f.engine == nil || f.cachePID == nil {
		return problem.New(problem.Unavailable, "validator cache is unavailable")
	}
	f.engine.Send(f.cachePID, applyRuntimeUpdateMessage{Event: event})
	return nil
}

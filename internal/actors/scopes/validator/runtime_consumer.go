package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type RuntimeConsumerConfig struct {
	URL      string
	Registry adapternats.ConfigctlRegistry
	CachePID *actor.PID
}

type RuntimeConsumerActor struct {
	cfg                 RuntimeConsumerConfig
	logger              *slog.Logger
	engine              *actor.Engine
	activatedConsumer   *adapternats.ConfigActivatedConsumer
	deactivatedConsumer *adapternats.ConfigDeactivatedConsumer
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
		activatedConsumer := adapternats.NewConfigActivatedConsumer(a.cfg.URL, a.cfg.Registry.ValidatorRuntime, &cacheForwarder{
			engine:   a.engine,
			cachePID: a.cfg.CachePID,
		})
		if err := activatedConsumer.Start(); err != nil {
			a.logger.Error("start validator runtime consumer", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		deactivatedConsumer := adapternats.NewConfigDeactivatedConsumer(a.cfg.URL, a.cfg.Registry.ValidatorRuntimeCleared, &cacheEvicter{
			engine:   a.engine,
			cachePID: a.cfg.CachePID,
		})
		if err := deactivatedConsumer.Start(); err != nil {
			_ = activatedConsumer.Close()
			a.logger.Error("start validator runtime clear consumer", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.activatedConsumer = activatedConsumer
		a.deactivatedConsumer = deactivatedConsumer
	case actor.Stopped:
		if a.activatedConsumer != nil {
			if err := a.activatedConsumer.Close(); err != nil {
				a.logger.Error("close validator runtime consumer", "error", err)
			}
		}
		if a.deactivatedConsumer != nil {
			if err := a.deactivatedConsumer.Close(); err != nil {
				a.logger.Error("close validator runtime clear consumer", "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validator runtime consumer: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

type cacheEvicter struct {
	engine   *actor.Engine
	cachePID *actor.PID
}

func (f *cacheEvicter) HandleConfigDeactivated(_ context.Context, event configdomain.ConfigDeactivatedEvent) *problem.Problem {
	if f == nil || f.engine == nil || f.cachePID == nil {
		return problem.New(problem.Unavailable, "validator cache is unavailable")
	}
	deactivatedAt := time.Time{}
	if event.Activation.DeactivatedAt != nil {
		deactivatedAt = *event.Activation.DeactivatedAt
	}
	f.engine.Send(f.cachePID, evictRuntimeScopeMessage{
		Scope:         event.Scope,
		ConfigSetID:   event.ConfigSetID,
		VersionID:     event.VersionID,
		DeactivatedAt: deactivatedAt,
	})
	return nil
}

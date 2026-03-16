package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type DataPlaneConsumerConfig struct {
	URL            string
	Registry       adapternats.DataPlaneRegistry
	RouterPID      *actor.PID
	RequestTimeout time.Duration
}

type DataPlaneConsumerActor struct {
	cfg      DataPlaneConsumerConfig
	logger   *slog.Logger
	engine   *actor.Engine
	consumer *adapternats.DataPlaneConsumer
}

func NewDataPlaneConsumerActor(cfg DataPlaneConsumerConfig) actor.Producer {
	return func() actor.Receiver {
		return &DataPlaneConsumerActor{cfg: cfg}
	}
}

func (a *DataPlaneConsumerActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		consumer := adapternats.NewDataPlaneConsumer(a.cfg.URL, a.cfg.Registry.ValidatorIngested, &validationForwarder{
			engine:    a.engine,
			routerPID: a.cfg.RouterPID,
			timeout:   a.cfg.RequestTimeout,
		})
		if err := consumer.Start(); err != nil {
			a.logger.Error("start validator data plane consumer", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.consumer = consumer
	case actor.Stopped:
		if a.consumer != nil {
			if err := a.consumer.Close(); err != nil {
				a.logger.Error("close validator data plane consumer", "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validator data plane consumer: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

type validationForwarder struct {
	engine    *actor.Engine
	routerPID *actor.PID
	timeout   time.Duration
}

func (f *validationForwarder) HandleDataPlaneMessage(_ context.Context, message dataplaneapp.Message) *problem.Problem {
	if f == nil || f.engine == nil || f.routerPID == nil {
		return problem.New(problem.Unavailable, "validator dependencies are unavailable").MarkRetryable()
	}

	timeout := f.timeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	rawResult, err := f.engine.Request(f.routerPID, routeValidationMessage{Message: message}, timeout).Result()
	if err != nil {
		return problem.Wrap(err, problem.Unavailable, "request validator router").MarkRetryable()
	}

	resolved, ok := rawResult.(routeValidationResult)
	if !ok {
		return problem.New(problem.Internal, "validator router response is invalid")
	}
	return resolved.Prob
}

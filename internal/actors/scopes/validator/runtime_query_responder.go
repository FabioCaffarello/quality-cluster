package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	adapternats "internal/adapters/nats"
	runtimecontracts "internal/application/validatorruntime/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type RuntimeQueryResponderConfig struct {
	URL             string
	Source          string
	Registry        adapternats.ValidatorRuntimeRegistry
	RuntimeCachePID *actor.PID
	RequestTimeout  time.Duration
}

type RuntimeQueryResponderActor struct {
	cfg       RuntimeQueryResponderConfig
	logger    *slog.Logger
	engine    *actor.Engine
	responder *adapternats.RequestReplyResponder
}

func NewRuntimeQueryResponderActor(cfg RuntimeQueryResponderConfig) actor.Producer {
	return func() actor.Receiver {
		return &RuntimeQueryResponderActor{cfg: cfg}
	}
}

func (a *RuntimeQueryResponderActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		routes := []adapternats.ControlRoute{
			adapternats.NewTypedControlRoute(a.cfg.Registry.GetActive, a.cfg.Source, a.handleGetActiveRuntime),
		}
		responder := adapternats.NewRequestReplyResponder(a.cfg.URL, routes)
		if err := responder.Start(); err != nil {
			a.logger.Error("start validator runtime responder", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.responder = responder
	case actor.Stopped:
		if a.responder != nil {
			if err := a.responder.Close(); err != nil {
				a.logger.Error("close validator runtime responder", "error", err)
			}
		}
	default:
		a.logger.Warn("validator runtime responder: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *RuntimeQueryResponderActor) handleGetActiveRuntime(ctx context.Context, query runtimecontracts.GetActiveRuntimeQuery) (runtimecontracts.GetActiveRuntimeReply, *problem.Problem) {
	if a.engine == nil || a.cfg.RuntimeCachePID == nil {
		return runtimecontracts.GetActiveRuntimeReply{}, problem.New(problem.Unavailable, "validator runtime cache is unavailable")
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := a.engine.Request(a.cfg.RuntimeCachePID, queryActiveRuntimeMessage{
		Query:         query.Normalize(),
		CorrelationID: requestctx.CorrelationID(ctx),
	}, timeout)
	result, err := response.Result()
	if err != nil {
		return runtimecontracts.GetActiveRuntimeReply{}, problem.Wrap(err, problem.Unavailable, "request validator runtime cache")
	}

	typed, ok := result.(queryActiveRuntimeResult)
	if !ok {
		return runtimecontracts.GetActiveRuntimeReply{}, problem.New(problem.Internal, "validator runtime response is invalid")
	}

	return typed.Reply, typed.Prob
}

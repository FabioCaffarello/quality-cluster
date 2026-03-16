package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	adapternats "internal/adapters/nats"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type IncidentsQueryResponderConfig struct {
	URL             string
	Source          string
	Registry        adapternats.ValidatorIncidentsRegistry
	ResultsStorePID *actor.PID
	RequestTimeout  time.Duration
}

type IncidentsQueryResponderActor struct {
	cfg       IncidentsQueryResponderConfig
	logger    *slog.Logger
	engine    *actor.Engine
	responder *adapternats.RequestReplyResponder
}

func NewIncidentsQueryResponderActor(cfg IncidentsQueryResponderConfig) actor.Producer {
	return func() actor.Receiver {
		return &IncidentsQueryResponderActor{cfg: cfg}
	}
}

func (a *IncidentsQueryResponderActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		routes := []adapternats.ControlRoute{
			adapternats.NewTypedControlRoute(a.cfg.Registry.List, a.cfg.Source, a.handleListValidationIncidents),
		}
		responder := adapternats.NewRequestReplyResponder(a.cfg.URL, routes)
		if err := responder.Start(); err != nil {
			a.logger.Error("start validator incidents responder", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.responder = responder
	case actor.Stopped:
		if a.responder != nil {
			if err := a.responder.Close(); err != nil {
				a.logger.Error("close validator incidents responder", "error", err)
			}
		}
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validator incidents responder: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *IncidentsQueryResponderActor) handleListValidationIncidents(ctx context.Context, query validatorincidentscontracts.ListValidationIncidentsQuery) (validatorincidentscontracts.ListValidationIncidentsReply, *problem.Problem) {
	if a.engine == nil || a.cfg.ResultsStorePID == nil {
		return validatorincidentscontracts.ListValidationIncidentsReply{}, problem.New(problem.Unavailable, "validator incidents store is unavailable")
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := a.engine.Request(a.cfg.ResultsStorePID, listValidationIncidentsMessage{
		Query:         query.Normalize(),
		CorrelationID: requestctx.CorrelationID(ctx),
	}, timeout)
	result, err := response.Result()
	if err != nil {
		return validatorincidentscontracts.ListValidationIncidentsReply{}, problem.Wrap(err, problem.Unavailable, "request validator incidents store")
	}

	typed, ok := result.(listValidationIncidentsResult)
	if !ok {
		return validatorincidentscontracts.ListValidationIncidentsReply{}, problem.New(problem.Internal, "validator incidents response is invalid")
	}

	return typed.Reply, typed.Prob
}

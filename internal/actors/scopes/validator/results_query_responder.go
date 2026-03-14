package validator

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	adapternats "internal/adapters/nats"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/anthdm/hollywood/actor"
)

type ResultsQueryResponderConfig struct {
	URL             string
	Source          string
	Registry        adapternats.ValidatorResultsRegistry
	ResultsStorePID *actor.PID
	RequestTimeout  time.Duration
}

type ResultsQueryResponderActor struct {
	cfg       ResultsQueryResponderConfig
	logger    *slog.Logger
	engine    *actor.Engine
	responder *adapternats.RequestReplyResponder
}

func NewResultsQueryResponderActor(cfg ResultsQueryResponderConfig) actor.Producer {
	return func() actor.Receiver {
		return &ResultsQueryResponderActor{cfg: cfg}
	}
}

func (a *ResultsQueryResponderActor) Receive(c *actor.Context) {
	if a.logger == nil {
		a.logger = slog.Default()
	}
	if a.engine == nil && c != nil {
		a.engine = c.Engine()
	}

	switch msg := c.Message().(type) {
	case actor.Started:
		routes := []adapternats.ControlRoute{
			adapternats.NewTypedControlRoute(a.cfg.Registry.List, a.cfg.Source, a.handleListValidationResults),
		}
		responder := adapternats.NewRequestReplyResponder(a.cfg.URL, routes)
		if err := responder.Start(); err != nil {
			a.logger.Error("start validator results responder", "error", err)
			c.Engine().Poison(c.PID())
			return
		}
		a.responder = responder
	case actor.Stopped:
		if a.responder != nil {
			if err := a.responder.Close(); err != nil {
				a.logger.Error("close validator results responder", "error", err)
			}
		}
	default:
		a.logger.Warn("validator results responder: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ResultsQueryResponderActor) handleListValidationResults(ctx context.Context, query validatorresultscontracts.ListValidationResultsQuery) (validatorresultscontracts.ListValidationResultsReply, *problem.Problem) {
	if a.engine == nil || a.cfg.ResultsStorePID == nil {
		return validatorresultscontracts.ListValidationResultsReply{}, problem.New(problem.Unavailable, "validator results store is unavailable")
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := a.engine.Request(a.cfg.ResultsStorePID, listValidationResultsMessage{
		Query:         query.Normalize(),
		CorrelationID: requestctx.CorrelationID(ctx),
	}, timeout)
	result, err := response.Result()
	if err != nil {
		return validatorresultscontracts.ListValidationResultsReply{}, problem.Wrap(err, problem.Unavailable, "request validator results store")
	}

	typed, ok := result.(listValidationResultsResult)
	if !ok {
		return validatorresultscontracts.ListValidationResultsReply{}, problem.New(problem.Internal, "validator results response is invalid")
	}

	return typed.Reply, typed.Prob
}

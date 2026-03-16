package validator

import (
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	validatorresultsapp "internal/application/validatorresults"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

type ValidationWorkerConfig struct {
	ResultsStorePID *actor.PID
	RequestTimeout  time.Duration
}

type ValidationWorkerActor struct {
	cfg    ValidationWorkerConfig
	logger *slog.Logger
}

func NewValidationWorkerActor(cfg ValidationWorkerConfig) actor.Producer {
	return func() actor.Receiver {
		return &ValidationWorkerActor{
			cfg:    cfg,
			logger: slog.Default(),
		}
	}
}

func (a *ValidationWorkerActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case validationWorkMessage:
		c.Send(c.Parent(), validationWorkCompletedMessage{
			RequestID: msg.RequestID,
			Prob:      a.handleWork(c, msg),
		})
	case actor.Stopped:
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validation worker: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ValidationWorkerActor) handleWork(c *actor.Context, msg validationWorkMessage) *problem.Problem {
	if a.cfg.ResultsStorePID == nil {
		return problem.New(problem.Unavailable, "validator results store is unavailable").MarkRetryable()
	}

	result, prob := validatorresultsapp.Evaluate(msg.Runtime.Runtime, msg.Message, time.Now().UTC())
	if prob != nil {
		return prob
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	rawResult, err := c.Request(a.cfg.ResultsStorePID, recordValidationResultMessage{Result: result}, timeout).Result()
	if err != nil {
		return problem.Wrap(err, problem.Unavailable, "record validation result").MarkRetryable()
	}

	reply, ok := rawResult.(recordValidationResultResult)
	if !ok {
		return problem.New(problem.Internal, "validation results store response is invalid")
	}
	return reply.Prob
}

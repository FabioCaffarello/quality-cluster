package validator

import (
	"fmt"
	"log/slog"
	"time"

	actorcommon "internal/actors/common"
	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

const defaultValidationWorkerCount = 4

type routeValidationMessage struct {
	Message dataplaneapp.Message
}

type routeValidationResult struct {
	Prob *problem.Problem
}

type validationWorkMessage struct {
	RequestID string
	Runtime   resolveRuntimeProjectionResult
	Message   dataplaneapp.Message
}

type validationWorkCompletedMessage struct {
	RequestID string
	Prob      *problem.Problem
}

type ValidationRouterConfig struct {
	RuntimeCachePID *actor.PID
	ResultsStorePID *actor.PID
	WorkerCount     int
	RequestTimeout  time.Duration
}

type ValidationRouterActor struct {
	cfg        ValidationRouterConfig
	logger     *slog.Logger
	workers    []*actor.PID
	pending    map[string]*actor.PID
	nextWorker int
	sequence   uint64
}

func NewValidationRouterActor(cfg ValidationRouterConfig) actor.Producer {
	return func() actor.Receiver {
		return &ValidationRouterActor{
			cfg:     cfg,
			logger:  slog.Default(),
			pending: make(map[string]*actor.PID),
		}
	}
}

func (a *ValidationRouterActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		workers := a.cfg.WorkerCount
		if workers <= 0 {
			workers = defaultValidationWorkerCount
		}
		for index := 0; index < workers; index++ {
			a.workers = append(a.workers, c.SpawnChild(NewValidationWorkerActor(ValidationWorkerConfig{
				ResultsStorePID: a.cfg.ResultsStorePID,
				RequestTimeout:  a.cfg.RequestTimeout,
			}), fmt.Sprintf("worker-%d", index+1)))
		}
	case routeValidationMessage:
		a.route(c, msg)
	case validationWorkCompletedMessage:
		sender := a.pending[msg.RequestID]
		delete(a.pending, msg.RequestID)
		if sender != nil {
			c.Send(sender, routeValidationResult{Prob: msg.Prob})
		}
	case actor.Stopped:
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validation router: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ValidationRouterActor) route(c *actor.Context, msg routeValidationMessage) {
	if c.Sender() == nil {
		return
	}
	if a.cfg.RuntimeCachePID == nil {
		c.Respond(routeValidationResult{Prob: problem.New(problem.Unavailable, "validator runtime cache is unavailable").MarkRetryable()})
		return
	}
	if len(a.workers) == 0 {
		c.Respond(routeValidationResult{Prob: problem.New(problem.Unavailable, "validator worker pool is unavailable").MarkRetryable()})
		return
	}

	timeout := a.cfg.RequestTimeout
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	response := c.Request(a.cfg.RuntimeCachePID, resolveRuntimeProjectionMessage{
		ScopeKind: msg.Message.Binding.Scope.Kind,
		ScopeKey:  msg.Message.Binding.Scope.Key,
	}, timeout)
	rawResult, err := response.Result()
	if err != nil {
		c.Respond(routeValidationResult{Prob: problem.Wrap(err, problem.Unavailable, "request validator runtime cache").MarkRetryable()})
		return
	}

	resolved, ok := rawResult.(resolveRuntimeProjectionResult)
	if !ok {
		c.Respond(routeValidationResult{Prob: problem.New(problem.Internal, "validator runtime resolution response is invalid")})
		return
	}
	if resolved.Prob != nil {
		c.Respond(routeValidationResult{Prob: resolved.Prob})
		return
	}

	requestID := a.nextRequestID(msg.Message.MessageID())
	a.pending[requestID] = c.Sender()
	c.Send(a.nextWorkerPID(), validationWorkMessage{
		RequestID: requestID,
		Runtime:   resolved,
		Message:   msg.Message,
	})
}

func (a *ValidationRouterActor) nextWorkerPID() *actor.PID {
	if len(a.workers) == 0 {
		return nil
	}
	pid := a.workers[a.nextWorker%len(a.workers)]
	a.nextWorker++
	return pid
}

func (a *ValidationRouterActor) nextRequestID(messageID string) string {
	a.sequence++
	return fmt.Sprintf("%s:%d", messageID, a.sequence)
}

package validator

import (
	"fmt"
	"log/slog"
	"strings"

	actorcommon "internal/actors/common"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"

	"github.com/anthdm/hollywood/actor"
)

const defaultValidationResultsCapacity = 200

type recordValidationResultMessage struct {
	Result validatorresultscontracts.ValidationResultRecord
}

type recordValidationResultResult struct {
	Prob *problem.Problem
}

type listValidationResultsMessage struct {
	Query         validatorresultscontracts.ListValidationResultsQuery
	CorrelationID string
}

type listValidationResultsResult struct {
	Reply validatorresultscontracts.ListValidationResultsReply
	Prob  *problem.Problem
}

type ValidationResultsStoreActor struct {
	logger   *slog.Logger
	capacity int
	results  []validatorresultscontracts.ValidationResultRecord
}

func NewValidationResultsStoreActor() actor.Producer {
	return func() actor.Receiver {
		return &ValidationResultsStoreActor{
			logger:   slog.Default(),
			capacity: defaultValidationResultsCapacity,
			results:  make([]validatorresultscontracts.ValidationResultRecord, 0, defaultValidationResultsCapacity),
		}
	}
}

func (a *ValidationResultsStoreActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		a.logger.Info("validator results store started")
	case recordValidationResultMessage:
		c.Respond(recordValidationResultResult{Prob: a.record(msg.Result)})
	case listValidationResultsMessage:
		a.reply(c, listValidationResultsResult{
			Reply: validatorresultscontracts.ListValidationResultsReply{
				Results: a.list(msg.Query.Normalize()),
			},
		})
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("validator results store: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ValidationResultsStoreActor) record(result validatorresultscontracts.ValidationResultRecord) *problem.Problem {
	if a == nil {
		return problem.New(problem.Unavailable, "validation results store is unavailable").MarkRetryable()
	}
	if prob := result.Validate(); prob != nil {
		a.logger.Warn("skip invalid validation result", "error", prob)
		return prob
	}

	filtered := a.results[:0]
	for _, existing := range a.results {
		if strings.TrimSpace(existing.MessageID) == strings.TrimSpace(result.MessageID) {
			continue
		}
		filtered = append(filtered, existing)
	}

	a.results = append([]validatorresultscontracts.ValidationResultRecord{result}, filtered...)
	if len(a.results) > a.capacity {
		a.results = a.results[:a.capacity]
	}
	return nil
}

func (a *ValidationResultsStoreActor) list(query validatorresultscontracts.ListValidationResultsQuery) []validatorresultscontracts.ValidationResultRecord {
	if a == nil || len(a.results) == 0 {
		return nil
	}

	results := make([]validatorresultscontracts.ValidationResultRecord, 0, min(query.Limit, len(a.results)))
	for _, result := range a.results {
		if !matchesResultsQuery(result, query) {
			continue
		}
		results = append(results, result)
		if len(results) >= query.Limit {
			break
		}
	}
	return results
}

func matchesResultsQuery(result validatorresultscontracts.ValidationResultRecord, query validatorresultscontracts.ListValidationResultsQuery) bool {
	if strings.TrimSpace(result.Binding.Scope.Kind) != query.ScopeKind {
		return false
	}
	if strings.TrimSpace(result.Binding.Scope.Key) != query.ScopeKey {
		return false
	}
	if query.BindingName != "" && strings.TrimSpace(result.Binding.Name) != query.BindingName {
		return false
	}
	if query.Topic != "" && strings.TrimSpace(result.Binding.Topic) != query.Topic {
		return false
	}
	if query.MessageID != "" && strings.TrimSpace(result.MessageID) != query.MessageID {
		return false
	}
	if query.CorrelationID != "" && strings.TrimSpace(result.CorrelationID) != query.CorrelationID {
		return false
	}
	return true
}

func (a *ValidationResultsStoreActor) reply(c *actor.Context, msg any) {
	if sender := c.Sender(); sender != nil {
		c.Send(sender, msg)
	}
}

func min(left, right int) int {
	if left < right {
		return left
	}
	return right
}

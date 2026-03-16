package validator

import (
	"fmt"
	"log/slog"
	"sort"
	"strings"

	actorcommon "internal/actors/common"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
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

type listValidationIncidentsMessage struct {
	Query         validatorincidentscontracts.ListValidationIncidentsQuery
	CorrelationID string
}

type listValidationIncidentsResult struct {
	Reply validatorincidentscontracts.ListValidationIncidentsReply
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
	case listValidationIncidentsMessage:
		a.reply(c, listValidationIncidentsResult{
			Reply: validatorincidentscontracts.ListValidationIncidentsReply{
				Incidents: a.listIncidents(msg.Query.Normalize()),
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

	processingKey := strings.TrimSpace(result.NormalizedProcessingKey())
	filtered := a.results[:0]
	for _, existing := range a.results {
		if sameValidationResult(existing, result, processingKey) {
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

func (a *ValidationResultsStoreActor) listIncidents(query validatorincidentscontracts.ListValidationIncidentsQuery) []validatorincidentscontracts.ValidationIncidentRecord {
	if a == nil || len(a.results) == 0 {
		return nil
	}

	aggregated := make(map[string]validatorincidentscontracts.ValidationIncidentRecord)
	order := make([]string, 0)
	for _, result := range a.results {
		if result.Status != validatorresultscontracts.ValidationStatusFailed || len(result.Violations) == 0 {
			continue
		}

		incidentKey := strings.TrimSpace(validatorincidentscontracts.BuildIncidentKey(result))
		if incidentKey == "" {
			continue
		}

		record, exists := aggregated[incidentKey]
		if !exists {
			record = validatorincidentscontracts.ValidationIncidentRecord{
				IncidentKey: incidentKey,
				Kind:        validatorincidentscontracts.ValidationIncidentKindRuleViolation,
				Status:      validatorincidentscontracts.ValidationIncidentStatusOpen,
				Binding: validatorincidentscontracts.ValidationIncidentBindingRecord{
					Name:  result.Binding.Name,
					Topic: result.Binding.Topic,
					Scope: result.Binding.Scope,
				},
				Config: validatorincidentscontracts.ValidationIncidentConfigRecord{
					SetID:              result.Config.SetID,
					Key:                result.Config.Key,
					VersionID:          result.Config.VersionID,
					Version:            result.Config.Version,
					DefinitionChecksum: result.Config.DefinitionChecksum,
				},
				FirstSeenAt:         result.ProcessedAt,
				LastSeenAt:          result.ProcessedAt,
				LatestMessageID:     result.MessageID,
				LatestCorrelationID: result.CorrelationID,
				LatestProcessingKey: result.NormalizedProcessingKey(),
				Violations:          append([]validatorresultscontracts.ViolationRecord(nil), result.Violations...),
			}
			order = append(order, incidentKey)
		}

		record.Count++
		if record.FirstSeenAt.IsZero() || result.ProcessedAt.Before(record.FirstSeenAt) {
			record.FirstSeenAt = result.ProcessedAt
		}
		if result.ProcessedAt.After(record.LastSeenAt) || record.LastSeenAt.IsZero() {
			record.LastSeenAt = result.ProcessedAt
			record.LatestMessageID = result.MessageID
			record.LatestCorrelationID = result.CorrelationID
			record.LatestProcessingKey = result.NormalizedProcessingKey()
			record.Violations = append([]validatorresultscontracts.ViolationRecord(nil), result.Violations...)
		}
		aggregated[incidentKey] = record
	}

	if len(aggregated) == 0 {
		return nil
	}

	incidents := make([]validatorincidentscontracts.ValidationIncidentRecord, 0, min(query.Limit, len(aggregated)))
	for _, incidentKey := range order {
		incident := aggregated[incidentKey]
		if !matchesIncidentsQuery(incident, query) {
			continue
		}
		incidents = append(incidents, incident)
		if len(incidents) >= query.Limit {
			break
		}
	}

	sort.SliceStable(incidents, func(left, right int) bool {
		return incidents[left].LastSeenAt.After(incidents[right].LastSeenAt)
	})
	return incidents
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
	if query.Status != "" && result.Status != query.Status {
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

func matchesIncidentsQuery(incident validatorincidentscontracts.ValidationIncidentRecord, query validatorincidentscontracts.ListValidationIncidentsQuery) bool {
	if strings.TrimSpace(incident.Binding.Scope.Kind) != query.ScopeKind {
		return false
	}
	if strings.TrimSpace(incident.Binding.Scope.Key) != query.ScopeKey {
		return false
	}
	if query.BindingName != "" && strings.TrimSpace(incident.Binding.Name) != query.BindingName {
		return false
	}
	if query.Topic != "" && strings.TrimSpace(incident.Binding.Topic) != query.Topic {
		return false
	}
	if query.Kind != "" && incident.Kind != query.Kind {
		return false
	}
	if query.Status != "" && incident.Status != query.Status {
		return false
	}
	return true
}

func sameValidationResult(existing validatorresultscontracts.ValidationResultRecord, incoming validatorresultscontracts.ValidationResultRecord, incomingKey string) bool {
	existingKey := strings.TrimSpace(existing.NormalizedProcessingKey())
	if incomingKey != "" && existingKey != "" {
		return existingKey == incomingKey
	}
	return strings.TrimSpace(existing.MessageID) == strings.TrimSpace(incoming.MessageID)
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

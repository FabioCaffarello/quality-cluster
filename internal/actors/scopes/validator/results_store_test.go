package validator

import (
	"testing"
	"time"

	actorcommon "internal/actors/common"
	sharedruntime "internal/application/runtimecontracts"
	validatorincidentscontracts "internal/application/validatorincidents/contracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
)

func TestValidationResultsStoreActorRecordsAndQueriesResults(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-results-store-test")
	engine.Send(storePID, recordValidationResultMessage{
		Result: validatorresultscontracts.ValidationResultRecord{
			ProcessingKey: "msg-1|global|default|orders|sales.order.created|ver-1|sum-1",
			MessageID:     "msg-1",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
			Status:      validatorresultscontracts.ValidationStatusPassed,
			ProcessedAt: time.Unix(10, 0).UTC(),
		},
	})

	result, err := engine.Request(storePID, listValidationResultsMessage{
		Query: validatorresultscontracts.ListValidationResultsQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("query results store: %v", err)
	}

	reply := result.(listValidationResultsResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if len(reply.Reply.Results) != 1 || reply.Reply.Results[0].MessageID != "msg-1" {
		t.Fatalf("unexpected results reply %+v", reply.Reply.Results)
	}
}

func TestValidationResultsStoreActorFiltersByStatus(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-results-store-status-test")
	engine.Send(storePID, recordValidationResultMessage{
		Result: validatorresultscontracts.ValidationResultRecord{
			ProcessingKey: "msg-pass|global|default|orders|sales.order.created|ver-1|sum-1",
			MessageID:     "msg-pass",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
			Status:      validatorresultscontracts.ValidationStatusPassed,
			ProcessedAt: time.Unix(10, 0).UTC(),
		},
	})
	engine.Send(storePID, recordValidationResultMessage{
		Result: validatorresultscontracts.ValidationResultRecord{
			ProcessingKey: "msg-fail|global|default|orders|sales.order.created|ver-1|sum-1",
			MessageID:     "msg-fail",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
			Status:      validatorresultscontracts.ValidationStatusFailed,
			ProcessedAt: time.Unix(11, 0).UTC(),
			Violations: []validatorresultscontracts.ViolationRecord{{
				Rule:     "order_id_required",
				Field:    "order_id",
				Operator: "required",
				Severity: "error",
				Message:  "field is required",
			}},
		},
	})

	result, err := engine.Request(storePID, listValidationResultsMessage{
		Query: validatorresultscontracts.ListValidationResultsQuery{
			Status: validatorresultscontracts.ValidationStatusFailed,
		},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("query results store: %v", err)
	}

	reply := result.(listValidationResultsResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if len(reply.Reply.Results) != 1 || reply.Reply.Results[0].MessageID != "msg-fail" {
		t.Fatalf("unexpected filtered results %+v", reply.Reply.Results)
	}
}

func TestValidationResultsStoreActorAggregatesValidationIncidents(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-incidents-store-test")
	engine.Send(storePID, recordValidationResultMessage{
		Result: validatorresultscontracts.ValidationResultRecord{
			ProcessingKey: "msg-1|global|default|orders|sales.order.created|ver-1|sum-1",
			MessageID:     "msg-1",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
			Status:      validatorresultscontracts.ValidationStatusFailed,
			ProcessedAt: time.Unix(10, 0).UTC(),
			Violations: []validatorresultscontracts.ViolationRecord{{
				Rule:     "order_id_required",
				Field:    "order_id",
				Operator: "required",
				Severity: "error",
				Message:  "field is required",
			}},
		},
	})
	engine.Send(storePID, recordValidationResultMessage{
		Result: validatorresultscontracts.ValidationResultRecord{
			ProcessingKey: "msg-2|global|default|orders|sales.order.created|ver-1|sum-1",
			MessageID:     "msg-2",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
			Status:      validatorresultscontracts.ValidationStatusFailed,
			ProcessedAt: time.Unix(20, 0).UTC(),
			Violations: []validatorresultscontracts.ViolationRecord{{
				Rule:     "order_id_required",
				Field:    "order_id",
				Operator: "required",
				Severity: "error",
				Message:  "field is required",
			}},
		},
	})

	result, err := engine.Request(storePID, listValidationIncidentsMessage{
		Query: validatorincidentscontracts.ListValidationIncidentsQuery{
			Status: validatorincidentscontracts.ValidationIncidentStatusOpen,
		},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("query incidents store: %v", err)
	}

	reply := result.(listValidationIncidentsResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}
	if len(reply.Reply.Incidents) != 1 {
		t.Fatalf("expected one incident, got %+v", reply.Reply.Incidents)
	}
	incident := reply.Reply.Incidents[0]
	if incident.Count != 2 || incident.LatestMessageID != "msg-2" {
		t.Fatalf("unexpected aggregated incident %+v", incident)
	}
}

func TestValidationResultsStoreActorDeduplicatesByProcessingKey(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-results-dedupe-test")
	base := validatorresultscontracts.ValidationResultRecord{
		ProcessingKey: "dup|global|default|orders|sales.order.created|ver-1|sum-1",
		MessageID:     "msg-1",
		Binding: validatorresultscontracts.ValidationBindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
		},
		Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1", DefinitionChecksum: "sum-1"},
		Status:      validatorresultscontracts.ValidationStatusFailed,
		ProcessedAt: time.Unix(10, 0).UTC(),
		Violations: []validatorresultscontracts.ViolationRecord{{
			Rule:     "order_id_required",
			Field:    "order_id",
			Operator: "required",
			Severity: "error",
			Message:  "field is required",
		}},
	}
	engine.Send(storePID, recordValidationResultMessage{Result: base})
	updated := base
	updated.MessageID = "msg-1-redelivery"
	updated.ProcessedAt = time.Unix(20, 0).UTC()
	engine.Send(storePID, recordValidationResultMessage{Result: updated})

	result, err := engine.Request(storePID, listValidationResultsMessage{
		Query: validatorresultscontracts.ListValidationResultsQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("query results store: %v", err)
	}

	reply := result.(listValidationResultsResult)
	if len(reply.Reply.Results) != 1 {
		t.Fatalf("expected one deduplicated result, got %+v", reply.Reply.Results)
	}
	if reply.Reply.Results[0].MessageID != "msg-1-redelivery" {
		t.Fatalf("expected latest result to replace duplicate, got %+v", reply.Reply.Results[0])
	}
}

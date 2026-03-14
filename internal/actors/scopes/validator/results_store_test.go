package validator

import (
	"testing"
	"time"

	actorcommon "internal/actors/common"
	sharedruntime "internal/application/runtimecontracts"
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
			MessageID: "msg-1",
			Binding: validatorresultscontracts.ValidationBindingRecord{
				Name:  "orders",
				Topic: "sales.order.created",
				Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			},
			Config:      validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1"},
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

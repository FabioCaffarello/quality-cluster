package validator

import (
	"testing"
	"time"

	actorcommon "internal/actors/common"
	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	sharedruntime "internal/application/runtimecontracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	configdomain "internal/domain/configctl"
)

func TestValidationRouterRoutesWorkAndStoresResult(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "validation-router-cache-test")
	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-router-store-test")
	routerPID := engine.Spawn(NewValidationRouterActor(ValidationRouterConfig{
		RuntimeCachePID: cachePID,
		ResultsStorePID: storePID,
		WorkerCount:     1,
		RequestTimeout:  time.Second,
	}), "validation-router-test")

	engine.Send(cachePID, applyRuntimeUpdateMessage{
		Event: configdomain.ConfigActivatedEvent{Projection: configdomain.RuntimeProjection{
			Scope:              configdomain.DefaultActivationScope(),
			ConfigSetID:        "set-1",
			ConfigKey:          "orders",
			VersionID:          "cfg-123",
			Version:            1,
			Artifact:           configdomain.CompilationArtifact{ID: "artifact-1", Checksum: "checksum-1", RuntimeLoader: "validator:v1", SchemaVersion: "runtime/v1", StorageRef: "memory://artifact-1", Capabilities: []string{configdomain.RuntimeCapabilityRuleRequired}, CreatedAt: time.Unix(10, 0).UTC()},
			ActivatedAt:        time.Unix(20, 0).UTC(),
			DefinitionChecksum: "definition-1",
			Rules: []configdomain.Rule{
				{Name: "customer-required", Field: "customer_id", Operator: configdomain.RuleOperatorRequired, Severity: configdomain.RuleSeverityError},
			},
		}},
	})

	message := mustDataPlaneMessage(t, configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
		Runtime: sharedruntime.RuntimeRecord{
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			Config: sharedruntime.ConfigRecord{
				VersionID:          "cfg-123",
				DefinitionChecksum: "definition-1",
			},
		},
	}, `{"customer_id":"c-1"}`)

	rawResult, err := engine.Request(routerPID, routeValidationMessage{Message: message}, time.Second).Result()
	if err != nil {
		t.Fatalf("route validation message: %v", err)
	}

	reply := rawResult.(routeValidationResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}

	resultsRaw, err := engine.Request(storePID, listValidationResultsMessage{
		Query: validatorresultscontracts.ListValidationResultsQuery{},
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("list validation results: %v", err)
	}

	results := resultsRaw.(listValidationResultsResult)
	if len(results.Reply.Results) != 1 {
		t.Fatalf("expected one validation result, got %+v", results.Reply.Results)
	}
	if results.Reply.Results[0].Status != validatorresultscontracts.ValidationStatusPassed {
		t.Fatalf("expected passed validation result, got %+v", results.Reply.Results[0])
	}
}

func TestValidationRouterReturnsProblemWhenRuntimeIsMissing(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	cachePID := engine.Spawn(NewRuntimeCacheActor(), "validation-router-empty-cache-test")
	storePID := engine.Spawn(NewValidationResultsStoreActor(), "validation-router-empty-store-test")
	routerPID := engine.Spawn(NewValidationRouterActor(ValidationRouterConfig{
		RuntimeCachePID: cachePID,
		ResultsStorePID: storePID,
		WorkerCount:     1,
		RequestTimeout:  time.Second,
	}), "validation-router-empty-test")

	message := mustDataPlaneMessage(t, configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
		Runtime: sharedruntime.RuntimeRecord{
			Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			Config: sharedruntime.ConfigRecord{VersionID: "cfg-123"},
		},
	}, `{"customer_id":"c-1"}`)

	rawResult, err := engine.Request(routerPID, routeValidationMessage{Message: message}, time.Second).Result()
	if err != nil {
		t.Fatalf("route validation message: %v", err)
	}

	reply := rawResult.(routeValidationResult)
	if reply.Prob == nil || reply.Prob.Code == "" {
		t.Fatalf("expected missing runtime problem, got %+v", reply)
	}
}

func mustDataPlaneMessage(t *testing.T, binding configctlcontracts.ActiveIngestionBindingRecord, payload string) dataplaneapp.Message {
	t.Helper()

	message, prob := dataplaneapp.NewMessage(binding, []byte(payload), dataplaneapp.OriginRecord{
		Source:      dataplaneapp.SourceKafka,
		Topic:       binding.Binding.Topic,
		PublishedAt: time.Unix(10, 0).UTC(),
	}, dataplaneapp.MetadataRecord{
		MessageID:     dataplaneapp.MessageIDForKafkaRecord(binding, binding.Binding.Topic, 0, 1),
		CorrelationID: "corr-1",
		IngestedAt:    time.Unix(20, 0).UTC(),
		ContentType:   dataplaneapp.ContentTypeJSON,
	})
	if prob != nil {
		t.Fatalf("new data plane message: %v", prob)
	}
	return message
}

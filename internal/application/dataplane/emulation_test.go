package dataplane

import (
	"encoding/json"
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
)

func TestBuildSyntheticRecordUsesFieldShapes(t *testing.T) {
	t.Parallel()

	record, prob := BuildSyntheticRecord(configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders"},
		Fields: []configctlcontracts.FieldRecord{
			{Name: "order_id", Type: "string", Required: true},
			{Name: "amount", Type: "number"},
			{Name: "processed", Type: "boolean"},
			{Name: "created_at", Type: "timestamp"},
		},
	}, SyntheticInput{
		Now:      time.Unix(50, 0).UTC(),
		Sequence: 7,
		Scenario: SyntheticScenarioValid,
	})
	if prob != nil {
		t.Fatalf("build synthetic record: %v", prob)
	}

	var body map[string]any
	if err := json.Unmarshal(record.Payload, &body); err != nil {
		t.Fatalf("decode payload: %v", err)
	}
	if body["order_id"] == "" {
		t.Fatalf("expected string field to be populated, got %v", body)
	}
	if _, ok := body["amount"].(float64); !ok {
		t.Fatalf("expected number field, got %T", body["amount"])
	}
	if _, ok := body["processed"].(bool); !ok {
		t.Fatalf("expected boolean field, got %T", body["processed"])
	}
	if record.Key == "" {
		t.Fatal("expected synthetic record key")
	}
}

func TestBuildSyntheticRecordSupportsSimpleInvalidScenario(t *testing.T) {
	t.Parallel()

	record, prob := BuildSyntheticRecord(configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders"},
		Fields: []configctlcontracts.FieldRecord{
			{Name: "order_id", Type: "string", Required: true},
			{Name: "status", Type: "string", Required: true},
		},
	}, SyntheticInput{
		Now:      time.Unix(50, 0).UTC(),
		Sequence: 7,
		Scenario: SyntheticScenarioInvalidMissingField,
	})
	if prob != nil {
		t.Fatalf("build invalid synthetic record: %v", prob)
	}

	var body map[string]any
	if err := json.Unmarshal(record.Payload, &body); err != nil {
		t.Fatalf("decode payload: %v", err)
	}
	if _, exists := body["order_id"]; exists {
		t.Fatalf("expected invalid scenario to remove a required field, got %v", body)
	}
}

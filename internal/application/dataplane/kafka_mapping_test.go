package dataplane

import (
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
)

func TestMapKafkaRecordBuildsCanonicalMessage(t *testing.T) {
	t.Parallel()

	record, prob := NewKafkaRecord(
		"sales.order.created",
		[]byte("order-1"),
		[]byte(`{"order_id":"1"}`),
		map[string]string{
			"content-type":     "application/json",
			"x-correlation-id": "corr-1",
		},
		2,
		30,
		time.Unix(10, 0).UTC(),
	)
	if prob != nil {
		t.Fatalf("new kafka record: %v", prob)
	}

	mapped, prob := MapKafkaRecord(configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
		Runtime: sharedruntime.RuntimeRecord{
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			Config: sharedruntime.ConfigRecord{
				SetID:              "set-1",
				Key:                "orders-prod",
				VersionID:          "ver-1",
				Version:            3,
				DefinitionChecksum: "definition-1",
			},
		},
	}, DefaultRegistry(), record, time.Unix(20, 0).UTC())
	if prob != nil {
		t.Fatalf("map kafka record: %v", prob)
	}

	if mapped.Route.JetStreamSubject != "dataplane.ingestion.received.global.default.orders" {
		t.Fatalf("unexpected route %+v", mapped.Route)
	}
	if mapped.CorrelationID != "corr-1" {
		t.Fatalf("expected correlation id to be preserved, got %q", mapped.CorrelationID)
	}
	if mapped.Message.Origin.Source != SourceKafka {
		t.Fatalf("expected kafka source, got %+v", mapped.Message.Origin)
	}
	if mapped.Message.Metadata.MessageID == "" {
		t.Fatalf("expected message id to be populated, got %+v", mapped.Message.Metadata)
	}
}

func TestNewKafkaRecordRejectsInvalidMetadata(t *testing.T) {
	t.Parallel()

	_, prob := NewKafkaRecord("", nil, nil, nil, -1, -1, time.Time{})
	if prob == nil {
		t.Fatal("expected invalid kafka record to fail")
	}
}

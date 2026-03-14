package dataplane

import (
	"testing"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
)

func TestDefaultRegistryBuildsDeterministicChannelNames(t *testing.T) {
	t.Parallel()

	registry := DefaultRegistry()
	route, prob := registry.RouteForBinding(configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "Orders V1", Topic: "sales.order.created"},
		Runtime: sharedruntime.RuntimeRecord{
			Scope: sharedruntime.ScopeRecord{Kind: "Tenant", Key: "BR-East"},
		},
	})
	if prob != nil {
		t.Fatalf("channel for binding: %v", prob)
	}

	if route.KafkaTopic != "sales.order.created" {
		t.Fatalf("expected kafka topic to stay untouched, got %q", route.KafkaTopic)
	}
	if route.JetStreamSubject != "dataplane.ingestion.received.tenant.br-east.orders-v1" {
		t.Fatalf("unexpected jetstream subject %q", route.JetStreamSubject)
	}
}

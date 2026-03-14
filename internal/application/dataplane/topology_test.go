package dataplane

import (
	"testing"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
)

func TestNewRuntimeTopologyBuildsRoutesPerTopic(t *testing.T) {
	t.Parallel()

	index, prob := NewBindingIndex([]configctlcontracts.ActiveIngestionBindingRecord{
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
				Config: sharedruntime.ConfigRecord{VersionID: "cfg-1"},
			},
		},
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders-audit", Topic: "sales.order.created"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
				Config: sharedruntime.ConfigRecord{VersionID: "cfg-2"},
			},
		},
	})
	if prob != nil {
		t.Fatalf("new binding index: %v", prob)
	}

	topology, prob := NewRuntimeTopology(index, DefaultRegistry())
	if prob != nil {
		t.Fatalf("new runtime topology: %v", prob)
	}

	if got := topology.TopicNames(); len(got) != 1 || got[0] != "sales.order.created" {
		t.Fatalf("unexpected topics: %+v", got)
	}

	bindings := topology.BindingsForTopic("sales.order.created")
	if len(bindings) != 2 {
		t.Fatalf("expected two bindings for topic, got %+v", bindings)
	}
	if bindings[0].Route.JetStreamSubject != "dataplane.ingestion.received.global.default.orders" {
		t.Fatalf("unexpected first route: %+v", bindings[0].Route)
	}
	if bindings[1].Route.JetStreamSubject != "dataplane.ingestion.received.tenant.br.orders-audit" {
		t.Fatalf("unexpected second route: %+v", bindings[1].Route)
	}
}

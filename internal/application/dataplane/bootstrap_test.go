package dataplane

import (
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
)

func TestNewBindingIndexGroupsBindingsByTopic(t *testing.T) {
	t.Parallel()

	index, prob := NewBindingIndex([]configctlcontracts.ActiveIngestionBindingRecord{
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders-br", Topic: "sales.order.created"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:       sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
				Config:      sharedruntime.ConfigRecord{VersionID: "ver-1"},
				ActivatedAt: time.Unix(10, 0).UTC(),
			},
		},
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders-us", Topic: "sales.order.created"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:       sharedruntime.ScopeRecord{Kind: "tenant", Key: "us"},
				Config:      sharedruntime.ConfigRecord{VersionID: "ver-2"},
				ActivatedAt: time.Unix(20, 0).UTC(),
			},
		},
	})
	if prob != nil {
		t.Fatalf("expected binding index, got %v", prob)
	}

	if len(index.Topics()) != 1 {
		t.Fatalf("expected one topic, got %v", index.Topics())
	}
	if got := index.BindingsForTopic("sales.order.created"); len(got) != 2 {
		t.Fatalf("expected two bindings for topic, got %d", len(got))
	}
}

func TestNewBindingIndexRejectsDuplicateBindingRoute(t *testing.T) {
	t.Parallel()

	_, prob := NewBindingIndex([]configctlcontracts.ActiveIngestionBindingRecord{
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
				Config: sharedruntime.ConfigRecord{VersionID: "ver-1"},
			},
		},
		{
			Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.updated"},
			Runtime: sharedruntime.RuntimeRecord{
				Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
				Config: sharedruntime.ConfigRecord{VersionID: "ver-1"},
			},
		},
	})
	if prob == nil {
		t.Fatal("expected duplicate binding route to fail")
	}
}

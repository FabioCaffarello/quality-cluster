package dataplane

import (
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	sharedruntime "internal/application/runtimecontracts"
)

func TestNewMessageBuildsCanonicalContract(t *testing.T) {
	t.Parallel()

	message, prob := NewMessage(configctlcontracts.ActiveIngestionBindingRecord{
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
	}, []byte(`{"order_id":"42"}`), OriginRecord{
		Source:      SourceKafka,
		Topic:       "sales.order.created",
		Key:         "order-42",
		PublishedAt: time.Unix(20, 0).UTC(),
	}, MetadataRecord{
		MessageID:   "kafka:sales.order.created:1:42:global:default:ver-1:orders",
		IngestedAt:  time.Unix(30, 0).UTC(),
		ContentType: "application/json",
	})
	if prob != nil {
		t.Fatalf("expected canonical message, got %v", prob)
	}

	if message.Binding.Config.VersionID != "ver-1" {
		t.Fatalf("expected version id to be preserved, got %+v", message.Binding)
	}
	if message.Origin.Source != SourceKafka {
		t.Fatalf("expected source to be preserved, got %+v", message.Origin)
	}
	if message.Metadata.ContentType != ContentTypeJSON {
		t.Fatalf("expected default content type, got %q", message.Metadata.ContentType)
	}
	if message.MessageID() == "" {
		t.Fatal("expected message id")
	}
}

func TestNewMessageRejectsInvalidJSONPayload(t *testing.T) {
	t.Parallel()

	_, prob := NewMessage(configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
		Runtime: sharedruntime.RuntimeRecord{
			Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			Config: sharedruntime.ConfigRecord{VersionID: "ver-1"},
		},
	}, []byte(`not-json`), OriginRecord{
		Source:      SourceKafka,
		Topic:       "sales.order.created",
		PublishedAt: time.Unix(20, 0).UTC(),
	}, MetadataRecord{
		MessageID:  "msg-1",
		IngestedAt: time.Unix(30, 0).UTC(),
	})
	if prob == nil {
		t.Fatal("expected invalid payload to fail")
	}
}

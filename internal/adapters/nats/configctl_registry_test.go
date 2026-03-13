package nats

import "testing"

func TestConfigctlRegistryKeepsSubjectsAndStreamsSeparated(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	subjects := map[string]struct{}{
		registry.CreateDraft.Subject:   {},
		registry.GetConfig.Subject:     {},
		registry.GetActive.Subject:     {},
		registry.ListConfigs.Subject:   {},
		registry.ValidateDraft.Subject: {},
	}

	if len(subjects) != 5 {
		t.Fatalf("expected unique control subjects, got %d", len(subjects))
	}
	if registry.RuntimeUpdated.Stream.Name == "" {
		t.Fatal("expected runtime stream name")
	}
	if registry.RuntimeUpdated.Stream.Name == registry.CreateDraft.Subject {
		t.Fatal("expected runtime stream registry to stay separate from control plane")
	}
}

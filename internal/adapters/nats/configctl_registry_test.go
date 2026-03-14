package nats

import "testing"

func TestConfigctlRegistryKeepsSubjectsAndStreamsSeparated(t *testing.T) {
	t.Parallel()

	registry := DefaultConfigctlRegistry()
	subjects := map[string]struct{}{
		registry.CreateDraft.Subject:                 {},
		registry.GetConfig.Subject:                   {},
		registry.GetActive.Subject:                   {},
		registry.ListActiveIngestionBindings.Subject: {},
		registry.ListConfigs.Subject:                 {},
		registry.ValidateDraft.Subject:               {},
		registry.ValidateConfig.Subject:              {},
		registry.CompileConfig.Subject:               {},
		registry.ActivateConfig.Subject:              {},
	}

	if len(subjects) != 9 {
		t.Fatalf("expected unique control subjects, got %d", len(subjects))
	}
	if registry.Activated.Stream.Name == "" {
		t.Fatal("expected runtime stream name")
	}
	if registry.Activated.Stream.Name == registry.CreateDraft.Subject {
		t.Fatal("expected runtime stream registry to stay separate from control plane")
	}
	if registry.IngestionRuntimeChanged.Subject == registry.Activated.Subject {
		t.Fatal("expected ingestion runtime changed event subject to stay separate from config.activated")
	}
}

func TestValidatorRuntimeRegistryUsesDedicatedControlSubject(t *testing.T) {
	t.Parallel()

	registry := DefaultValidatorRuntimeRegistry()
	if registry.GetActive.Subject == "" {
		t.Fatal("expected validator runtime subject")
	}
	if registry.GetActive.Subject == DefaultConfigctlRegistry().GetActive.Subject {
		t.Fatal("expected validator runtime control subject to stay separate from configctl")
	}
}

func TestValidatorResultsRegistryUsesDedicatedControlSubject(t *testing.T) {
	t.Parallel()

	registry := DefaultValidatorResultsRegistry()
	if registry.List.Subject == "" {
		t.Fatal("expected validator results subject")
	}
	if registry.List.Subject == DefaultValidatorRuntimeRegistry().GetActive.Subject {
		t.Fatal("expected validator results control subject to stay separate from validator runtime")
	}
}

func TestDataPlaneRegistryUsesDedicatedStreamAndDurable(t *testing.T) {
	t.Parallel()

	registry := DefaultDataPlaneRegistry()
	if registry.Ingested.Stream.Name == "" {
		t.Fatal("expected data plane stream name")
	}
	if registry.Ingested.SubjectPattern == DefaultConfigctlRegistry().Activated.Subject {
		t.Fatal("expected data plane subject space to stay separate from configctl events")
	}
	if registry.Ingested.ValidatorDurable == DefaultConfigctlRegistry().ValidatorRuntime.Durable {
		t.Fatal("expected validator durable names to stay separate across planes")
	}
}

package validatorresults

import (
	"testing"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	sharedruntime "internal/application/runtimecontracts"
	validatorcontracts "internal/application/validatorresults/contracts"
	configdomain "internal/domain/configctl"
)

func TestEvaluateReturnsPassedWhenRulesMatch(t *testing.T) {
	t.Parallel()

	result, prob := Evaluate(configdomain.RuntimeProjection{
		Scope:              configdomain.DefaultActivationScope(),
		ConfigSetID:        "set-1",
		ConfigKey:          "orders-prod",
		VersionID:          "ver-1",
		Version:            2,
		DefinitionChecksum: "definition-1",
		Rules: []configdomain.Rule{
			{Name: "order_id_required", Field: "order_id", Operator: configdomain.RuleOperatorRequired, Severity: configdomain.RuleSeverityError},
			{Name: "status_not_empty", Field: "status", Operator: configdomain.RuleOperatorNotEmpty, Severity: configdomain.RuleSeverityError},
		},
	}, mustMessage(t, `{"order_id":"1","status":"approved"}`), time.Unix(20, 0).UTC())
	if prob != nil {
		t.Fatalf("evaluate message: %v", prob)
	}
	if result.Status != validatorcontracts.ValidationStatusPassed {
		t.Fatalf("expected passed status, got %+v", result)
	}
}

func TestEvaluateReturnsViolationsForMissingRequiredField(t *testing.T) {
	t.Parallel()

	result, prob := Evaluate(configdomain.RuntimeProjection{
		Scope:              configdomain.DefaultActivationScope(),
		ConfigSetID:        "set-1",
		ConfigKey:          "orders-prod",
		VersionID:          "ver-1",
		Version:            2,
		DefinitionChecksum: "definition-1",
		Rules: []configdomain.Rule{
			{Name: "order_id_required", Field: "order_id", Operator: configdomain.RuleOperatorRequired, Severity: configdomain.RuleSeverityError},
			{Name: "status_not_empty", Field: "status", Operator: configdomain.RuleOperatorNotEmpty, Severity: configdomain.RuleSeverityError},
		},
	}, mustMessage(t, `{"status":""}`), time.Unix(20, 0).UTC())
	if prob != nil {
		t.Fatalf("evaluate message: %v", prob)
	}
	if result.Status != validatorcontracts.ValidationStatusFailed {
		t.Fatalf("expected failed status, got %+v", result)
	}
	if len(result.Violations) != 2 {
		t.Fatalf("expected two violations, got %+v", result.Violations)
	}
}

func TestEvaluateSupportsEqualsOperator(t *testing.T) {
	t.Parallel()

	result, prob := Evaluate(configdomain.RuntimeProjection{
		Scope:              configdomain.DefaultActivationScope(),
		ConfigSetID:        "set-1",
		ConfigKey:          "orders-prod",
		VersionID:          "ver-1",
		Version:            2,
		DefinitionChecksum: "definition-1",
		Rules: []configdomain.Rule{
			{Name: "status_equals", Field: "status", Operator: configdomain.RuleOperatorEquals, ExpectedValue: "approved", Severity: configdomain.RuleSeverityWarn},
		},
	}, mustMessage(t, `{"status":"pending"}`), time.Unix(20, 0).UTC())
	if prob != nil {
		t.Fatalf("evaluate message: %v", prob)
	}
	if len(result.Violations) != 1 || result.Violations[0].Operator != "equals" {
		t.Fatalf("expected equals violation, got %+v", result.Violations)
	}
}

func mustMessage(t *testing.T, payload string) dataplaneapp.Message {
	t.Helper()

	message, prob := dataplaneapp.NewMessage(activeBindingRecord(), []byte(payload), dataplaneapp.OriginRecord{
		Source:      dataplaneapp.SourceKafka,
		Topic:       "sales.order.created",
		PublishedAt: time.Unix(10, 0).UTC(),
	}, dataplaneapp.MetadataRecord{
		MessageID:  "msg-1",
		IngestedAt: time.Unix(15, 0).UTC(),
	})
	if prob != nil {
		t.Fatalf("build message: %v", prob)
	}
	return message
}

func activeBindingRecord() configctlcontracts.ActiveIngestionBindingRecord {
	return configctlcontracts.ActiveIngestionBindingRecord{
		Binding: configctlcontracts.BindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
		},
		Runtime: sharedruntime.RuntimeRecord{
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
			Config: sharedruntime.ConfigRecord{
				SetID:              "set-1",
				Key:                "orders-prod",
				VersionID:          "ver-1",
				Version:            2,
				DefinitionChecksum: "definition-1",
			},
		},
	}
}

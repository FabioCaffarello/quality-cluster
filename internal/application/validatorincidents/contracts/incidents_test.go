package contracts

import (
	"testing"
	"time"

	sharedruntime "internal/application/runtimecontracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
)

func TestListValidationIncidentsQueryNormalizesDefaults(t *testing.T) {
	t.Parallel()

	query := (ListValidationIncidentsQuery{}).Normalize()
	if query.ScopeKind != "global" || query.ScopeKey != "default" {
		t.Fatalf("unexpected default scope %+v", query)
	}
	if query.Limit != DefaultListLimit {
		t.Fatalf("expected default limit %d, got %d", DefaultListLimit, query.Limit)
	}
}

func TestListValidationIncidentsQueryValidateRejectsInvalidKind(t *testing.T) {
	t.Parallel()

	prob := (ListValidationIncidentsQuery{Kind: "broken"}).Validate()
	if prob == nil {
		t.Fatal("expected invalid kind to fail validation")
	}
}

func TestValidationIncidentRecordValidate(t *testing.T) {
	t.Parallel()

	prob := (ValidationIncidentRecord{
		IncidentKey: "validation.rule_violation|global|default|orders|sales.order.created|ver-1|order_id_required:order_id:required:error",
		Kind:        ValidationIncidentKindRuleViolation,
		Status:      ValidationIncidentStatusOpen,
		Binding: ValidationIncidentBindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
		},
		Config:              ValidationIncidentConfigRecord{VersionID: "ver-1"},
		Count:               2,
		FirstSeenAt:         time.Unix(10, 0).UTC(),
		LastSeenAt:          time.Unix(20, 0).UTC(),
		LatestMessageID:     "msg-2",
		LatestProcessingKey: "proc-2",
		Violations: []validatorresultscontracts.ViolationRecord{{
			Rule:     "order_id_required",
			Field:    "order_id",
			Operator: "required",
			Severity: "error",
			Message:  "field is required",
		}},
	}).Validate()
	if prob != nil {
		t.Fatalf("expected valid incident, got %v", prob)
	}
}

func TestBuildIncidentKeyUsesScopeBindingVersionAndViolations(t *testing.T) {
	t.Parallel()

	key := BuildIncidentKey(validatorresultscontracts.ValidationResultRecord{
		MessageID:     "msg-1",
		ProcessingKey: "proc-1",
		Binding: validatorresultscontracts.ValidationBindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
		},
		Config: validatorresultscontracts.ValidationConfigRecord{VersionID: "ver-1"},
		Status: validatorresultscontracts.ValidationStatusFailed,
		Violations: []validatorresultscontracts.ViolationRecord{
			{Rule: "rule-b", Field: "field-b", Operator: "required", Severity: "error"},
			{Rule: "rule-a", Field: "field-a", Operator: "equals", Severity: "warn"},
		},
	})
	expected := "validation.rule_violation|global|default|orders|sales.order.created|ver-1|rule-a:field-a:equals:warn,rule-b:field-b:required:error"
	if key != expected {
		t.Fatalf("expected %q, got %q", expected, key)
	}
}

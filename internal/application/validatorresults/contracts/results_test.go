package contracts

import (
	"testing"
	"time"

	sharedruntime "internal/application/runtimecontracts"
)

func TestListValidationResultsQueryNormalizesDefaults(t *testing.T) {
	t.Parallel()

	query := (ListValidationResultsQuery{}).Normalize()
	if query.ScopeKind != "global" || query.ScopeKey != "default" {
		t.Fatalf("unexpected default scope %+v", query)
	}
	if query.Limit != DefaultListLimit {
		t.Fatalf("expected default limit %d, got %d", DefaultListLimit, query.Limit)
	}
}

func TestListValidationResultsQueryNormalizesStatus(t *testing.T) {
	t.Parallel()

	query := (ListValidationResultsQuery{Status: " FAILED "}).Normalize()
	if query.Status != ValidationStatusFailed {
		t.Fatalf("expected normalized failed status, got %+v", query)
	}
}

func TestListValidationResultsQueryValidateRejectsInvalidStatus(t *testing.T) {
	t.Parallel()

	prob := (ListValidationResultsQuery{Status: "broken"}).Validate()
	if prob == nil {
		t.Fatal("expected invalid status to fail validation")
	}
}

func TestValidationResultRecordValidate(t *testing.T) {
	t.Parallel()

	prob := (ValidationResultRecord{
		ProcessingKey: "msg-1|global|default|orders|sales.order.created|ver-1|",
		MessageID:     "msg-1",
		Binding: ValidationBindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
		},
		Config: ValidationConfigRecord{VersionID: "ver-1"},
		Status: ValidationStatusFailed,
		Violations: []ViolationRecord{{
			Rule:     "order_id_required",
			Field:    "order_id",
			Operator: "required",
			Severity: "error",
			Message:  "field is required",
		}},
		ProcessedAt: time.Unix(10, 0).UTC(),
	}).Validate()
	if prob != nil {
		t.Fatalf("expected valid result, got %v", prob)
	}
}

func TestValidationResultRecordBuildsFallbackProcessingKey(t *testing.T) {
	t.Parallel()

	record := ValidationResultRecord{
		MessageID: "msg-1",
		Binding: ValidationBindingRecord{
			Name:  "orders",
			Topic: "sales.order.created",
			Scope: sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
		},
		Config: ValidationConfigRecord{
			VersionID:          "ver-1",
			DefinitionChecksum: "sum-1",
		},
	}

	expected := "msg-1|global|default|orders|sales.order.created|ver-1|sum-1"
	if got := record.NormalizedProcessingKey(); got != expected {
		t.Fatalf("expected processing key %q, got %q", expected, got)
	}
}

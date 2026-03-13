package envelope

import (
	"testing"
	"time"

	"internal/shared/problem"
)

func TestNewEnvelopeAppliesDefaults(t *testing.T) {
	env := New(KindEvent, "quality.created", map[string]string{"id": "1"}).
		WithSource("validator").
		WithSubject("quality.created").
		WithCorrelationID("corr-1").
		WithHeader("tenant", "acme")

	if env.ID == "" {
		t.Fatalf("expected envelope id to be generated")
	}
	if env.ContentType != DefaultContentType {
		t.Fatalf("expected default content type, got %q", env.ContentType)
	}
	if env.Timestamp.IsZero() {
		t.Fatalf("expected timestamp to be set")
	}
	if prob := env.Validate(); prob != nil {
		t.Fatalf("expected envelope to be valid, got %v", prob)
	}
}

func TestEnvelopeValidationIsStructured(t *testing.T) {
	env := Envelope[any]{
		Headers: map[string]string{
			"": "invalid",
		},
		Timestamp: time.Time{},
	}

	prob := env.Validate()
	if prob == nil {
		t.Fatalf("expected invalid envelope")
	}
	if prob.Code != problem.InvalidArgument {
		t.Fatalf("expected invalid argument problem, got %q", prob.Code)
	}

	rawIssues := prob.Details[problem.DetailIssues]
	issues, ok := rawIssues.([]problem.ValidationIssue)
	if !ok {
		t.Fatalf("expected typed issues, got %#v", rawIssues)
	}
	if len(issues) < 4 {
		t.Fatalf("expected multiple issues, got %d", len(issues))
	}
}
